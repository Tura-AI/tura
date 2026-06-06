use super::download::resolve_ytdlp_command;
use super::html::{
    direct_webpage_urls, extract_bing_image_page_url, extract_bing_image_title,
    parse_duckduckgo_html_results, title_from_url,
};
use super::policy;
use super::types::SearchResult;
use super::util::{
    clean_text, env_value, html_unescape, json_unescape, percent_decode, string_field,
    string_field_at, truncate_chars, EmptyDefault,
};
use super::POLICY;
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::process::{Command, Stdio};

pub(super) fn search_websites(
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
    let mut errors = Vec::new();
    for route in configured_search_routes() {
        match route {
            policy::SearchRoute::Brave => match brave_search_api_key() {
                Some(key) => match search_brave_web_links(client, query, limit, &key) {
                    Ok(results) => return Ok(results),
                    Err(err) => errors.push(format!("brave: {err}")),
                },
                None => errors.push("brave: api key unavailable or disabled".to_string()),
            },
            policy::SearchRoute::Exa => match search_exa_web_links(client, query, limit) {
                Ok(results) => return Ok(results),
                Err(err) => errors.push(format!("exa: {err}")),
            },
            policy::SearchRoute::DuckDuckGo => {
                match search_duckduckgo_web_links(client, query, limit) {
                    Ok(results) => return Ok(results),
                    Err(err) => errors.push(format!("duckduckgo: {err}")),
                }
            }
        }
    }
    Err(format!(
        "website search routes failed: {}",
        errors.join(" | ")
    ))
}

pub(super) fn configured_search_routes() -> [policy::SearchRoute; 3] {
    policy::search_routes(POLICY)
}

pub(super) fn search_duckduckgo_web_links(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
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
        "DuckDuckGo HTML fallback failed: {}",
        errors.join(" | ")
    ))
}

pub(super) fn search_exa_web_links(
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

pub(super) fn parse_exa_web_results(raw: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
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

pub(super) fn search_duckduckgo_html_endpoint(
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

pub(super) fn search_custom_endpoint(
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

pub(super) fn search_brave_web_links(
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

pub(super) fn search_media_links(
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

pub(super) fn direct_media_results(kind: &str, urls: Vec<String>) -> Vec<SearchResult> {
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

pub(super) fn search_image_links(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    if env_value("TURA_IMAGE_SEARCH_ENDPOINT").is_some() {
        return search_bing_image_links(client, query, limit);
    }
    let mut errors = Vec::new();
    for route in configured_search_routes() {
        match route {
            policy::SearchRoute::Brave => match brave_search_api_key() {
                Some(key) => match search_brave_image_links(client, query, limit, &key) {
                    Ok(results) => return Ok(results),
                    Err(err) => errors.push(format!("brave: {err}")),
                },
                None => errors.push("brave: api key unavailable or disabled".to_string()),
            },
            policy::SearchRoute::Exa => match search_exa_image_links(client, query, limit) {
                Ok(results) => return Ok(results),
                Err(err) => errors.push(format!("exa: {err}")),
            },
            policy::SearchRoute::DuckDuckGo => {
                match search_duckduckgo_image_route(client, query, limit) {
                    Ok(results) => return Ok(results),
                    Err(err) => errors.push(format!("duckduckgo: {err}")),
                }
            }
        }
    }
    Err(format!(
        "image search routes failed: {}",
        errors.join(" | ")
    ))
}

pub(super) fn search_duckduckgo_image_route(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    search_duckduckgo_image_links(client, query, limit)
}

pub(super) fn search_bing_image_links(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
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

pub(super) fn brave_search_api_key() -> Option<String> {
    if env_value("TURA_BRAVE_SEARCH_DISABLED")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
    {
        return None;
    }
    env_value("TURA_BRAVE_SEARCH_API_KEY").or_else(|| env_value("BRAVE_API_KEY"))
}

pub(super) fn search_brave_image_links(
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

pub(super) fn search_exa_image_links(
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

pub(super) fn extract_page_image_url(html: &str, base_url: &str) -> Option<String> {
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

pub(super) fn resolve_page_url(base_url: &str, candidate: &str) -> Option<String> {
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

pub(super) fn looks_like_image_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains(".jpg")
        || lower.contains(".jpeg")
        || lower.contains(".png")
        || lower.contains(".webp")
        || lower.contains("image")
        || lower.contains("img")
}

pub(super) fn search_duckduckgo_image_links(
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

pub(super) fn search_duckduckgo_image_links_from_endpoint(
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

pub(super) fn search_duckduckgo_image_links_with_library(
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

pub(super) fn extract_duckduckgo_vqd(page: &str) -> Option<String> {
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

pub(super) fn search_ytdlp_links(
    kind: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
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
