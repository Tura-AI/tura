use super::filter::url_host;
use super::types::SearchResult;
use super::util::{
    clean_text, extension_from_url, html_unescape, json_unescape, percent_decode, split_cli_words,
};
use quick_html2md::{html_to_markdown_with_options, MarkdownOptions};
use regex::Regex;

pub(super) fn extract_bing_image_page_url(context: &str) -> Option<String> {
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

pub(super) fn extract_bing_image_title(
    context: &str,
    page_url: Option<&str>,
    media_url: &str,
) -> String {
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

pub(super) fn parse_duckduckgo_html_results(html: &str, limit: usize) -> Vec<SearchResult> {
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

pub(super) fn normalize_duckduckgo_url(url: &str) -> String {
    if let Some(encoded) = url
        .split("uddg=")
        .nth(1)
        .and_then(|rest| rest.split('&').next())
    {
        return percent_decode(encoded);
    }
    html_unescape(url)
}

pub(super) fn html_to_markdown_text(html: &str, base_url: &str) -> String {
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

pub(super) fn remove_html_noise(html: &str) -> String {
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

pub(super) fn extract_media_links(html: &str, base_url: &str) -> Vec<String> {
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

pub(super) fn normalize_media_url(candidate: &str, base_url: &str) -> Option<String> {
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

pub(super) fn normalize_markdown(value: &str) -> String {
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

pub(super) fn extract_title(html: &str) -> Option<String> {
    Regex::new("(?is)<title[^>]*>(.*?)</title>")
        .ok()?
        .captures(html)
        .and_then(|capture| capture.get(1))
        .map(|value| clean_text(value.as_str()))
        .filter(|value| !value.is_empty())
}

pub(super) fn extract_reader_title(text: &str) -> Option<String> {
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

pub(super) fn direct_webpage_url(query: &str) -> Option<String> {
    let mut urls = direct_webpage_urls(query);
    if urls.len() == 1 {
        urls.pop()
    } else {
        None
    }
}

pub(super) fn direct_webpage_urls(query: &str) -> Vec<String> {
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

pub(super) fn title_from_url(url: &str) -> String {
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
