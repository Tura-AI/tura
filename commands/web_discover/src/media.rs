use super::download::{download_images, download_ytdlp_media};
use super::types::{SearchResult, WebDiscoverArgs};
use serde_json::{json, Value};
use std::path::Path;

pub(super) fn media_records(
    args: &WebDiscoverArgs,
    results: &[SearchResult],
    output_dir: Option<&Path>,
    session_dir: &Path,
) -> Result<(Vec<Value>, Vec<Value>), String> {
    let Some(output_dir) = output_dir else {
        return Ok((
            results
                .iter()
                .map(|result| {
                    json!({
                        "title": result.title,
                        "url": result.url,
                        "page_url": result.page_url,
                        "file_type": args.kind,
                        "snippet": result.snippet,
                        "source": result.source,
                    })
                })
                .collect(),
            Vec::new(),
        ));
    };
    std::fs::create_dir_all(output_dir)
        .map_err(|err| format!("failed to create download_dir: {err}"))?;
    if args.kind == "image" {
        download_images(args, results, output_dir, session_dir)
    } else {
        download_ytdlp_media(args, results, output_dir, session_dir)
    }
}
