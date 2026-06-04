use super::files::relative_or_display;
use super::html::{extract_reader_title, extract_title, html_to_markdown_text};
use super::types::{
    SearchResult, WebsiteContent, MAX_WEBSITE_RESPONSE_SIZE, MIN_WEBSITE_TEXT_CHARS_FOR_READER,
};
use super::util::{env_value, middle_truncate_chars, safe_filename};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::path::Path;

pub(super) fn website_records(
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

pub(super) fn fetch_website_content(client: &Client, url: &str) -> Result<WebsiteContent, String> {
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

pub(super) fn fetch_website_content_once(
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

pub(super) fn fetch_reader_content(client: &Client, url: &str) -> Result<WebsiteContent, String> {
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

pub(super) fn response_to_website_content(
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

pub(super) fn browser_user_agent() -> &'static str {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36"
}

pub(super) fn reader_min_text_chars() -> usize {
    env_value("TURA_WEB_READER_MIN_TEXT_CHARS")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(MIN_WEBSITE_TEXT_CHARS_FOR_READER)
}
