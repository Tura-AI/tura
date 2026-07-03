use super::types::{
    WebDiscoverArgs, DEFAULT_IMAGE_MIN_SIZE, DEFAULT_MAX_RESULTS, DEFAULT_MAX_SIZE,
    DEFAULT_MIN_SIZE,
};
use super::util::{split_cli_words, string_field, u64_field};
use serde_json::Value;

pub(super) fn parse_args_text(command_line: &str) -> Result<WebDiscoverArgs, String> {
    let trimmed = command_line.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_str::<Value>(trimmed)
            .map_err(|err| format!("invalid web_discover JSON: {err}"))
            .and_then(parse_args_value);
    }
    parse_cli_args(trimmed)
}

pub(super) fn parse_args_value(value: Value) -> Result<WebDiscoverArgs, String> {
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

pub(super) fn parse_cli_args(input: &str) -> Result<WebDiscoverArgs, String> {
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
            _ if query_parts.is_empty() && kind == "website" && is_kind_word(&word) => {
                kind = normalize_kind(&word)
            }
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

pub(super) fn is_web_discover_command_name(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "web_discover" | "web-discover" | "webdiscover" | "web_search" | "web-search"
    )
}

pub(super) fn is_kind_word(value: &str) -> bool {
    matches!(
        normalize_kind(value).as_str(),
        "website" | "image" | "video" | "audio" | "asset"
    )
}

pub(super) fn split_cli_assignment(word: &str) -> (String, Option<String>) {
    if let Some((key, value)) = word.split_once('=') {
        if key.starts_with('-') {
            return (key.to_string(), Some(value.to_string()));
        }
    }
    (word.to_string(), None)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn args_from_parts(
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
    if !matches!(
        kind.as_str(),
        "website" | "image" | "video" | "audio" | "asset"
    ) {
        return Err(format!("unsupported web_discover type: {kind}"));
    }
    if query.trim().is_empty() {
        return Err("web_discover query cannot be empty".to_string());
    }
    let (asset_type, query) = if kind == "asset" {
        split_asset_type_from_query(query)
    } else {
        (None, query)
    };
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
        asset_type,
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

fn split_asset_type_from_query(query: String) -> (Option<String>, String) {
    let trimmed = query.trim();
    let Some((head, tail)) = trimmed.split_once(char::is_whitespace) else {
        return (normalize_asset_type(trimmed), String::new());
    };
    match normalize_asset_type(head) {
        Some(asset_type) => (Some(asset_type), tail.trim().to_string()),
        None => (None, trimmed.to_string()),
    }
}

fn normalize_asset_type(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "3d" | "model" | "models" => Some("3d".to_string()),
        "texture" | "textures" | "material" | "materials" => Some("texture".to_string()),
        "2d" | "sprite" | "sprites" | "ui" => Some("2d".to_string()),
        "shader" | "shaders" => Some("shader".to_string()),
        "audio" | "sound" | "sounds" | "sfx" => Some("audio".to_string()),
        "auto" | "any" | "asset" | "assets" => Some("auto".to_string()),
        _ => None,
    }
}

pub(super) fn normalize_kind(value: &str) -> String {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "web" | "page" | "pages" | "site" | "website" | "webpage" | "webpages" | "web_page"
        | "web_pages" => "website".to_string(),
        "img" | "images" | "photo" | "photos" => "image".to_string(),
        "videos" | "movie" | "movies" => "video".to_string(),
        "sound" | "music" => "audio".to_string(),
        "assets" => "asset".to_string(),
        other => other.to_string(),
    }
}
