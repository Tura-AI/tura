use regex::Regex;
use serde_json::{Map, Value};

pub fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

pub fn parse_xml_parameter_value(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_string()))
}

pub fn xml_parameters(text: &str) -> Map<String, Value> {
    let mut arguments = Map::new();
    let Ok(param_re) = Regex::new(r#"(?s)<parameter\s+name=["']([^"']+)["']\s*>(.*?)</parameter>"#)
    else {
        return arguments;
    };

    for parameter in param_re.captures_iter(text) {
        let key = xml_unescape(
            parameter
                .get(1)
                .map(|value| value.as_str())
                .unwrap_or_default(),
        );
        let value = xml_unescape(
            parameter
                .get(2)
                .map(|value| value.as_str())
                .unwrap_or_default(),
        )
        .trim()
        .to_string();
        arguments.insert(key, parse_xml_parameter_value(&value));
    }
    arguments
}

#[cfg(test)]
mod tests {
    use super::{xml_parameters, xml_unescape};
    use serde_json::json;

    #[test]
    fn unescapes_xml_entities() {
        assert_eq!(xml_unescape("&lt;a&amp;b&gt;"), "<a&b>");
    }

    #[test]
    fn extracts_parameter_values() {
        let params = xml_parameters(
            r#"<parameter name="command_line">cat package.json</parameter><parameter name="step">1</parameter>"#,
        );
        assert_eq!(params["command_line"], "cat package.json");
        assert_eq!(params["step"], json!(1));
    }
}
