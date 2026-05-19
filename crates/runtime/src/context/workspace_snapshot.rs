use std::collections::BTreeMap;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use chrono::DateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_utils_output_truncation::formatted_truncate_lines;

use super::ContextualUserFragment;

const MAX_DEPTH: usize = 2;
const MAX_SNAPSHOT_LINES: usize = 8_000;

pub(crate) struct WorkspaceSnapshot {
    cwd: PathBuf,
    total_files: usize,
    total_dirs: usize,
    extension_counts: BTreeMap<String, usize>,
    entries: Vec<WorkspaceSnapshotEntry>,
}

struct WorkspaceSnapshotEntry {
    relative_path: String,
    modified_utc: String,
    line_count: String,
    suffix: String,
}

impl WorkspaceSnapshot {
    pub(crate) fn from_cwd(cwd: &Path) -> Option<Self> {
        let mut snapshot = Self {
            cwd: cwd.to_path_buf(),
            total_files: 0,
            total_dirs: 0,
            extension_counts: BTreeMap::new(),
            entries: Vec::new(),
        };

        collect_entries(cwd, cwd, /*depth*/ 0, &mut snapshot);
        snapshot
            .entries
            .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

        Some(snapshot)
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
        let mut lines = vec![
            format!("cwd: {}", self.cwd.to_string_lossy()),
            format!("scan_depth: {MAX_DEPTH}"),
            format!("total_files: {}", self.total_files),
            format!("total_dirs: {}", self.total_dirs),
            format!("suffix_counts: {}", self.extension_counts_text()),
            "columns: modified_utc | lines | suffix | path".to_string(),
        ];
        lines.extend(self.entries.iter().map(|entry| {
            format!(
                "{} | {} | {} | {}",
                entry.modified_utc, entry.line_count, entry.suffix, entry.relative_path
            )
        }));
        formatted_truncate_lines(&lines.join("\n"), MAX_SNAPSHOT_LINES)
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
            if depth < MAX_DEPTH {
                collect_entries(root, &path, depth + 1, snapshot);
            }
            continue;
        }

        if !metadata.is_file() {
            continue;
        }

        snapshot.total_files += 1;
        let suffix = file_suffix(&path);
        *snapshot.extension_counts.entry(suffix.clone()).or_insert(0) += 1;
        snapshot.entries.push(WorkspaceSnapshotEntry {
            relative_path: relative_path_text(root, &path),
            modified_utc: metadata
                .modified()
                .map(format_system_time)
                .unwrap_or_else(|_| "unknown".to_string()),
            line_count: count_lines(&path)
                .map(|count| count.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            suffix,
        });
    }
}

fn relative_path_text(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn file_suffix(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!(".{extension}"))
        .unwrap_or_else(|| "(none)".to_string())
}

fn format_system_time(time: SystemTime) -> String {
    let datetime: DateTime<Utc> = time.into();
    datetime.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn count_lines(path: &Path) -> Option<usize> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut count = 0usize;
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        let bytes = reader.read_until(b'\n', &mut buffer).ok()?;
        if bytes == 0 {
            break;
        }
        count += 1;
    }

    Some(count)
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
        assert!(rendered.contains("a/child.py"));
        assert!(rendered.contains("a/b/grandchild.txt"));
        assert!(!rendered.contains("too_deep.txt"));
        assert!(rendered.contains(".rs=1"));
        assert!(rendered.contains(".py=1"));
        assert!(rendered.contains(".txt=1"));
    }
}
