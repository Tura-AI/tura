use crate::commands::CommandResponse;
use crate::runtime::file_locks::Access;
use crate::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload,
};
use quick_html2md::{html_to_markdown_with_options, MarkdownOptions};
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

pub const PROMPT: &str = include_str!("prompt.md");
pub const SCHEMA: &str = include_str!("schema.json");

const DEFAULT_MAX_RESULTS: usize = 5;
const DEFAULT_MIN_SIZE: u64 = 1;
const DEFAULT_IMAGE_MIN_SIZE: u64 = 10_000;
const DEFAULT_MAX_SIZE: u64 = 80_000_000;
const MAX_WEBSITE_RESPONSE_SIZE: usize = 5 * 1024 * 1024;
const MIN_WEBSITE_TEXT_CHARS_FOR_READER: usize = 1_200;

#[derive(Clone, Debug)]
struct WebDiscoverArgs {
    kind: String,
    query: String,
    include_regex: Option<String>,
    exclude_regex: Option<String>,
    max_results: usize,
    download_dir: Option<String>,
    min_size: u64,
    max_size: u64,
    format_selector: Option<String>,
}

#[derive(Clone, Debug)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
    source: String,
    page_url: Option<String>,
}

#[derive(Clone, Debug)]
struct WebsiteContent {
    title: Option<String>,
    text: String,
    content_type: String,
    fetch_mode: String,
}

pub fn execute(command_line: &str, session_dir: &Path, _timeout_secs: u64) -> CommandResponse {
    match run_web_discover(parse_args_text(command_line), session_dir) {
        Ok(output) => CommandResponse {
            success: true,
            exit_code: 0,
            stdout: summary_text(&output),
            stderr: String::new(),
            output,
            changes: Vec::new(),
        },
        Err(err) => CommandResponse {
            success: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: err.clone(),
            output: json!({ "error": err }),
            changes: Vec::new(),
        },
    }
}

pub fn access(command_line: &str, session_dir: &Path) -> Access {
    let Ok(args) = parse_args_text(command_line) else {
        return Access::default();
    };
    if args.download_dir.is_none() {
        return Access::default();
    }
    let mut access = Access::default();
    if let Some(dir) = args.download_dir.as_deref() {
        if let Some(relative) = workspace_relative_path(dir, session_dir) {
            access
                .write_paths
                .push(web_discover_write_scope(&args, &relative));
        }
    }
    access
}

pub struct WebDiscoverHandler;

#[async_trait::async_trait]
impl ToolHandler for WebDiscoverHandler {
    fn tool_name(&self) -> &'static str {
        "web_discover"
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    async fn is_mutating(&self, _call: &ToolCall, _ctx: &ToolContext) -> bool {
        false
    }

    async fn access(&self, call: &ToolCall, ctx: &ToolContext) -> Access {
        match &call.payload {
            ToolPayload::Function { arguments } => {
                access_for_value(arguments.clone(), &ctx.session_dir)
            }
            ToolPayload::Freeform { input } => access(input, &ctx.session_dir),
        }
    }

    async fn handle(
        &self,
        call: ToolCall,
        ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError> {
        let args = match call.payload {
            ToolPayload::Function { arguments } => parse_args_value(arguments),
            ToolPayload::Freeform { input } => parse_args_text(&input),
        }
        .map_err(ToolError::RespondToModel)?;
        let output =
            run_web_discover(Ok(args), &ctx.session_dir).map_err(ToolError::RespondToModel)?;
        Ok(FunctionToolOutput::from_value(output, Some(true)))
    }
}

fn access_for_value(value: Value, session_dir: &Path) -> Access {
    let Ok(args) = parse_args_value(value) else {
        return Access::default();
    };
    if args.download_dir.is_none() {
        return Access::default();
    }
    let mut access = Access::default();
    if let Some(dir) = args.download_dir.as_deref() {
        if let Some(relative) = workspace_relative_path(dir, session_dir) {
            access
                .write_paths
                .push(web_discover_write_scope(&args, &relative));
        }
    }
    access
}

fn parse_args_text(command_line: &str) -> Result<WebDiscoverArgs, String> {
    let trimmed = command_line.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_str::<Value>(trimmed)
            .map_err(|err| format!("invalid web_discover JSON: {err}"))
            .and_then(parse_args_value);
    }
    parse_cli_args(trimmed)
}

fn parse_args_value(value: Value) -> Result<WebDiscoverArgs, String> {
    if let Some(text) = value.as_str() {
        return parse_cli_args(text);
    }
    if let Some(cli) = string_field(
        &value,
        &[
            "cli",
            "command_line",
            "commandLine",
            "input",
            "args",
            "payload",
        ],
    ) {
        return parse_cli_args(&cli);
    }
    let object = value
        .as_object()
        .ok_or_else(|| "web_discover input must be object or CLI text".to_string())?;
    let kind = object
        .get("type")
        .or_else(|| object.get("kind"))
        .or_else(|| object.get("media_type"))
        .or_else(|| object.get("mediaType"))
        .and_then(Value::as_str)
        .unwrap_or("website");
    let query = string_field(&value, &["query", "q", "search", "keywords", "keyword"])
        .unwrap_or_default()
        .trim()
        .to_string();
    args_from_parts(
        kind,
        query,
        object
            .get("include_regex")
            .or_else(|| object.get("includeRegex"))
            .or_else(|| object.get("include"))
            .and_then(Value::as_str)
            .map(str::to_string),
        object
            .get("exclude_regex")
            .or_else(|| object.get("excludeRegex"))
            .or_else(|| object.get("exclude"))
            .and_then(Value::as_str)
            .map(str::to_string),
        u64_field(&value, &["max_results", "maxResults", "limit", "n"])
            .map(|value| value.clamp(1, 20) as usize)
            .unwrap_or(DEFAULT_MAX_RESULTS),
        string_field(
            &value,
            &[
                "download_dir",
                "downloadDir",
                "output",
                "out_dir",
                "outDir",
                "dir",
            ],
        ),
        u64_field(&value, &["min_size", "minSize"]),
        u64_field(&value, &["max_size", "maxSize"]),
        string_field(
            &value,
            &[
                "format",
                "media_format",
                "mediaFormat",
                "yt_dlp_format",
                "ytDlpFormat",
            ],
        ),
    )
}

fn parse_cli_args(input: &str) -> Result<WebDiscoverArgs, String> {
    let words = split_cli_words(input);
    let mut kind = "website".to_string();
    let mut query_parts = Vec::new();
    let mut include_regex = None;
    let mut exclude_regex = None;
    let mut max_results = DEFAULT_MAX_RESULTS;
    let mut download_dir = None;
    let mut min_size = None;
    let mut max_size = None;
    let mut format_selector = None;
    let mut index = 0usize;
    while index < words.len() {
        let original_word = &words[index];
        if index == 0 && is_web_discover_command_name(original_word) {
            index += 1;
            continue;
        }
        let (word, inline_value) = split_cli_assignment(original_word);
        let take_value = |index: &mut usize| -> Result<String, String> {
            if let Some(value) = inline_value.as_ref() {
                return Ok(value.clone());
            }
            *index += 1;
            words
                .get(*index)
                .cloned()
                .ok_or_else(|| format!("{word} requires a value"))
        };
        match word.as_str() {
            "--type" | "--kind" | "--media-type" | "--media_type" | "-t" => {
                kind = take_value(&mut index)?
            }
            "--query" | "--search" | "--q" | "-q" => query_parts.push(take_value(&mut index)?),
            "--include-regex" | "--include_regex" => include_regex = Some(take_value(&mut index)?),
            "--exclude-regex" | "--exclude_regex" => exclude_regex = Some(take_value(&mut index)?),
            "--max-results" | "--max_results" | "--limit" | "-n" => {
                max_results = take_value(&mut index)?
                    .parse::<usize>()
                    .unwrap_or(DEFAULT_MAX_RESULTS)
                    .clamp(1, 20)
            }
            "--download-dir" | "--download_dir" | "-o" => {
                download_dir = Some(take_value(&mut index)?)
            }
            "--min-size" | "--min_size" => {
                min_size = Some(
                    take_value(&mut index)?
                        .parse::<u64>()
                        .unwrap_or(DEFAULT_MIN_SIZE),
                )
            }
            "--max-size" | "--max_size" => {
                max_size = Some(
                    take_value(&mut index)?
                        .parse::<u64>()
                        .unwrap_or(DEFAULT_MAX_SIZE),
                )
            }
            "--format" | "--media-format" | "--media_format" | "--yt-dlp-format"
            | "--yt_dlp_format" => format_selector = Some(take_value(&mut index)?),
            _ if query_parts.is_empty() && is_media_kind(&word) => kind = normalize_kind(&word),
            _ if !word.starts_with("--") => query_parts.push(word.clone()),
            _ => {
                if inline_value.is_none()
                    && words
                        .get(index + 1)
                        .is_some_and(|next| !next.starts_with('-'))
                {
                    index += 1;
                }
            }
        }
        index += 1;
    }
    args_from_parts(
        &kind,
        query_parts.join(" "),
        include_regex,
        exclude_regex,
        max_results,
        download_dir,
        min_size,
        max_size,
        format_selector,
    )
}

fn is_web_discover_command_name(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "web_discover" | "web-discover" | "webdiscover" | "web_search" | "web-search"
    )
}

fn is_media_kind(value: &str) -> bool {
    matches!(
        normalize_kind(value).as_str(),
        "website" | "image" | "video" | "audio"
    )
}

fn split_cli_assignment(word: &str) -> (String, Option<String>) {
    if let Some((key, value)) = word.split_once('=') {
        if key.starts_with('-') {
            return (key.to_string(), Some(value.to_string()));
        }
    }
    (word.to_string(), None)
}

#[allow(clippy::too_many_arguments)]
fn args_from_parts(
    kind: &str,
    query: String,
    include_regex: Option<String>,
    exclude_regex: Option<String>,
    max_results: usize,
    download_dir: Option<String>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    format_selector: Option<String>,
) -> Result<WebDiscoverArgs, String> {
    let kind = normalize_kind(kind);
    if !matches!(kind.as_str(), "website" | "image" | "video" | "audio") {
        return Err(format!("unsupported web_discover type: {kind}"));
    }
    if query.trim().is_empty() {
        return Err("web_discover query cannot be empty".to_string());
    }
    let default_min_size = if kind == "image" {
        DEFAULT_IMAGE_MIN_SIZE
    } else {
        DEFAULT_MIN_SIZE
    };
    Ok(WebDiscoverArgs {
        kind,
        query,
        include_regex,
        exclude_regex,
        max_results: max_results.clamp(1, 20),
        download_dir,
        min_size: min_size.unwrap_or(default_min_size),
        max_size: max_size.unwrap_or(DEFAULT_MAX_SIZE).max(1),
        format_selector: format_selector
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    })
}

fn normalize_kind(value: &str) -> String {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "web" | "page" | "pages" | "site" | "website" | "webpage" | "webpages" => {
            "website".to_string()
        }
        "img" | "images" | "photo" | "photos" => "image".to_string(),
        "videos" | "movie" | "movies" => "video".to_string(),
        "sound" | "music" => "audio".to_string(),
        other => other.to_string(),
    }
}

fn run_web_discover(
    args: Result<WebDiscoverArgs, String>,
    session_dir: &Path,
) -> Result<Value, String> {
    let args = args?;
    let session_dir = session_dir.to_path_buf();
    std::thread::spawn(move || run_web_discover_inner(args, &session_dir))
        .join()
        .map_err(|_| "web_discover worker thread panicked".to_string())?
}

fn run_web_discover_inner(args: WebDiscoverArgs, session_dir: &Path) -> Result<Value, String> {
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

fn search_websites(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    if let Some(endpoint) =
        env_value("TURA_WEB_DISCOVER_ENDPOINT").or_else(|| env_value("TURA_WEB_SEARCH_ENDPOINT"))
    {
        if let Ok(results) = search_custom_endpoint(client, &endpoint, query, limit) {
            return Ok(results);
        }
    }
    if let Some(key) = brave_search_api_key() {
        if let Ok(results) = search_brave_web_links(client, query, limit, &key) {
            return Ok(results);
        }
    }
    if let Ok(results) = search_exa_web_links(client, query, limit) {
        return Ok(results);
    }
    let endpoints = env_value("TURA_DUCKDUCKGO_SEARCH_ENDPOINT")
        .or_else(|| env_value("TURA_DUCKDUCKGO_HTML_ENDPOINT"))
        .map(|endpoint| vec![endpoint])
        .unwrap_or_else(|| {
            vec![
                "https://html.duckduckgo.com/html/".to_string(),
                "https://duckduckgo.com/html/".to_string(),
                "https://lite.duckduckgo.com/lite/".to_string(),
            ]
        });
    let mut errors = Vec::new();
    for endpoint in endpoints {
        match search_duckduckgo_html_endpoint(client, &endpoint, query, limit) {
            Ok(results) => return Ok(results),
            Err(err) => errors.push(format!("{endpoint}: {err}")),
        }
    }
    Err(format!(
        "website DuckDuckGo HTML fallback failed: {}",
        errors.join(" | ")
    ))
}

fn search_exa_web_links(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    if env_value("TURA_EXA_SEARCH_DISABLED")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
    {
        return Err("exa search disabled".to_string());
    }
    let endpoint =
        env_value("TURA_EXA_MCP_ENDPOINT").unwrap_or_else(|| "https://mcp.exa.ai/mcp".to_string());
    let context_max_characters = env_value("TURA_EXA_CONTEXT_MAX_CHARACTERS")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(8_000);
    let raw = client
        .post(endpoint)
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "web_search_exa",
                "arguments": {
                    "query": query,
                    "type": env_value("TURA_EXA_SEARCH_TYPE").unwrap_or_else(|| "auto".to_string()),
                    "numResults": limit.clamp(1, 20),
                    "livecrawl": env_value("TURA_EXA_LIVECRAWL").unwrap_or_else(|| "fallback".to_string()),
                    "contextMaxCharacters": context_max_characters,
                }
            }
        }))
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("exa web search failed: {err}"))?
        .text()
        .map_err(|err| format!("failed to read exa web search response: {err}"))?;
    parse_exa_web_results(&raw, limit)
}

fn parse_exa_web_results(raw: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
    let mut text_blocks = Vec::new();
    for line in raw.lines() {
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if let Some(items) = value
            .get("result")
            .and_then(|result| result.get("content"))
            .and_then(Value::as_array)
        {
            for item in items {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    text_blocks.push(text.to_string());
                }
            }
        }
    }
    if text_blocks.is_empty() {
        let value = serde_json::from_str::<Value>(raw)
            .map_err(|_| "exa web search returned no parseable content".to_string())?;
        if let Some(items) = value
            .get("result")
            .and_then(|result| result.get("content"))
            .and_then(Value::as_array)
        {
            for item in items {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    text_blocks.push(text.to_string());
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for block in text_blocks {
        let mut current_title: Option<String> = None;
        let mut current_url: Option<String> = None;
        let mut current_snippet = Vec::new();
        for line in block.lines().chain(std::iter::once("---")) {
            let trimmed = line.trim();
            if trimmed == "---" {
                if let Some(url) = current_url.take() {
                    if seen.insert(url.clone()) {
                        out.push(SearchResult {
                            title: current_title
                                .take()
                                .filter(|title| !title.is_empty())
                                .unwrap_or_else(|| "Exa web result".to_string()),
                            url,
                            page_url: None,
                            snippet: truncate_chars(&clean_text(&current_snippet.join(" ")), 1_000),
                            source: "exa_web".to_string(),
                        });
                        if out.len() >= limit {
                            break;
                        }
                    }
                }
                current_title = None;
                current_snippet.clear();
                continue;
            }
            if let Some(title) = trimmed.strip_prefix("Title:") {
                current_title = Some(clean_text(title));
                continue;
            }
            if let Some(url) = trimmed.strip_prefix("URL:") {
                let url = url.trim().to_string();
                if url.starts_with("http") {
                    current_url = Some(url);
                }
                continue;
            }
            if !trimmed.is_empty()
                && !trimmed.starts_with("Published:")
                && !trimmed.starts_with("Author:")
                && !trimmed.starts_with("Highlights:")
            {
                current_snippet.push(trimmed.to_string());
            }
        }
        if out.len() >= limit {
            break;
        }
    }
    if out.is_empty() {
        Err("exa web search returned no usable results".to_string())
    } else {
        Ok(out)
    }
}

fn search_duckduckgo_html_endpoint(
    client: &Client,
    endpoint: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    let html = client
        .get(endpoint)
        .query(&[("q", query)])
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("request failed: {err}"))?
        .text()
        .map_err(|err| format!("failed to read response: {err}"))?;
    let results = parse_duckduckgo_html_results(&html, limit);
    if results.is_empty() {
        Err("returned no usable results".to_string())
    } else {
        Ok(results)
    }
}

fn search_custom_endpoint(
    client: &Client,
    endpoint: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    let raw = client
        .post(endpoint)
        .json(&json!({ "query": query, "max_results": limit }))
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("custom search endpoint failed: {err}"))?
        .json::<Value>()
        .map_err(|err| format!("custom search endpoint returned invalid JSON: {err}"))?;
    Ok(raw
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(limit)
        .map(|item| SearchResult {
            title: string_field(item, &["title", "name"]).unwrap_or_else(|| "Untitled".to_string()),
            url: string_field(item, &["url", "link"]).unwrap_or_default(),
            page_url: string_field(item, &["page_url", "pageUrl", "source_url", "sourceUrl"]),
            snippet: string_field(item, &["snippet", "description", "text"]).unwrap_or_default(),
            source: "custom_endpoint".to_string(),
        })
        .filter(|item| !item.url.is_empty())
        .collect())
}

fn search_brave_web_links(
    client: &Client,
    query: &str,
    limit: usize,
    api_key: &str,
) -> Result<Vec<SearchResult>, String> {
    let endpoint = env_value("TURA_BRAVE_WEB_SEARCH_ENDPOINT")
        .or_else(|| env_value("TURA_BRAVE_SEARCH_ENDPOINT"))
        .unwrap_or_else(|| "https://api.search.brave.com/res/v1/web/search".to_string());
    let raw = client
        .get(endpoint)
        .header("Accept", "application/json")
        .header("X-Subscription-Token", api_key)
        .query(&[
            ("q", query),
            ("count", &limit.clamp(1, 20).to_string()),
            ("safesearch", "moderate"),
        ])
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("brave web search failed: {err}"))?
        .json::<Value>()
        .map_err(|err| format!("brave web search returned invalid JSON: {err}"))?;
    let results_array = raw
        .get("web")
        .and_then(|web| web.get("results"))
        .or_else(|| raw.get("results"))
        .and_then(Value::as_array);
    let results = results_array
        .into_iter()
        .flatten()
        .take(limit)
        .filter_map(|item| {
            let url = string_field(item, &["url", "link"])?;
            if !url.starts_with("http") {
                return None;
            }
            Some(SearchResult {
                title: string_field(item, &["title", "name"])
                    .unwrap_or_else(|| "Brave web result".to_string()),
                url,
                page_url: None,
                snippet: string_field(item, &["description", "snippet"]).unwrap_or_default(),
                source: "brave_web".to_string(),
            })
        })
        .collect::<Vec<_>>();
    if results.is_empty() {
        Err("brave web search returned no usable results".to_string())
    } else {
        Ok(results)
    }
}

fn search_media_links(
    client: &Client,
    kind: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    if kind == "image" {
        let urls = direct_webpage_urls(query);
        if !urls.is_empty() {
            return Ok(direct_media_results(kind, urls));
        }
        return search_image_links(client, query, limit);
    }
    let urls = direct_webpage_urls(query);
    if !urls.is_empty() {
        return Ok(direct_media_results(kind, urls));
    }
    search_ytdlp_links(kind, query, limit)
}

fn direct_media_results(kind: &str, urls: Vec<String>) -> Vec<SearchResult> {
    urls.into_iter()
        .map(|url| SearchResult {
            title: title_from_url(&url),
            url,
            page_url: None,
            snippet: format!("Direct {kind} URL from query."),
            source: format!("direct_{kind}_url"),
        })
        .collect()
}

fn search_image_links(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    if env_value("TURA_IMAGE_SEARCH_ENDPOINT").is_none() {
        if let Some(key) = brave_search_api_key() {
            if let Ok(results) = search_brave_image_links(client, query, limit, &key) {
                return Ok(results);
            }
        }
    }
    if env_value("TURA_IMAGE_SEARCH_ENDPOINT").is_none() {
        if let Ok(results) = search_exa_image_links(client, query, limit) {
            return Ok(results);
        }
    }
    if env_value("TURA_IMAGE_SEARCH_ENDPOINT").is_none() {
        return search_duckduckgo_image_links(client, query, limit);
    }
    let endpoint = env_value("TURA_IMAGE_SEARCH_ENDPOINT")
        .unwrap_or_else(|| "https://www.bing.com/images/search".to_string());
    let html = client
        .get(endpoint)
        .query(&[("q", query)])
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("image search failed: {err}"))?
        .text()
        .map_err(|err| format!("failed to read image search response: {err}"))?;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    if let Ok(re) =
        Regex::new(r#""murl"\s*:\s*"((?:\\.|[^"\\])*)"(?:.*?"t"\s*:\s*"((?:\\.|[^"\\])*)")?"#)
    {
        for capture in re.captures_iter(&html) {
            let url = json_unescape(capture.get(1).map(|v| v.as_str()).unwrap_or_default());
            if !url.starts_with("http") || !seen.insert(url.clone()) {
                continue;
            }
            out.push(SearchResult {
                title: capture
                    .get(2)
                    .map(|v| clean_text(&json_unescape(v.as_str())))
                    .unwrap_or_else(|| "Image result".to_string()),
                url,
                page_url: None,
                snippet: String::new(),
                source: "bing_images".to_string(),
            });
            if out.len() >= limit {
                break;
            }
        }
    }
    if out.len() < limit {
        if let Ok(re) = Regex::new(r#"mediaurl=([^&"'>\s]+)"#) {
            for capture in re.captures_iter(&html) {
                let context_start = capture.get(0).map(|m| m.start()).unwrap_or(0);
                let context_end = html.len().min(context_start + 2_500);
                let context = &html[context_start..context_end];
                let url = percent_decode(capture.get(1).map(|v| v.as_str()).unwrap_or_default());
                if !url.starts_with("http") || !seen.insert(url.clone()) {
                    continue;
                }
                let page_url = extract_bing_image_page_url(context);
                let title = extract_bing_image_title(context, page_url.as_deref(), &url);
                out.push(SearchResult {
                    title,
                    url,
                    page_url: page_url.clone(),
                    snippet: page_url.clone().unwrap_or_default(),
                    source: "bing_images_mediaurl".to_string(),
                });
                if out.len() >= limit {
                    break;
                }
            }
        }
    }
    if out.is_empty() {
        Err("image search returned no usable results".to_string())
    } else {
        Ok(out)
    }
}

fn brave_search_api_key() -> Option<String> {
    if env_value("TURA_BRAVE_SEARCH_DISABLED")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
    {
        return None;
    }
    env_value("TURA_BRAVE_SEARCH_API_KEY").or_else(|| env_value("BRAVE_API_KEY"))
}

fn search_brave_image_links(
    client: &Client,
    query: &str,
    limit: usize,
    api_key: &str,
) -> Result<Vec<SearchResult>, String> {
    let endpoint = env_value("TURA_BRAVE_IMAGE_SEARCH_ENDPOINT")
        .unwrap_or_else(|| "https://api.search.brave.com/res/v1/images/search".to_string());
    let raw = client
        .get(endpoint)
        .header("Accept", "application/json")
        .header("X-Subscription-Token", api_key)
        .query(&[
            ("q", query),
            ("count", &limit.clamp(1, 20).to_string()),
            ("safesearch", "strict"),
        ])
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("brave image search failed: {err}"))?
        .json::<Value>()
        .map_err(|err| format!("brave image search returned invalid JSON: {err}"))?;
    let results = raw
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(limit)
        .filter_map(|item| {
            let image_url = string_field_at(item, &[&["properties", "url"], &["url"]])
                .or_else(|| string_field_at(item, &[&["thumbnail", "src"]]))?;
            if !image_url.starts_with("http") {
                return None;
            }
            Some(SearchResult {
                title: string_field(item, &["title"]).unwrap_or_else(|| "Brave image".to_string()),
                url: image_url,
                page_url: string_field(item, &["source"]),
                snippet: string_field_at(item, &[&["meta_url", "hostname"]]).unwrap_or_default(),
                source: "brave_images".to_string(),
            })
        })
        .collect::<Vec<_>>();
    if results.is_empty() {
        Err("brave image search returned no usable results".to_string())
    } else {
        Ok(results)
    }
}

fn search_exa_image_links(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    let web_results = search_exa_web_links(client, query, limit.clamp(1, 10) * 2)?;
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for result in web_results {
        let Ok(html) = client
            .get(&result.url)
            .send()
            .and_then(|reply| reply.error_for_status())
            .and_then(|reply| reply.text())
        else {
            continue;
        };
        let Some(image_url) = extract_page_image_url(&html, &result.url) else {
            continue;
        };
        if !seen.insert(image_url.clone()) {
            continue;
        }
        out.push(SearchResult {
            title: result.title,
            url: image_url,
            page_url: Some(result.url),
            snippet: result.snippet,
            source: "exa_image".to_string(),
        });
        if out.len() >= limit {
            break;
        }
    }
    if out.is_empty() {
        Err("exa image search returned no usable images".to_string())
    } else {
        Ok(out)
    }
}

fn extract_page_image_url(html: &str, base_url: &str) -> Option<String> {
    for pattern in [
        r#"(?is)<meta[^>]+property=["']og:image(?::secure_url)?["'][^>]+content=["']([^"']+)["']"#,
        r#"(?is)<meta[^>]+name=["']twitter:image(?::src)?["'][^>]+content=["']([^"']+)["']"#,
        r#"(?is)<meta[^>]+content=["']([^"']+)["'][^>]+property=["']og:image(?::secure_url)?["']"#,
        r#"(?is)<meta[^>]+content=["']([^"']+)["'][^>]+name=["']twitter:image(?::src)?["']"#,
        r#"(?is)<img[^>]+src=["']([^"']+)["']"#,
    ] {
        let Ok(re) = Regex::new(pattern) else {
            continue;
        };
        for capture in re.captures_iter(html) {
            let candidate = html_unescape(
                capture
                    .get(1)
                    .map(|value| value.as_str())
                    .unwrap_or_default(),
            );
            if let Some(url) = resolve_page_url(base_url, &candidate) {
                if looks_like_image_url(&url) {
                    return Some(url);
                }
            }
        }
    }
    None
}

fn resolve_page_url(base_url: &str, candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() || trimmed.starts_with("data:") {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }
    let base = reqwest::Url::parse(base_url).ok()?;
    base.join(trimmed).ok().map(|url| url.to_string())
}

fn looks_like_image_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains(".jpg")
        || lower.contains(".jpeg")
        || lower.contains(".png")
        || lower.contains(".webp")
        || lower.contains("image")
        || lower.contains("img")
}

fn search_duckduckgo_image_links(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    if env_value("TURA_DUCKDUCKGO_IMAGE_PAGE_ENDPOINT").is_some()
        || env_value("TURA_DUCKDUCKGO_IMAGE_SEARCH_ENDPOINT").is_some()
        || env_value("TURA_DUCKDUCKGO_IMAGES_ENDPOINT").is_some()
    {
        return search_duckduckgo_image_links_from_endpoint(client, query, limit);
    }
    search_duckduckgo_image_links_with_library(query, limit)
}

fn search_duckduckgo_image_links_from_endpoint(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    let page_endpoint = env_value("TURA_DUCKDUCKGO_IMAGE_PAGE_ENDPOINT")
        .unwrap_or_else(|| "https://duckduckgo.com/".to_string());
    let page = client
        .get(page_endpoint)
        .query(&[("q", query), ("iax", "images"), ("ia", "images")])
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("duckduckgo image page failed: {err}"))?
        .text()
        .map_err(|err| format!("failed to read duckduckgo image page: {err}"))?;
    let vqd = extract_duckduckgo_vqd(&page)
        .ok_or_else(|| "duckduckgo image page did not contain a vqd token".to_string())?;
    let endpoint = env_value("TURA_DUCKDUCKGO_IMAGE_SEARCH_ENDPOINT")
        .or_else(|| env_value("TURA_DUCKDUCKGO_IMAGES_ENDPOINT"))
        .unwrap_or_else(|| "https://duckduckgo.com/i.js".to_string());
    let raw = client
        .get(endpoint)
        .query(&[
            ("q", query),
            ("vqd", &vqd),
            ("o", "json"),
            ("l", "us-en"),
            ("p", "1"),
            ("f", ",,,"),
        ])
        .header("Accept", "application/json")
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("duckduckgo image search failed: {err}"))?
        .json::<Value>()
        .map_err(|err| format!("duckduckgo image search returned invalid JSON: {err}"))?;
    let results = raw
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(limit)
        .filter_map(|item| {
            let image_url = string_field(item, &["image", "thumbnail"])?;
            if !image_url.starts_with("http") {
                return None;
            }
            let page_url = string_field(item, &["url"])
                .filter(|url| url.starts_with("http") && url != &image_url);
            Some(SearchResult {
                title: string_field(item, &["title"])
                    .or_else(|| string_field(item, &["source"]))
                    .unwrap_or_else(|| "DuckDuckGo image".to_string()),
                url: image_url,
                page_url,
                snippet: string_field(item, &["source"]).unwrap_or_default(),
                source: "duckduckgo_images".to_string(),
            })
        })
        .collect::<Vec<_>>();
    if results.is_empty() {
        Err("duckduckgo image search returned no usable results".to_string())
    } else {
        Ok(results)
    }
}

fn search_duckduckgo_image_links_with_library(
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    let script = r#"
import json
import sys

query = sys.argv[1]
limit = int(sys.argv[2])
backend = sys.argv[3]
timeout = int(sys.argv[4])

try:
    from ddgs import DDGS
except Exception:
    try:
        from duckduckgo_search import DDGS
    except Exception as exc:
        print(f"missing DuckDuckGo search package: {exc}", file=sys.stderr)
        sys.exit(42)

results = []
last_error = None
try:
    with DDGS(timeout=timeout) as ddgs:
        for item in ddgs.images(query, max_results=limit, safesearch="moderate", backend=backend):
            image = item.get("image") or item.get("thumbnail")
            if not image or not str(image).startswith("http"):
                continue
            results.append({
                "title": item.get("title") or item.get("source") or "DuckDuckGo image",
                "image": image,
                "url": item.get("url"),
                "source": item.get("source") or backend,
            })
            if len(results) >= limit:
                break
except Exception as exc:
    last_error = exc

if not results and last_error is not None:
    raise last_error

sys.stdout.buffer.write(json.dumps({"results": results}, ensure_ascii=False).encode("utf-8"))
"#;
    let output = Command::new("python")
        .arg("-c")
        .arg(script)
        .arg(query)
        .arg(limit.clamp(1, 20).to_string())
        .arg(
            env_value("TURA_DUCKDUCKGO_IMAGE_BACKEND")
                .unwrap_or_else(|| "auto".to_string())
                .trim()
                .to_string()
                .if_empty("auto"),
        )
        .arg(
            env_value("TURA_DUCKDUCKGO_SEARCH_TIMEOUT")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(30)
                .clamp(1, 120)
                .to_string(),
        )
        .output()
        .map_err(|err| format!("failed to run DuckDuckGo image library: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "DuckDuckGo image library failed: {stderr}. Install with: python -m pip install ddgs"
        ));
    }
    let raw = serde_json::from_slice::<Value>(&output.stdout)
        .map_err(|err| format!("DuckDuckGo image library returned invalid JSON: {err}"))?;
    let results = raw
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(limit)
        .filter_map(|item| {
            let image_url = string_field(item, &["image", "thumbnail"])?;
            if !image_url.starts_with("http") {
                return None;
            }
            let page_url = string_field(item, &["url"])
                .filter(|url| url.starts_with("http") && url != &image_url);
            Some(SearchResult {
                title: string_field(item, &["title"])
                    .or_else(|| string_field(item, &["source"]))
                    .unwrap_or_else(|| "DuckDuckGo image".to_string()),
                url: image_url,
                page_url,
                snippet: string_field(item, &["source"]).unwrap_or_default(),
                source: "duckduckgo_images".to_string(),
            })
        })
        .collect::<Vec<_>>();
    if results.is_empty() {
        Err("DuckDuckGo image library returned no usable results".to_string())
    } else {
        Ok(results)
    }
}

fn extract_duckduckgo_vqd(page: &str) -> Option<String> {
    [
        r#"vqd\s*=\s*['"]([^'"]+)['"]"#,
        r#""vqd"\s*:\s*"([^"]+)""#,
        r#"vqd=([^&"'\s]+)"#,
    ]
    .iter()
    .filter_map(|pattern| Regex::new(pattern).ok())
    .find_map(|re| {
        re.captures(page)
            .and_then(|capture| capture.get(1))
            .map(|value| html_unescape(&percent_decode(value.as_str())))
    })
    .filter(|value| !value.trim().is_empty())
}

fn search_ytdlp_links(kind: &str, query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
    let output = Command::new(resolve_ytdlp_command().0)
        .args(resolve_ytdlp_command().1)
        .args(["--dump-json", "--skip-download", "--flat-playlist"])
        .arg(format!("ytsearch{}:{query}", limit.clamp(1, 20)))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("failed to run yt-dlp search: {err}"))?;
    if !output.status.success() && output.stdout.is_empty() {
        return Err(format!(
            "yt-dlp search failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let mut out = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(item) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let mut url = string_field(&item, &["webpage_url", "url"]).unwrap_or_default();
        if !url.starts_with("http") && !url.is_empty() {
            url = format!("https://www.youtube.com/watch?v={url}");
        }
        if url.is_empty() {
            continue;
        }
        out.push(SearchResult {
            title: string_field(&item, &["title"]).unwrap_or_else(|| "Untitled".to_string()),
            url,
            page_url: string_field(&item, &["webpage_url"]),
            snippet: string_field(&item, &["description"]).unwrap_or_default(),
            source: format!("yt-dlp_{kind}"),
        });
        if out.len() >= limit {
            break;
        }
    }
    if out.is_empty() {
        Err("yt-dlp returned no usable results".to_string())
    } else {
        Ok(out)
    }
}

fn website_records(
    client: &Client,
    results: &[SearchResult],
    download: bool,
    output_dir: Option<&Path>,
    session_dir: &Path,
) -> Result<(Vec<Value>, Vec<Value>), String> {
    if download {
        let output_dir = output_dir
            .ok_or_else(|| "download_dir is required to save website pages".to_string())?;
        std::fs::create_dir_all(output_dir)
            .map_err(|err| format!("failed to create download_dir: {err}"))?;
    }
    let mut records = Vec::new();
    let mut downloaded = Vec::new();
    for (index, result) in results.iter().enumerate() {
        let content =
            fetch_website_content(client, &result.url).unwrap_or_else(|_| WebsiteContent {
                title: None,
                text: String::new(),
                content_type: String::new(),
                fetch_mode: "failed".to_string(),
            });
        let title = content
            .title
            .clone()
            .unwrap_or_else(|| result.title.clone());
        let text = content.text;
        if download {
            let mut record = json!({
                "title": title,
                "url": result.url,
                "snippet": result.snippet,
                "source": result.source,
                "fetch_mode": content.fetch_mode,
                "content_type": content.content_type,
            });
            let output_dir = output_dir
                .ok_or_else(|| "download_dir is required to save website pages".to_string())?;
            let filename = format!("{:02}-{}.md", index + 1, safe_filename(&title));
            let path = output_dir.join(filename);
            let md = format!("# {title}\n\nSource: {}\n\n{}\n", result.url, text);
            std::fs::write(&path, md).map_err(|err| format!("failed to write markdown: {err}"))?;
            let metadata = std::fs::metadata(&path).ok();
            let rel = relative_or_display(&path, session_dir);
            record["local_path"] = json!(rel);
            downloaded.push(json!({
                "path": relative_or_display(&path, session_dir),
                "absolute_path": path.display().to_string(),
                "name": path.file_name().and_then(|v| v.to_str()).unwrap_or_default(),
                "content_type": "text/markdown",
                "size": metadata.map(|m| m.len()).unwrap_or(0),
                "source_url": result.url,
            }));
            records.push(record);
        } else {
            records.push(json!(middle_truncate_chars(&text, 1_000)));
        }
    }
    Ok((records, downloaded))
}

fn fetch_website_content(client: &Client, url: &str) -> Result<WebsiteContent, String> {
    let primary = fetch_website_content_once(client, url, browser_user_agent(), "primary");
    match primary {
        Ok(content) if content.text.chars().count() >= reader_min_text_chars() => Ok(content),
        Ok(content) => match fetch_reader_content(client, url) {
            Ok(reader) => {
                if reader.text.chars().count() > content.text.chars().count() {
                    Ok(reader)
                } else {
                    Ok(content)
                }
            }
            Err(_) => Ok(content),
        },
        Err(primary_err) => fetch_reader_content(client, url).map_err(|reader_err| {
            format!(
                "primary website fetch failed: {primary_err}; reader fallback failed: {reader_err}"
            )
        }),
    }
}

fn fetch_website_content_once(
    client: &Client,
    url: &str,
    user_agent: &str,
    fetch_mode: &str,
) -> Result<WebsiteContent, String> {
    let mut response = client
        .get(url)
        .header("User-Agent", user_agent)
        .header(
            "Accept",
            "text/markdown;q=1.0, text/x-markdown;q=0.9, text/plain;q=0.8, text/html;q=0.7, */*;q=0.1",
        )
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .map_err(|err| format!("request failed: {err}"))?;
    if response.status().as_u16() == 403
        && response
            .headers()
            .get("cf-mitigated")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.eq_ignore_ascii_case("challenge"))
            .unwrap_or(false)
    {
        response = client
            .get(url)
            .header("User-Agent", "Tura web_discover/1.0")
            .header(
                "Accept",
                "text/markdown;q=1.0, text/x-markdown;q=0.9, text/plain;q=0.8, text/html;q=0.7, */*;q=0.1",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .map_err(|err| format!("retry request failed: {err}"))?;
    }
    let status = response.status();
    if !status.is_success() {
        return Err(format!("request failed with status code: {status}"));
    }
    response_to_website_content(response, fetch_mode)
}

fn fetch_reader_content(client: &Client, url: &str) -> Result<WebsiteContent, String> {
    if env_value("TURA_WEB_READER_DISABLED")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
    {
        return Err("web reader fallback disabled".to_string());
    }
    let endpoint = env_value("TURA_WEB_READER_ENDPOINT")
        .unwrap_or_else(|| "https://r.jina.ai/http://".to_string());
    let reader_url = if endpoint.contains("{url}") {
        endpoint.replace("{url}", url)
    } else {
        format!("{endpoint}{url}")
    };
    let response = client
        .get(&reader_url)
        .header("User-Agent", browser_user_agent())
        .header("Accept", "text/markdown, text/plain;q=0.9, */*;q=0.1")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .map_err(|err| format!("reader request failed: {err}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("reader request failed with status code: {status}"));
    }
    response_to_website_content(response, "reader_fallback")
}

fn response_to_website_content(
    response: reqwest::blocking::Response,
    fetch_mode: &str,
) -> Result<WebsiteContent, String> {
    let base_url = response.url().to_string();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    if let Some(length) = response.content_length() {
        if length as usize > MAX_WEBSITE_RESPONSE_SIZE {
            return Err("response too large (exceeds 5MB limit)".to_string());
        }
    }
    let bytes = response
        .bytes()
        .map_err(|err| format!("failed to read response: {err}"))?;
    if bytes.len() > MAX_WEBSITE_RESPONSE_SIZE {
        return Err("response too large (exceeds 5MB limit)".to_string());
    }
    let raw = String::from_utf8_lossy(&bytes).to_string();
    let title = if content_type.to_ascii_lowercase().contains("html") {
        extract_title(&raw)
    } else {
        extract_reader_title(&raw)
    };
    let text = if content_type.to_ascii_lowercase().contains("html") || raw.contains('<') {
        html_to_markdown_text(&raw, &base_url)
    } else {
        raw.trim().to_string()
    };
    Ok(WebsiteContent {
        title,
        text,
        content_type,
        fetch_mode: fetch_mode.to_string(),
    })
}

fn browser_user_agent() -> &'static str {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36"
}

fn reader_min_text_chars() -> usize {
    env_value("TURA_WEB_READER_MIN_TEXT_CHARS")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(MIN_WEBSITE_TEXT_CHARS_FOR_READER)
}

fn media_records(
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

fn download_images(
    args: &WebDiscoverArgs,
    results: &[SearchResult],
    output_dir: &Path,
    session_dir: &Path,
) -> Result<(Vec<Value>, Vec<Value>), String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent("Tura web_discover/1.0")
        .build()
        .map_err(|err| err.to_string())?;
    let mut handles = Vec::new();
    for (index, result) in results.iter().cloned().enumerate() {
        let client = client.clone();
        let args = args.clone();
        let output_dir = output_dir.to_path_buf();
        let session_dir = session_dir.to_path_buf();
        handles.push(std::thread::spawn(
            move || -> Result<Option<(usize, Value, Value)>, String> {
                let Ok(bytes) = client
                    .get(&result.url)
                    .send()
                    .and_then(|reply| reply.error_for_status())
                    .and_then(|reply| reply.bytes())
                else {
                    return Ok(None);
                };
                let size = bytes.len() as u64;
                if size < args.min_size || size > args.max_size {
                    return Ok(None);
                }
                let ext = extension_from_url(&result.url).unwrap_or("jpg");
                let base_name = format!("{:02}-{}", index + 1, safe_filename(&result.title));
                let path = write_unique_download(&output_dir, &base_name, ext, bytes.as_ref())?;
                let item = downloaded_file_value(
                    &path,
                    &session_dir,
                    &result.url,
                    result.page_url.as_deref(),
                    &args.kind,
                );
                let record = json!({
                    "title": result.title,
                    "url": result.url,
                    "page_url": result.page_url,
                    "file_type": args.kind,
                    "local_path": item["path"],
                    "size": item["size"],
                    "source": result.source,
                });
                Ok(Some((index, record, item)))
            },
        ));
    }

    let mut indexed = Vec::new();
    for handle in handles {
        let result = handle
            .join()
            .map_err(|_| "image download worker panicked".to_string())??;
        if let Some(item) = result {
            indexed.push(item);
        }
    }
    indexed.sort_by_key(|(index, _, _)| *index);
    let records = indexed
        .iter()
        .map(|(_, record, _)| record.clone())
        .collect();
    let downloaded = indexed.into_iter().map(|(_, _, item)| item).collect();
    Ok((records, downloaded))
}

fn download_ytdlp_media(
    args: &WebDiscoverArgs,
    results: &[SearchResult],
    output_dir: &Path,
    session_dir: &Path,
) -> Result<(Vec<Value>, Vec<Value>), String> {
    let mut handles = Vec::new();
    for (index, result) in results.iter().cloned().enumerate() {
        let args = args.clone();
        let output_dir = output_dir.to_path_buf();
        let session_dir = session_dir.to_path_buf();
        handles.push(std::thread::spawn(
            move || -> Result<Option<(usize, Value, Value)>, String> {
                let format_arg = args
                    .format_selector
                    .as_deref()
                    .unwrap_or_else(|| default_ytdlp_format(&args.kind));
                let temp_dir = output_dir.join(format!(
                    ".tura-ytdlp-{}-{}-{}",
                    std::process::id(),
                    index,
                    stable_hash(&result.url)
                ));
                std::fs::create_dir_all(&temp_dir)
                    .map_err(|err| format!("failed to create yt-dlp temp dir: {err}"))?;
                let output_template = temp_dir.join("%(title).80s-%(id)s.%(ext)s");
                let command_parts = resolve_ytdlp_command();
                let mut command = Command::new(command_parts.0);
                command
                    .args(command_parts.1)
                    .args([
                        "-f",
                        format_arg,
                        "--no-playlist",
                        "--no-progress",
                        "--max-filesize",
                    ])
                    .arg(args.max_size.to_string())
                    .arg("-o")
                    .arg(&output_template)
                    .arg(&result.url);
                let output = command.output().map_err(|err| {
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    format!("failed to run yt-dlp download: {err}")
                })?;
                if !output.status.success() {
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    return Ok(None);
                }
                let mut new_files = snapshot_files(&temp_dir)
                    .into_iter()
                    .filter(|path| {
                        std::fs::metadata(path)
                            .map(|m| m.len() >= args.min_size && m.len() <= args.max_size)
                            .unwrap_or(false)
                    })
                    .collect::<Vec<_>>();
                new_files.sort_by_key(|path| ytdlp_download_candidate_rank(path, &args.kind));
                let Some(path) = new_files.first() else {
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    return Ok(None);
                };
                let ext = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("bin");
                let base_name = format!("{:02}-{}", index + 1, safe_filename(&result.title));
                let path = move_unique_download(path, &output_dir, &base_name, ext)?;
                let _ = std::fs::remove_dir_all(&temp_dir);
                let item = downloaded_file_value(
                    &path,
                    &session_dir,
                    &result.url,
                    result.page_url.as_deref(),
                    &args.kind,
                );
                let record = json!({
                    "title": result.title,
                    "url": result.url,
                    "page_url": result.page_url,
                    "file_type": args.kind,
                    "local_path": item["path"],
                    "size": item["size"],
                    "source": result.source,
                });
                Ok(Some((index, record, item)))
            },
        ));
    }

    let mut indexed = Vec::new();
    for handle in handles {
        let result = handle
            .join()
            .map_err(|_| "yt-dlp download worker panicked".to_string())??;
        if let Some(item) = result {
            indexed.push(item);
        }
    }
    indexed.sort_by_key(|(index, _, _)| *index);
    let records = indexed
        .iter()
        .map(|(_, record, _)| record.clone())
        .collect();
    let downloaded = indexed.into_iter().map(|(_, _, item)| item).collect();
    Ok((records, downloaded))
}

fn default_ytdlp_format(kind: &str) -> &'static str {
    if kind == "audio" {
        "bestaudio/best"
    } else {
        "best[height<=540][ext=mp4]/best[height<=540]/best"
    }
}

fn ytdlp_download_candidate_rank(path: &Path, kind: &str) -> (u8, std::cmp::Reverse<u64>) {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let rank = if kind == "audio" {
        match extension.as_str() {
            "mp3" | "m4a" | "aac" | "opus" | "ogg" | "webm" | "flac" | "wav" => 0,
            _ => 1,
        }
    } else {
        match extension.as_str() {
            "mp4" | "mkv" | "mov" => 0,
            "webm" => 1,
            "mp3" | "m4a" | "aac" | "opus" | "ogg" | "flac" | "wav" => 2,
            _ => 3,
        }
    };
    (rank, std::cmp::Reverse(size))
}

fn resolve_ytdlp_command() -> (&'static str, Vec<&'static str>) {
    if find_on_path("yt-dlp").is_some() {
        ("yt-dlp", Vec::new())
    } else {
        ("python", vec!["-m", "yt_dlp"])
    }
}

fn parse_query_requirements(query: &str) -> (Vec<String>, Vec<String>) {
    let mut positive = Vec::new();
    let mut negative = Vec::new();
    let Ok(negative_re) =
        Regex::new(r"(?i)^(-|not\s+|exclude\s+|without\s+|反向[:：]?\s*|排除[:：]?\s*|不要\s*)")
    else {
        return (vec![query.to_string()], Vec::new());
    };
    for part in query.split([',', '，']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if negative_re.is_match(part) {
            let cleaned = negative_re.replace(part, "").trim().to_string();
            if !cleaned.is_empty() {
                negative.push(cleaned);
            }
        } else {
            positive.push(part.to_string());
        }
    }
    if positive.is_empty() {
        positive.push(query.to_string());
    }
    (positive, negative)
}

fn build_search_query(parts: &(Vec<String>, Vec<String>)) -> String {
    let mut query = parts.0.join(" ");
    for term in &parts.1 {
        query.push(' ');
        query.push('-');
        query.push_str(term);
    }
    query.trim().to_string()
}

fn filter_results(
    results: Vec<SearchResult>,
    args: &WebDiscoverArgs,
) -> Result<Vec<SearchResult>, String> {
    let include = args
        .include_regex
        .as_deref()
        .map(Regex::new)
        .transpose()
        .map_err(|err| format!("invalid include_regex: {err}"))?;
    let exclude = args
        .exclude_regex
        .as_deref()
        .map(Regex::new)
        .transpose()
        .map_err(|err| format!("invalid exclude_regex: {err}"))?;
    Ok(results
        .into_iter()
        .filter(|result| {
            let haystack = format!("{}\n{}\n{}", result.title, result.url, result.snippet);
            let strict_haystack = format!(
                "{}\n{}\n{}",
                result.url,
                result.page_url.as_deref().unwrap_or_default(),
                result.snippet
            );
            include
                .as_ref()
                .map(|re| {
                    if result.source.starts_with("bing_images") {
                        re.is_match(&strict_haystack)
                    } else {
                        re.is_match(&haystack)
                    }
                })
                .unwrap_or(true)
                && !exclude
                    .as_ref()
                    .map(|re| re.is_match(&haystack))
                    .unwrap_or(false)
                && (args.kind != "website" || site_filters_match(&args.query, result))
        })
        .take(args.max_results)
        .collect())
}

fn site_filters_to_image_keywords(query: &str) -> String {
    let Ok(re) = Regex::new(r"(?i)\bsite:\s*([^\s,，]+)") else {
        return query.to_string();
    };
    re.replace_all(query, ", $1, ")
        .split(|ch: char| ch.is_whitespace() || ch == ',' || ch == '，')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_site_filters_from_query(query: &str) -> String {
    let Ok(re) = Regex::new(r"(?i)\bsite:\s*[^\s,，]+") else {
        return query.to_string();
    };
    re.replace_all(query, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn site_filters_match(query: &str, result: &SearchResult) -> bool {
    let sites = site_filters(query);
    if sites.is_empty() {
        return true;
    }
    sites.into_iter().any(|site| {
        url_host_matches(&result.url, &site)
            || result
                .page_url
                .as_deref()
                .map(|url| url_host_matches(url, &site))
                .unwrap_or(false)
    })
}

fn site_filters(query: &str) -> Vec<String> {
    let Ok(re) = Regex::new(r"(?i)\bsite:\s*([^\s,，]+)") else {
        return Vec::new();
    };
    re.captures_iter(query)
        .filter_map(|capture| capture.get(1).map(|value| value.as_str()))
        .map(|site| {
            site.trim()
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .trim_start_matches("www.")
                .trim_matches('/')
                .to_ascii_lowercase()
        })
        .filter(|site| !site.is_empty())
        .collect()
}

fn url_host_matches(url: &str, site: &str) -> bool {
    let Some(host) = url_host(url) else {
        return false;
    };
    host == site || host.ends_with(&format!(".{site}"))
}

fn url_host(url: &str) -> Option<String> {
    let rest = url
        .trim()
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next()?.trim();
    if host.is_empty() {
        return None;
    }
    Some(
        host.split('@')
            .next_back()
            .unwrap_or(host)
            .split(':')
            .next()
            .unwrap_or(host)
            .trim_start_matches("www.")
            .to_ascii_lowercase(),
    )
}

fn extract_bing_image_page_url(context: &str) -> Option<String> {
    if let Some(url) = Regex::new(r#"(?i)[?&](?:purl|pageurl)=([^&"'>\s]+)"#)
        .ok()
        .and_then(|re| {
            re.captures(context).and_then(|capture| {
                capture
                    .get(1)
                    .map(|value| percent_decode(&html_unescape(value.as_str())))
            })
        })
        .filter(|url| url.starts_with("http"))
    {
        return Some(url);
    }
    let href_re = Regex::new(r#"(?is)<a[^>]+href="([^"]+)""#).ok()?;
    for capture in href_re.captures_iter(context) {
        let raw = html_unescape(
            capture
                .get(1)
                .map(|value| value.as_str())
                .unwrap_or_default(),
        );
        let url = percent_decode(&raw);
        if !url.starts_with("http") {
            continue;
        }
        let host = url_host(&url).unwrap_or_default();
        if host.contains("bing.com")
            || host.contains("bing.net")
            || host == "th.bing.com"
            || extension_from_url(&url).is_some()
        {
            continue;
        }
        return Some(url);
    }
    None
}

fn extract_bing_image_title(context: &str, page_url: Option<&str>, media_url: &str) -> String {
    let alt = Regex::new(r#"(?is)\balt="([^"]+)""#)
        .ok()
        .and_then(|re| re.captures(context))
        .and_then(|capture| capture.get(1).map(|value| clean_text(value.as_str())))
        .filter(|title| {
            let lower = title.to_ascii_lowercase();
            !lower.contains("image result")
                && !lower.contains("résultat d")
                && !lower.contains("recherche images")
        });
    if let Some(title) = alt {
        return title;
    }
    if let Some(host) = page_url.and_then(url_host) {
        return host;
    }
    url_host(media_url).unwrap_or_else(|| "Image result".to_string())
}

fn parse_duckduckgo_html_results(html: &str, limit: usize) -> Vec<SearchResult> {
    let Ok(link_re) =
        Regex::new(r#"(?s)<a[^>]*class="[^"]*result__a[^"]*"[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#)
    else {
        return Vec::new();
    };
    let Ok(snippet_re) =
        Regex::new(r#"(?s)<a[^>]*class="[^"]*result__snippet[^"]*"[^>]*>(.*?)</a>"#)
    else {
        return Vec::new();
    };
    let snippets = snippet_re
        .captures_iter(html)
        .filter_map(|capture| capture.get(1).map(|value| clean_text(value.as_str())))
        .collect::<Vec<_>>();
    link_re
        .captures_iter(html)
        .take(limit)
        .enumerate()
        .filter_map(|(index, capture)| {
            let url = normalize_duckduckgo_url(capture.get(1)?.as_str());
            let title = clean_text(capture.get(2)?.as_str());
            Some(SearchResult {
                title,
                url,
                page_url: None,
                snippet: snippets.get(index).cloned().unwrap_or_default(),
                source: "duckduckgo_html".to_string(),
            })
        })
        .filter(|item| !item.title.is_empty() && item.url.starts_with("http"))
        .collect()
}

fn normalize_duckduckgo_url(url: &str) -> String {
    if let Some(encoded) = url
        .split("uddg=")
        .nth(1)
        .and_then(|rest| rest.split('&').next())
    {
        return percent_decode(encoded);
    }
    html_unescape(url)
}

fn html_to_markdown_text(html: &str, base_url: &str) -> String {
    let media_links = extract_media_links(html, base_url);
    let cleaned = remove_html_noise(html);
    let mut markdown = normalize_markdown(&html_to_markdown_with_options(
        &cleaned,
        &MarkdownOptions::new()
            .include_images(true)
            .preserve_tables(true)
            .preserve_code(true)
            .base_url(base_url),
    ));
    if !media_links.is_empty() {
        let section = media_links
            .iter()
            .map(|url| format!("- <{url}>"))
            .collect::<Vec<_>>()
            .join("\n");
        if markdown.is_empty() {
            markdown = format!("## Media links\n\n{section}");
        } else {
            markdown.push_str("\n\n## Media links\n\n");
            markdown.push_str(&section);
        }
    }
    markdown
}

fn remove_html_noise(html: &str) -> String {
    let mut text = html.to_string();
    for pattern in [
        "(?is)<script[^>]*>.*?</script>",
        "(?is)<style[^>]*>.*?</style>",
    ] {
        if let Ok(re) = Regex::new(pattern) {
            text = re.replace_all(&text, " ").to_string();
        }
    }
    text
}

fn extract_media_links(html: &str, base_url: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |candidate: &str| {
        if let Some(url) = normalize_media_url(candidate, base_url) {
            if seen.insert(url.clone()) {
                out.push(url);
            }
        }
    };

    if let Ok(attr_re) = Regex::new(
        r#"(?is)\b(?:src|srcset|data-src|data-original|poster|href|content)\s*=\s*['"]([^'"]+)['"]"#,
    ) {
        for capture in attr_re.captures_iter(html) {
            if let Some(value) = capture.get(1) {
                for part in value.as_str().split(',') {
                    let candidate = part.split_whitespace().next().unwrap_or_default();
                    push(candidate);
                }
            }
        }
    }

    if let Ok(url_re) = Regex::new(
        r#"https?:\\?/\\?/[^"'\s<>)]+?\.(?:jpg|jpeg|png|webp|gif|avif|mp4|webm|mov|m4v|mp3|wav|ogg|m4a)(?:[?#][^"'\s<>)\\]*)?"#,
    ) {
        for capture in url_re.find_iter(html) {
            push(capture.as_str());
        }
    }

    out
}

fn normalize_media_url(candidate: &str, base_url: &str) -> Option<String> {
    let mut value = html_unescape(candidate).trim().to_string();
    if value.is_empty() || value.starts_with("data:") || value.starts_with("blob:") {
        return None;
    }
    value = json_unescape(&value)
        .replace("\\u002F", "/")
        .replace("\\/", "/");
    let lower = value.to_ascii_lowercase();
    if !matches!(
        lower
            .split(['?', '#'])
            .next()
            .and_then(|path| path.rsplit('.').next()),
        Some(
            "jpg"
                | "jpeg"
                | "png"
                | "webp"
                | "gif"
                | "avif"
                | "mp4"
                | "webm"
                | "mov"
                | "m4v"
                | "mp3"
                | "wav"
                | "ogg"
                | "m4a"
        )
    ) {
        return None;
    }
    if value.starts_with("//") {
        value = format!("https:{value}");
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return Some(value);
    }
    reqwest::Url::parse(base_url)
        .ok()
        .and_then(|base| base.join(&value).ok())
        .map(|url| url.to_string())
}

fn normalize_markdown(value: &str) -> String {
    let unescaped = html_unescape(value);
    let mut lines = Vec::new();
    let mut blank_count = 0usize;
    for line in unescaped.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 1 && !lines.is_empty() {
                lines.push(String::new());
            }
            continue;
        }
        blank_count = 0;
        lines.push(trimmed.to_string());
    }
    lines.join("\n").trim().to_string()
}

fn extract_title(html: &str) -> Option<String> {
    Regex::new("(?is)<title[^>]*>(.*?)</title>")
        .ok()?
        .captures(html)
        .and_then(|capture| capture.get(1))
        .map(|value| clean_text(value.as_str()))
        .filter(|value| !value.is_empty())
}

fn extract_reader_title(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("Title:")
                .map(clean_text)
                .filter(|value| !value.is_empty())
        })
        .or_else(|| {
            text.lines()
                .find_map(|line| line.trim().strip_prefix("# ").map(clean_text))
                .filter(|value| !value.is_empty())
        })
}

fn direct_webpage_url(query: &str) -> Option<String> {
    let mut urls = direct_webpage_urls(query);
    if urls.len() == 1 {
        urls.pop()
    } else {
        None
    }
}

fn direct_webpage_urls(query: &str) -> Vec<String> {
    let words = split_cli_words(query);
    if words.is_empty() {
        return Vec::new();
    }
    let mut urls = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for word in words {
        let trimmed = word.trim().trim_matches(['"', '\'', ',', ';']);
        let Ok(parsed) = reqwest::Url::parse(trimmed) else {
            return Vec::new();
        };
        if !matches!(parsed.scheme(), "http" | "https") {
            return Vec::new();
        }
        let url = parsed.to_string();
        if seen.insert(url.clone()) {
            urls.push(url);
        }
    }
    urls
}

fn title_from_url(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_string))
                .filter(|segment| !segment.is_empty())
                .or_else(|| parsed.host_str().map(str::to_string))
        })
        .unwrap_or_else(|| "Webpage".to_string())
}

fn summarize_records(records: &[Value], downloaded: &[Value]) -> String {
    let mut lines = Vec::new();
    for (index, record) in records.iter().enumerate().take(10) {
        if let Some(text) = record.as_str() {
            lines.push(format!(
                "{}. {}",
                index + 1,
                truncate_chars(&text.replace('\n', " "), 220)
            ));
            continue;
        }
        let title = record
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled");
        let url = record
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let path = record
            .get("local_path")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if path.is_empty() {
            lines.push(format!("{}. [{}]({})", index + 1, title, url));
        } else {
            lines.push(format!("{}. [{}]({}) -> {}", index + 1, title, url, path));
        }
    }
    if !downloaded.is_empty() {
        lines.push("downloaded:".to_string());
        for item in downloaded {
            let path = item.get("path").and_then(Value::as_str).unwrap_or_default();
            let size = item.get("size").and_then(Value::as_u64).unwrap_or(0);
            lines.push(format!("- {path} ({size} bytes)"));
        }
    }
    lines.join("\n")
}

fn summary_text(value: &Value) -> String {
    value
        .get("summary_markdown")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn downloaded_file_value(
    path: &Path,
    session_dir: &Path,
    source_url: &str,
    source_page_url: Option<&str>,
    kind: &str,
) -> Value {
    let metadata = std::fs::metadata(path).ok();
    json!({
        "path": relative_or_display(path, session_dir),
        "absolute_path": path.display().to_string(),
        "name": path.file_name().and_then(|v| v.to_str()).unwrap_or_default(),
        "url": source_url,
        "source_page_url": source_page_url,
        "file_type": kind,
        "content_type": content_type_for_path(path, kind),
        "size": metadata.map(|m| m.len()).unwrap_or(0),
    })
}

fn resolve_download_dir(args: &WebDiscoverArgs, session_dir: &Path) -> Result<PathBuf, String> {
    let default_dir = match args.kind.as_str() {
        "website" => "web",
        "image" => "media/image",
        "video" => "media/video",
        "audio" => "media/audio",
        _ => "media",
    };
    let raw = args.download_dir.as_deref().unwrap_or(default_dir);
    let path = PathBuf::from(raw);
    let resolved = if path.is_absolute() {
        path
    } else {
        session_dir.join(path)
    };
    Ok(resolved)
}

fn relative_or_display(path: &Path, session_dir: &Path) -> String {
    path.strip_prefix(session_dir)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn workspace_relative_path(path: &str, session_dir: &Path) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    let resolved = if path.is_absolute() {
        path
    } else {
        session_dir.join(path)
    };
    resolved
        .strip_prefix(session_dir)
        .ok()
        .map(Path::to_path_buf)
}

fn web_discover_write_scope(args: &WebDiscoverArgs, relative_dir: &Path) -> String {
    format!(
        "{}/.web_discover-{}-{}",
        relative_dir.display(),
        args.kind,
        stable_hash(&args.query)
    )
}

fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn write_unique_download(
    output_dir: &Path,
    base_name: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    for copy in 0..1000 {
        let suffix = if copy == 0 {
            String::new()
        } else {
            format!("-{copy}")
        };
        let path = output_dir.join(format!("{base_name}{suffix}.{extension}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(bytes)
                    .map_err(|err| format!("failed to write image: {err}"))?;
                return Ok(path);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(format!("failed to write image: {err}")),
        }
    }
    Err(format!(
        "failed to choose unique download name for {base_name}.{extension}"
    ))
}

fn move_unique_download(
    source: &Path,
    output_dir: &Path,
    base_name: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    for copy in 0..1000 {
        let suffix = if copy == 0 {
            String::new()
        } else {
            format!("-{copy}")
        };
        let path = output_dir.join(format!("{base_name}{suffix}.{extension}"));
        match std::fs::rename(source, &path) {
            Ok(()) => return Ok(path),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(format!("failed to move downloaded media: {err}")),
        }
    }
    Err(format!(
        "failed to choose unique download name for {base_name}.{extension}"
    ))
}

fn split_cli_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in input.chars() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(ch),
            (None, c) if c.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn string_field_at(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        let mut current = value;
        for key in *path {
            current = current.get(*key)?;
        }
        current
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

fn u64_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
    })
}

fn clean_text(value: &str) -> String {
    let without_tags = Regex::new("(?is)<[^>]+>")
        .ok()
        .map(|re| re.replace_all(value, " ").to_string())
        .unwrap_or_else(|| value.to_string());
    html_unescape(&without_tags)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

fn json_unescape(value: &str) -> String {
    serde_json::from_str::<String>(&format!("\"{value}\""))
        .unwrap_or_else(|_| value.replace("\\/", "/"))
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    index += 3;
                    continue;
                }
            }
        }
        out.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn safe_filename(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    cleaned
        .split('-')
        .filter(|part| !part.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(80)
        .collect::<String>()
        .if_empty("result")
}

trait EmptyDefault {
    fn if_empty(self, fallback: &str) -> String;
}

impl EmptyDefault for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

fn extension_from_url(url: &str) -> Option<&'static str> {
    let lower = url.to_ascii_lowercase();
    if lower.contains(".png") {
        Some("png")
    } else if lower.contains(".webp") {
        Some("webp")
    } else if lower.contains(".jpeg") || lower.contains(".jpg") {
        Some("jpg")
    } else {
        None
    }
}

fn content_type_for_path(path: &Path, kind: &str) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        _ if kind == "website" => "text/markdown",
        _ => "application/octet-stream",
    }
}

fn snapshot_files(path: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect()
}

fn find_on_path(exe: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(if cfg!(windows) {
            format!("{exe}.exe")
        } else {
            exe.to_string()
        });
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            tura_llm_rust::TuraConfig::default()
                .get(name)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect::<String>()
}

fn middle_truncate_chars(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    let marker = "\n...[truncated]...\n";
    let marker_len = marker.chars().count();
    if max_chars <= marker_len {
        return text.chars().take(max_chars).collect();
    }
    let keep = max_chars - marker_len;
    let head = keep / 2;
    let tail = keep - head;
    let start = text.chars().take(head).collect::<String>();
    let end = text
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}{marker}{end}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_webpage_url_accepts_only_single_http_url() {
        assert_eq!(
            direct_webpage_url("https://cloud.google.com/vertex-ai/generative-ai/docs/image")
                .as_deref(),
            Some("https://cloud.google.com/vertex-ai/generative-ai/docs/image")
        );
        assert_eq!(
            direct_webpage_url("\"https://example.com/docs\"").as_deref(),
            Some("https://example.com/docs")
        );
        assert!(direct_webpage_url("site:cloud.google.com Vertex AI docs").is_none());
        assert!(direct_webpage_url("https://example.com/docs extra words").is_none());
        assert!(
            direct_webpage_url("https://example.com/a.jpg https://example.com/b.jpg").is_none()
        );
        assert!(direct_webpage_url("ftp://example.com/file").is_none());
        assert_eq!(
            direct_webpage_urls("https://example.com/a.jpg https://example.com/b.jpg"),
            vec!["https://example.com/a.jpg", "https://example.com/b.jpg"]
        );
    }

    #[test]
    fn direct_media_urls_bypass_search() {
        let client = Client::builder().build().expect("client");
        let image = search_media_links(
            &client,
            "image",
            "https://officialsite.cds-jp.online/prod/profile_member/105/158/2c38bd5497b94e38aba150b784a7de87.webp",
            5,
        )
        .expect("direct image url");
        assert_eq!(image.len(), 1);
        assert_eq!(
            image[0].url,
            "https://officialsite.cds-jp.online/prod/profile_member/105/158/2c38bd5497b94e38aba150b784a7de87.webp"
        );
        assert_eq!(image[0].source, "direct_image_url");
        let images = search_media_links(
            &client,
            "image",
            "https://example.com/a.jpg https://example.com/b.webp",
            5,
        )
        .expect("direct image urls");
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].url, "https://example.com/a.jpg");
        assert_eq!(images[1].url, "https://example.com/b.webp");

        let video = search_media_links(
            &client,
            "video",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            5,
        )
        .expect("direct video url");
        assert_eq!(video.len(), 1);
        assert_eq!(video[0].url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        assert_eq!(video[0].source, "direct_video_url");

        let audio = search_media_links(
            &client,
            "audio",
            "\"https://www.bilibili.com/video/BV1xx411c7mD\"",
            5,
        )
        .expect("direct audio url");
        assert_eq!(audio.len(), 1);
        assert_eq!(audio[0].url, "https://www.bilibili.com/video/BV1xx411c7mD");
        assert_eq!(audio[0].source, "direct_audio_url");
    }

    #[test]
    fn parse_video_format_selector_for_ytdlp_downloads() {
        let args = parse_args_text(
            r#"video "product demo" --download-dir media/video --format "bestvideo[height<=1080]+bestaudio/best""#,
        )
        .expect("parse web_discover args");

        assert_eq!(args.kind, "video");
        assert_eq!(
            args.format_selector.as_deref(),
            Some("bestvideo[height<=1080]+bestaudio/best")
        );
    }

    #[test]
    fn default_ytdlp_formats_prefer_best_available_media() {
        assert_eq!(default_ytdlp_format("audio"), "bestaudio/best");
        assert_eq!(
            default_ytdlp_format("video"),
            "best[height<=540][ext=mp4]/best[height<=540]/best"
        );
    }

    #[test]
    fn video_download_candidates_prefer_video_files_over_larger_audio() {
        let mut paths = [
            PathBuf::from("clip.f251.webm"),
            PathBuf::from("clip.f134.mp4"),
            PathBuf::from("clip.f251.m4a"),
        ];
        paths.sort_by_key(|path| ytdlp_download_candidate_rank(path, "video"));

        assert_eq!(paths[0], PathBuf::from("clip.f134.mp4"));
    }

    #[tokio::test]
    async fn download_web_discover_does_not_take_global_mutation_gate() {
        let handler = WebDiscoverHandler;
        let call = ToolCall {
            tool_name: "web_discover".to_string(),
            call_id: "test".to_string(),
            payload: ToolPayload::Freeform {
                input: r#"image "唐玄奘 画像" --download-dir media/image --max-results 3"#
                    .to_string(),
            },
        };
        let session_dir =
            std::env::temp_dir().join(format!("tura-web-discover-test-{}", std::process::id()));
        let ctx = ToolContext::new(session_dir);

        assert!(!handler.is_mutating(&call, &ctx).await);
        let access = handler.access(&call, &ctx).await;
        assert!(!access.workspace_write);
        assert_eq!(access.write_paths.len(), 1);
        assert!(access.write_paths[0].contains(".web_discover-image-"));
    }

    #[test]
    fn parse_exa_web_results_reads_sse_title_url_blocks() {
        let raw = r#"event: message
data: {"result":{"content":[{"type":"text","text":"Title: prunaai/z-image-turbo | API reference - Replicate\nURL: https://replicate.com/prunaai/z-image-turbo/api/api-reference\nPublished: 2026-02-27T15:34:39.000Z\nAuthor: N/A\nHighlights:\nPlayground API Examples README\n\n---\n\nTitle: Z-Image Turbo | Readme\nURL: https://replicate.com/prunaai/z-image-turbo/readme\nHighlights:\nReadme text"}]},"jsonrpc":"2.0","id":1}
"#;

        let results = parse_exa_web_results(raw, 5).expect("exa results");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, "exa_web");
        assert_eq!(
            results[0].url,
            "https://replicate.com/prunaai/z-image-turbo/api/api-reference"
        );
        assert!(results[0].title.contains("API reference"));
    }

    #[test]
    fn extract_page_image_url_reads_meta_and_resolves_relative_urls() {
        let html = r#"
            <html>
              <head><meta property="og:image" content="/images/minji.webp"></head>
              <body><img src="/fallback.jpg"></body>
            </html>
        "#;

        assert_eq!(
            extract_page_image_url(html, "https://example.com/profile/minji").as_deref(),
            Some("https://example.com/images/minji.webp")
        );
    }

    #[test]
    fn extract_reader_title_reads_jina_style_title() {
        assert_eq!(
            extract_reader_title("Title: Replicate API\n\nMarkdown body").as_deref(),
            Some("Replicate API")
        );
        assert_eq!(
            extract_reader_title("# Markdown Heading\n\nBody").as_deref(),
            Some("Markdown Heading")
        );
    }

    #[test]
    fn html_to_markdown_text_preserves_structure_and_drops_page_noise() {
        let html = r#"
            <html>
              <head>
                <meta property="og:image" content="/social-card.webp">
                <style>.hero { color: red; }</style>
              </head>
              <body>
                <nav>Home Docs Pricing</nav>
                <main>
                  <h1>API Reference</h1>
                  <img src="/media/profile.webp" alt="Profile photo">
                  <p>Create an image with this endpoint.</p>
                  <ul><li>Send a prompt</li><li>Read the output URL</li></ul>
                  <pre><code class="language-bash">curl https://api.example.com/v1/images</code></pre>
                  <a href="/docs/images">Image docs</a>
                  <script>window.payload = "https:\/\/cdn.example.com\/prod\/photo.jpg";</script>
                </main>
                <script>window.noise = true</script>
              </body>
            </html>
        "#;

        let markdown = html_to_markdown_text(html, "https://example.com/reference/");

        assert!(markdown.contains("# API Reference"));
        assert!(markdown.contains("- Send a prompt"));
        assert!(markdown.contains("```bash"));
        assert!(markdown.contains("[Image docs](https://example.com/docs/images)"));
        assert!(markdown.contains("![Profile photo](https://example.com/media/profile.webp)"));
        assert!(markdown.contains("https://example.com/social-card.webp"));
        assert!(markdown.contains("https://cdn.example.com/prod/photo.jpg"));
        assert!(!markdown.contains("window.noise"));
        assert!(!markdown.contains("color: red"));
    }

    #[test]
    fn site_filter_does_not_reject_image_results() {
        let result = SearchResult {
            title: "site:wikipedia.org 唐玄奘 画像".to_string(),
            url: "https://example-travel.invalid/random-garden.jpg".to_string(),
            page_url: Some("https://example-travel.invalid/article".to_string()),
            snippet: "https://example-travel.invalid/article".to_string(),
            source: "bing_images_mediaurl".to_string(),
        };

        let filtered = filter_results(
            vec![result],
            &WebDiscoverArgs {
                kind: "image".to_string(),
                query: "site:wikipedia.org 唐玄奘 画像".to_string(),
                include_regex: None,
                exclude_regex: None,
                max_results: 5,
                download_dir: Some("media".to_string()),
                min_size: 1,
                max_size: DEFAULT_MAX_SIZE,
                format_selector: None,
            },
        )
        .expect("filter results");

        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn image_query_treats_site_filter_as_keywords() {
        assert_eq!(
            site_filters_to_image_keywords("site: newjeans.kr Minji official profile"),
            "newjeans.kr Minji official profile"
        );
        assert_eq!(
            site_filters_to_image_keywords("site:newjeans.kr, Minji official profile"),
            "newjeans.kr Minji official profile"
        );
    }

    #[test]
    fn bing_mediaurl_title_uses_real_context_not_query_page_title() {
        let context = r#"
            mediaurl=https%3a%2f%2fimages.example.invalid%2funrelated.jpg&amp;cdnurl=https%3a%2f%2fth.bing.com%2fthumb.jpg
            <img alt="Résultat d’images pour site:wikipedia.org 唐玄奘 画像" />
            <div class="lnkw"><a title="example.invalid" target="_blank" data-hookid="pgdom" href="https://example.invalid/source-page">example.invalid</a></div>
        "#;

        let page_url = extract_bing_image_page_url(context).expect("page url");
        let title = extract_bing_image_title(
            context,
            Some(&page_url),
            "https://images.example.invalid/unrelated.jpg",
        );

        assert_eq!(page_url, "https://example.invalid/source-page");
        assert_eq!(title, "example.invalid");
        assert!(!title.contains("唐玄奘"));
    }
}
