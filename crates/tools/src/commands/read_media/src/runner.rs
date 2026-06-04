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
                    "error": err.to_string(),
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
