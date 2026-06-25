use super::types::{SearchResult, WebDiscoverArgs};
use regex::Regex;

pub(super) fn normalized_search_query(query: &str) -> String {
    query.trim().to_string()
}

pub(super) fn filter_results(
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

pub(super) fn site_filters_to_image_keywords(query: &str) -> String {
    let Ok(re) = Regex::new(r"(?i)\bsite:\s*([^\s,，]+)") else {
        return query.to_string();
    };
    re.replace_all(query, ", $1, ")
        .split(|ch: char| ch.is_whitespace() || ch == ',' || ch == '，')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn strip_site_filters_from_query(query: &str) -> String {
    let Ok(re) = Regex::new(r"(?i)\bsite:\s*[^\s,，]+") else {
        return query.to_string();
    };
    re.replace_all(query, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn site_filters_match(query: &str, result: &SearchResult) -> bool {
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

pub(super) fn site_filters(query: &str) -> Vec<String> {
    let Ok(re) = Regex::new(r"(?i)\bsite:\s*([^\s,，]+)") else {
        return Vec::new();
    };
    re.captures_iter(query)
        .filter_map(|capture| capture.get(1).map(|value| value.as_str()))
        .filter_map(url_host)
        .filter(|site| !site.is_empty())
        .collect()
}

pub(super) fn url_host_matches(url: &str, site: &str) -> bool {
    let Some(host) = url_host(url) else {
        return false;
    };
    host == site || host.ends_with(&format!(".{site}"))
}

pub(super) fn url_host(url: &str) -> Option<String> {
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
