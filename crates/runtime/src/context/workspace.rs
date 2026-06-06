use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use super::ContextualUserFragment;

const MAX_DEPTH: usize = 3;
const MAX_SNAPSHOT_LINES: usize = 1_000;
const MAX_DIRECTORY_CHILD_LINES: usize = 100;
const RECENT_FILE_DAYS: u64 = 7;
const MAX_RECENT_FILES: usize = 100;

pub(crate) struct WorkspaceSnapshot {
    cwd: PathBuf,
    total_files: usize,
    total_dirs: usize,
    extension_counts: BTreeMap<String, usize>,
    entries: Vec<WorkspaceSnapshotEntry>,
    recent_files: Vec<WorkspaceSnapshotEntry>,
    omitted_recent_files: usize,
}

#[derive(Clone)]
struct WorkspaceSnapshotEntry {
    relative_path: String,
    kind: WorkspaceSnapshotEntryKind,
    modified_time: Option<SystemTime>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkspaceSnapshotEntryKind {
    Directory,
    File,
}

impl WorkspaceSnapshot {
    pub(crate) fn from_cwd(cwd: &Path) -> Option<Self> {
        let mut snapshot = Self {
            cwd: cwd.to_path_buf(),
            total_files: 0,
            total_dirs: 0,
            extension_counts: BTreeMap::new(),
            entries: Vec::new(),
            recent_files: Vec::new(),
            omitted_recent_files: 0,
        };

        collect_entries(cwd, cwd, /*depth*/ 0, &mut snapshot);
        snapshot
            .entries
            .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        snapshot.collect_recent_files();

        Some(snapshot)
    }

    fn collect_recent_files(&mut self) {
        let Some(cutoff) = SystemTime::now().checked_sub(std::time::Duration::from_secs(
            RECENT_FILE_DAYS * 24 * 60 * 60,
        )) else {
            return;
        };

        self.recent_files = self
            .entries
            .iter()
            .filter(|entry| entry.kind == WorkspaceSnapshotEntryKind::File)
            .filter(|entry| {
                entry
                    .modified_time
                    .is_some_and(|modified| modified >= cutoff)
            })
            .cloned()
            .collect::<Vec<_>>();
        self.recent_files.sort_by(|left, right| {
            right
                .modified_time
                .cmp(&left.modified_time)
                .then_with(|| left.relative_path.cmp(&right.relative_path))
        });
        self.omitted_recent_files = self.recent_files.len().saturating_sub(MAX_RECENT_FILES);
        self.recent_files.truncate(MAX_RECENT_FILES);
    }

    fn extension_counts_text(&self) -> String {
        if self.extension_counts.is_empty() {
            return "none".to_string();
        }

        self.extension_counts
            .iter()
            .map(|(suffix, count)| format!("{suffix}={count}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn text(&self) -> String {
        let header = vec![
            format!("cwd: {}", self.cwd.to_string_lossy()),
            format!("scan_depth: {MAX_DEPTH}"),
            format!("total_files: {}", self.total_files),
            format!("total_dirs: {}", self.total_dirs),
            format!("suffix_counts: {}", self.extension_counts_text()),
            "paths: relative paths only; directories end with /".to_string(),
        ];
        render_snapshot_lines(
            header,
            &self.entries,
            &self.recent_files,
            self.omitted_recent_files,
            MAX_SNAPSHOT_LINES,
        )
    }
}

impl ContextualUserFragment for WorkspaceSnapshot {
    const ROLE: &'static str = "user";
    const START_MARKER: &'static str = "<WORKSPACE_SNAPSHOT>";
    const END_MARKER: &'static str = "</WORKSPACE_SNAPSHOT>";

    fn body(&self) -> String {
        format!("\n{}\n", self.text())
    }
}

fn collect_entries(root: &Path, dir: &Path, depth: usize, snapshot: &mut WorkspaceSnapshot) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        if metadata.is_dir() {
            snapshot.total_dirs += 1;
            let modified_time = metadata.modified().ok();
            snapshot.entries.push(WorkspaceSnapshotEntry {
                relative_path: directory_relative_path_text(root, &path),
                kind: WorkspaceSnapshotEntryKind::Directory,
                modified_time,
            });
            if depth < MAX_DEPTH {
                collect_entries(root, &path, depth + 1, snapshot);
            }
            continue;
        }

        if !metadata.is_file() {
            continue;
        }

        snapshot.total_files += 1;
        let modified_time = metadata.modified().ok();
        let suffix = file_suffix(&path);
        *snapshot.extension_counts.entry(suffix.clone()).or_insert(0) += 1;
        snapshot.entries.push(WorkspaceSnapshotEntry {
            relative_path: relative_path_text(root, &path),
            kind: WorkspaceSnapshotEntryKind::File,
            modified_time,
        });
    }
}

fn relative_path_text(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn directory_relative_path_text(root: &Path, path: &Path) -> String {
    let mut text = relative_path_text(root, path);
    if !text.ends_with('/') {
        text.push('/');
    }
    text
}

fn file_suffix(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!(".{extension}"))
        .unwrap_or_else(|| "(none)".to_string())
}

fn render_snapshot_lines(
    header: Vec<String>,
    entries: &[WorkspaceSnapshotEntry],
    recent_files: &[WorkspaceSnapshotEntry],
    omitted_recent_files: usize,
    max_lines: usize,
) -> String {
    let recent_lines = recent_file_lines(recent_files, omitted_recent_files);
    let entry_lines = directory_limited_entry_lines(entries, MAX_DIRECTORY_CHILD_LINES);
    if header.len() + recent_lines.len() + entry_lines.len() <= max_lines {
        let mut lines = header;
        lines.extend(recent_lines);
        lines.extend(entry_lines);
        return lines.join("\n");
    }

    let max_entry_lines = max_lines.saturating_sub(header.len() + recent_lines.len() + 1);
    let omitted_entries = entry_lines.len().saturating_sub(max_entry_lines);

    let mut lines = header;
    lines.extend(recent_lines);
    if omitted_entries > 0 {
        lines.push(format!(
            "... omitted {omitted_entries} older path lines ..."
        ));
    }
    let (directory_entry_lines, other_entry_lines): (Vec<_>, Vec<_>) = entry_lines
        .into_iter()
        .partition(|line| line.ends_with('/'));
    lines.extend(
        directory_entry_lines
            .into_iter()
            .chain(other_entry_lines)
            .take(max_entry_lines),
    );
    lines.join("\n")
}

fn directory_limited_entry_lines(
    entries: &[WorkspaceSnapshotEntry],
    max_child_lines: usize,
) -> Vec<String> {
    let mut grouped: BTreeMap<String, Vec<&WorkspaceSnapshotEntry>> = BTreeMap::new();
    for entry in entries {
        grouped
            .entry(parent_directory_key(&entry.relative_path))
            .or_default()
            .push(entry);
    }

    let mut lines = Vec::new();
    for (directory, mut children) in grouped {
        children.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        if children.len() <= max_child_lines {
            lines.extend(children.into_iter().map(format_entry_line));
            continue;
        }

        let head_count = max_child_lines / 2;
        let tail_count = max_child_lines.saturating_sub(head_count);
        let omitted = children.len().saturating_sub(head_count + tail_count);
        lines.extend(
            children
                .iter()
                .take(head_count)
                .map(|entry| format_entry_line(entry)),
        );
        lines.push(format!(
            "directory_truncated: {} omitted {} entries in the middle; max_child_entries={}",
            directory_label(&directory),
            omitted,
            max_child_lines
        ));
        lines.extend(
            children
                .iter()
                .skip(children.len().saturating_sub(tail_count))
                .map(|entry| format_entry_line(entry)),
        );
    }
    lines
}

fn parent_directory_key(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rsplit_once('/') {
        Some((parent, _)) => format!("{parent}/"),
        None => String::new(),
    }
}

fn directory_label(directory: &str) -> &str {
    if directory.is_empty() {
        "."
    } else {
        directory
    }
}

fn recent_file_lines(
    recent_files: &[WorkspaceSnapshotEntry],
    omitted_recent_files: usize,
) -> Vec<String> {
    let mut lines = vec![
        format!(
            "recent_files: relative paths modified in the last {RECENT_FILE_DAYS} days, newest first, max {MAX_RECENT_FILES}"
        ),
    ];
    lines.extend(recent_files.iter().map(format_entry_line));
    if omitted_recent_files > 0 {
        lines.push(format!(
            "recent_files_truncated: omitted {omitted_recent_files} older files modified in the last {RECENT_FILE_DAYS} days"
        ));
    }
    lines
}

fn format_entry_line(entry: &WorkspaceSnapshotEntry) -> String {
    entry.relative_path.clone()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn workspace_snapshot_includes_depth_limited_file_metadata() {
        let temp = TempDir::new().expect("tempdir");
        fs::write(temp.path().join("root.rs"), "one\n").expect("write root");
        fs::create_dir(temp.path().join("a")).expect("mkdir a");
        fs::write(temp.path().join("a").join("child.py"), "one\ntwo\n").expect("write child");
        fs::create_dir(temp.path().join("a").join("b")).expect("mkdir b");
        fs::write(temp.path().join("a").join("b").join("grandchild.txt"), "x")
            .expect("write grandchild");
        fs::create_dir(temp.path().join("a").join("b").join("c")).expect("mkdir c");
        fs::write(
            temp.path()
                .join("a")
                .join("b")
                .join("c")
                .join("too_deep.txt"),
            "x",
        )
        .expect("write too deep");

        let rendered = WorkspaceSnapshot::from_cwd(temp.path())
            .expect("snapshot")
            .render();

        assert!(rendered.contains("<WORKSPACE_SNAPSHOT>"));
        assert!(rendered.contains("root.rs"));
        assert!(rendered.contains("a/"));
        assert!(rendered.contains("a/child.py"));
        assert!(rendered.contains("a/b/"));
        assert!(rendered.contains("a/b/grandchild.txt"));
        assert!(rendered.contains("a/b/c/"));
        assert!(rendered.contains("too_deep.txt"));
        assert!(rendered.contains("recent_files: relative paths modified in the last 7 days"));
        assert!(rendered.contains(".rs=1"));
        assert!(rendered.contains(".py=1"));
        assert!(rendered.contains(".txt=2"));
    }

    #[test]
    fn workspace_snapshot_truncation_preserves_directories_before_files() {
        let temp = TempDir::new().expect("tempdir");
        fs::create_dir_all(temp.path().join("pkg").join("sub")).expect("mkdir pkg/sub");
        for index in 0..20 {
            fs::write(
                temp.path().join("pkg").join(format!("file_{index:02}.py")),
                "x\n",
            )
            .expect("write file");
        }

        let snapshot = WorkspaceSnapshot::from_cwd(temp.path()).expect("snapshot");
        let rendered = render_snapshot_lines(
            vec![
                "cwd: test".to_string(),
                "scan_depth: 3".to_string(),
                "total_files: 20".to_string(),
                "total_dirs: 2".to_string(),
                "suffix_counts: .py=20".to_string(),
                "paths: relative paths only; directories end with /".to_string(),
            ],
            &snapshot.entries,
            &[],
            0,
            18,
        );

        assert!(rendered.contains("pkg/"));
        assert!(rendered.contains("pkg/sub/"));
        assert!(rendered.contains("... omitted "));
        assert!(rendered.contains("older path lines ..."));
    }

    #[test]
    fn workspace_snapshot_recent_files_are_capped() {
        let temp = TempDir::new().expect("tempdir");
        for index in 0..120 {
            fs::write(temp.path().join(format!("recent_{index:03}.py")), "x\n")
                .expect("write recent file");
        }

        let rendered = WorkspaceSnapshot::from_cwd(temp.path())
            .expect("snapshot")
            .render();

        assert!(rendered.contains(
            "recent_files: relative paths modified in the last 7 days, newest first, max 100"
        ));
        assert!(rendered.contains(
            "recent_files_truncated: omitted 20 older files modified in the last 7 days"
        ));
    }

    #[test]
    fn workspace_snapshot_includes_generated_dependency_and_binary_paths() {
        let temp = TempDir::new().expect("tempdir");
        fs::write(temp.path().join("Cargo.lock"), "keep\n").expect("write lock");
        fs::write(temp.path().join("app.py"), "keep\n").expect("write source");
        fs::create_dir_all(temp.path().join(".git").join("hooks")).expect("mkdir git");
        fs::write(temp.path().join(".git").join("config"), "noise\n").expect("write git");
        fs::create_dir_all(temp.path().join("node_modules").join("pkg"))
            .expect("mkdir node_modules");
        fs::write(
            temp.path()
                .join("node_modules")
                .join("pkg")
                .join("index.js"),
            "noise\n",
        )
        .expect("write dependency");
        fs::create_dir_all(temp.path().join("target").join("debug")).expect("mkdir target");
        fs::write(
            temp.path().join("target").join("debug").join("build.log"),
            "noise\n",
        )
        .expect("write build log");
        fs::create_dir_all(temp.path().join("src").join("__pycache__")).expect("mkdir pycache");
        fs::write(
            temp.path()
                .join("src")
                .join("__pycache__")
                .join("module.cpython-313.pyc"),
            "noise\n",
        )
        .expect("write pyc");

        let rendered = WorkspaceSnapshot::from_cwd(temp.path())
            .expect("snapshot")
            .render();

        assert!(rendered.contains("Cargo.lock"));
        assert!(rendered.contains("app.py"));
        assert!(rendered.contains(".git/"));
        assert!(rendered.contains(".git/config"));
        assert!(rendered.contains("node_modules/"));
        assert!(rendered.contains("node_modules/pkg/index.js"));
        assert!(rendered.contains("target/"));
        assert!(rendered.contains("target/debug/build.log"));
        assert!(rendered.contains("__pycache__/"));
        assert!(rendered.contains(".pyc"));
        assert!(rendered.contains(".log"));
    }

    #[test]
    fn workspace_snapshot_truncates_large_directory_in_the_middle() {
        let temp = TempDir::new().expect("tempdir");
        fs::create_dir_all(temp.path().join("large")).expect("mkdir large");
        for index in 0..130 {
            fs::write(
                temp.path()
                    .join("large")
                    .join(format!("file_{index:03}.py")),
                "x\n",
            )
            .expect("write file");
        }

        let snapshot = WorkspaceSnapshot::from_cwd(temp.path()).expect("snapshot");
        let rendered = render_snapshot_lines(
            vec![
                "cwd: test".to_string(),
                "scan_depth: 3".to_string(),
                "total_files: 130".to_string(),
                "total_dirs: 1".to_string(),
                "suffix_counts: .py=130".to_string(),
                "paths: relative paths only; directories end with /".to_string(),
            ],
            &snapshot.entries,
            &[],
            0,
            2_000,
        );

        assert!(rendered.contains("large/file_000.py"));
        assert!(rendered.contains("large/file_129.py"));
        assert!(rendered.contains(
            "directory_truncated: large/ omitted 30 entries in the middle; max_child_entries=100"
        ));
        assert!(!rendered.contains("large/file_060.py"));
    }
}
