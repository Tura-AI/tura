use super::output::results_summary;
use super::paths::expand_media_paths;
use super::previews::compact_visual_previews;
use super::processing::{media_result, process_media_file};
use super::types::ReadMediaArgs;
use super::types::ReadMode;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::mpsc;

pub(super) fn run_read_media(
    args: Result<ReadMediaArgs, String>,
    session_dir: &Path,
) -> Result<Value, String> {
    let args = args?;
    let expanded = expand_media_paths(&args, session_dir)?;
    let mode = if expanded.len() == 1 {
        ReadMode::Detailed
    } else {
        ReadMode::ThumbnailOnly
    };
    let (tx, rx) = mpsc::channel();
    let mut worker_count = 0usize;
    for (index, (path, resolved)) in expanded.into_iter().enumerate() {
        let args = args.clone();
        let tx = tx.clone();
        worker_count += 1;
        std::thread::spawn(move || {
            let item = match process_media_file(&resolved, &args, mode) {
                Ok(content) => media_result(&path, &resolved, content),
                Err(err) => json!({
                    "path": path,
                    "resolved_path": resolved.display().to_string(),
                    "success": false,
                    "error": err,
                }),
            };
            let _ = tx.send((index, item));
        });
    }
    drop(tx);
    let mut indexed = Vec::new();
    for _ in 0..worker_count {
        match rx.recv() {
            Ok(item) => indexed.push(item),
            Err(_) => break,
        }
    }
    indexed.sort_by_key(|(index, _)| *index);
    let results = indexed
        .into_iter()
        .map(|(_, item)| item)
        .collect::<Vec<_>>();
    let mut output = json!({
        "media_results": results,
    });
    compact_visual_previews(&mut output)?;
    let summary = output
        .get("media_results")
        .and_then(Value::as_array)
        .map(|items| results_summary(items))
        .unwrap_or_default();
    output["summary_markdown"] = json!(summary);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::run_read_media;
    use crate::args::parse_args_text;

    #[test]
    fn run_read_media_reads_single_text_file_in_detailed_mode() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("notes.txt"), "hello from read_media").expect("write notes");

        let output = run_read_media(parse_args_text("notes.txt"), dir.path())
            .expect("read_media should read text file");
        let result = &output["media_results"][0];

        assert_eq!(result["success"], true);
        assert_eq!(result["path"], "notes.txt");
        assert_eq!(result["media_type"], "document");
        assert_eq!(result["extracted_text"], "hello from read_media");
        assert!(output["summary_markdown"]
            .as_str()
            .is_some_and(|summary| summary.contains("notes.txt: document")));
    }

    #[test]
    fn run_read_media_records_missing_file_as_failed_result_item() {
        let dir = tempfile::tempdir().expect("tempdir");

        let output = run_read_media(parse_args_text("missing.txt"), dir.path())
            .expect("missing item should be represented in output");
        let result = &output["media_results"][0];

        assert_eq!(result["success"], false);
        assert_eq!(result["path"], "missing.txt");
        assert!(result["error"]
            .as_str()
            .is_some_and(|error| error.contains("media path does not exist")));
        assert!(output["summary_markdown"]
            .as_str()
            .is_some_and(|summary| summary.contains("missing.txt: failed")));
    }

    #[test]
    fn run_read_media_preserves_input_order_across_worker_threads() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("first.txt"), "first").expect("write first");
        std::fs::write(dir.path().join("second.txt"), "second").expect("write second");
        std::fs::write(dir.path().join("third.txt"), "third").expect("write third");

        let output = run_read_media(
            parse_args_text("first.txt second.txt third.txt --max-files 10"),
            dir.path(),
        )
        .expect("read_media should read all files");
        let paths = output["media_results"]
            .as_array()
            .expect("media results")
            .iter()
            .map(|item| item["path"].as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();

        assert_eq!(paths, vec!["first.txt", "second.txt", "third.txt"]);
    }
}
