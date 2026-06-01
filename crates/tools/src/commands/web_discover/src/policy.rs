#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SearchRoute {
    Brave,
    Exa,
    DuckDuckGo,
}

const DEFAULT_ROUTES: [SearchRoute; 3] = [
    SearchRoute::Brave,
    SearchRoute::Exa,
    SearchRoute::DuckDuckGo,
];

pub(super) fn search_routes(policy: &str) -> [SearchRoute; 3] {
    [
        route_config_value(policy, "first_route").unwrap_or(DEFAULT_ROUTES[0]),
        route_config_value(policy, "second_route").unwrap_or(DEFAULT_ROUTES[1]),
        route_config_value(policy, "third_route").unwrap_or(DEFAULT_ROUTES[2]),
    ]
}

fn route_config_value(policy: &str, key: &str) -> Option<SearchRoute> {
    configurable_value(policy, key).and_then(|value| parse_route(&value))
}

fn parse_route(value: &str) -> Option<SearchRoute> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "brave" => Some(SearchRoute::Brave),
        "exa" => Some(SearchRoute::Exa),
        "duckduckgo" | "duck_duck_go" | "ddg" => Some(SearchRoute::DuckDuckGo),
        _ => None,
    }
}

fn configurable_value(policy: &str, key: &str) -> Option<String> {
    let mut in_configurable = false;
    for line in policy.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_configurable = trimmed == "[configurable]";
            continue;
        }
        if !in_configurable || trimmed.starts_with('#') {
            continue;
        }
        let Some((name, value)) = trimmed.split_once('=') else {
            continue;
        };
        if name.trim() != key {
            continue;
        }
        return inline_default_value(value).or_else(|| quoted_value(value));
    }
    None
}

fn inline_default_value(value: &str) -> Option<String> {
    let marker = "default";
    let default_at = value.find(marker)?;
    let after_default = &value[default_at + marker.len()..];
    let (_, after_equals) = after_default.split_once('=')?;
    quoted_value(after_equals)
}

fn quoted_value(value: &str) -> Option<String> {
    let start = value.find('"')?;
    let rest = &value[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_configurable_routes_from_policy() {
        let routes = search_routes(
            r#"
read_only = false

[configurable]
first_route = { default = "duckduckgo", enum = ["brave", "exa", "duckduckgo"] }
second_route = { default = "brave", enum = ["brave", "exa", "duckduckgo"] }
third_route = { default = "exa", enum = ["brave", "exa", "duckduckgo"] }
"#,
        );

        assert_eq!(
            routes,
            [
                SearchRoute::DuckDuckGo,
                SearchRoute::Brave,
                SearchRoute::Exa
            ]
        );
    }

    #[test]
    fn falls_back_when_route_is_not_in_enum() {
        let routes = search_routes(
            r#"
[configurable]
first_route = { default = "bing", enum = ["brave", "exa", "duckduckgo"] }
"#,
        );

        assert_eq!(routes[0], SearchRoute::Brave);
        assert_eq!(routes[1], SearchRoute::Exa);
        assert_eq!(routes[2], SearchRoute::DuckDuckGo);
    }
}
