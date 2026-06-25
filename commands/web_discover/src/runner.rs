use super::asset::asset_records;
use super::files::{relative_or_display, resolve_download_dir};
use super::filter::{
    build_search_query, filter_results, parse_query_requirements, site_filters_to_image_keywords,
    strip_site_filters_from_query,
};
use super::html::{direct_webpage_url, title_from_url};
use super::media::media_records;
use super::output::summarize_records;
use super::search::{search_media_links, search_websites};
use super::types::{SearchResult, WebDiscoverArgs};
use super::website::website_records;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

pub(super) fn run_web_discover(
    args: Result<WebDiscoverArgs, String>,
    session_dir: &Path,
) -> Result<Value, String> {
    let args = args?;
    let session_dir = session_dir.to_path_buf();
    std::thread::spawn(move || run_web_discover_inner(args, &session_dir))
        .join()
        .map_err(|_| "web_discover worker thread panicked".to_string())?
}

pub(super) fn run_web_discover_inner(
    args: WebDiscoverArgs,
    session_dir: &Path,
) -> Result<Value, String> {
    let should_download = args.download_dir.is_some();
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Tura web_discover/1.0")
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()
        .map_err(|err| format!("failed to create web client: {err}"))?;
    let query_parts = parse_query_requirements(&args.query);
    let normalized_query = build_search_query(&query_parts);
    let search_query = if args.kind == "image" {
        site_filters_to_image_keywords(&normalized_query)
    } else if matches!(args.kind.as_str(), "video" | "audio") {
        strip_site_filters_from_query(&normalized_query)
    } else {
        normalized_query.clone()
    };
    let output_dir = args
        .download_dir
        .as_ref()
        .map(|_| resolve_download_dir(&args, session_dir))
        .transpose()?;
    if args.kind == "website" {
        if let Some(url) = direct_webpage_url(&args.query) {
            let result = SearchResult {
                title: title_from_url(&url),
                url,
                snippet: "Direct webpage fetch from query URL.".to_string(),
                source: "direct_url".to_string(),
                page_url: None,
            };
            let (records, downloaded_files) = website_records(
                &client,
                &[result],
                should_download,
                output_dir.as_deref(),
                session_dir,
            )?;
            let output = json!({
                "query": args.query,
                "type": args.kind,
                "normalized_query": normalized_query,
                "direct_fetch": true,
                "saved": should_download,
                "download_dir": output_dir.as_deref().map(|path| relative_or_display(path, session_dir)),
                "result_count": records.len(),
                "results": records,
                "downloaded_files": downloaded_files,
                "summary_markdown": summarize_records(&records, &downloaded_files),
            });
            return Ok(output);
        }
    }
    if args.kind == "asset" {
        let (records, downloaded_files, searched_sources) = asset_records(
            &args,
            &client,
            &search_query,
            output_dir.as_deref(),
            session_dir,
        )?;
        let output = json!({
            "query": args.query,
            "type": args.kind,
            "asset_type": args.asset_type,
            "normalized_query": normalized_query,
            "searched_sources": searched_sources,
            "saved": should_download,
            "download_dir": output_dir.as_deref().map(|path| relative_or_display(path, session_dir)),
            "result_count": records.len(),
            "results": records,
            "downloaded_files": downloaded_files,
            "summary_markdown": summarize_records(&records, &downloaded_files),
        });
        return Ok(output);
    }
    let mut results = if args.kind == "website" {
        search_websites(&client, &search_query, args.max_results)?
    } else {
        search_media_links(&client, &args.kind, &search_query, args.max_results)?
    };
    results = filter_results(results, &args)?;

    let (records, downloaded_files) = if args.kind == "website" {
        website_records(
            &client,
            &results,
            should_download,
            output_dir.as_deref(),
            session_dir,
        )?
    } else {
        media_records(&args, &results, output_dir.as_deref(), session_dir)?
    };

    let output = json!({
        "query": args.query,
        "type": args.kind,
        "normalized_query": normalized_query,
        "saved": should_download,
        "download_dir": output_dir.as_deref().map(|path| relative_or_display(path, session_dir)),
        "result_count": records.len(),
        "results": records,
        "downloaded_files": downloaded_files,
        "summary_markdown": summarize_records(&records, &downloaded_files),
    });
    Ok(output)
}
