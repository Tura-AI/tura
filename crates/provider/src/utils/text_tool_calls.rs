use regex::Regex;
use serde_json::{json, Value};

use super::{
    json_prefix, normalize_command_run_tool_input, parse_xml_parameter_value, xml_parameters,
    xml_unescape,
};

pub fn text_tool_calls_value(text: &str) -> Value {
    let calls = extract_text_tool_calls(text);
    if calls.is_empty() {
        Value::Null
    } else {
        Value::Array(calls)
    }
}

pub fn strip_text_tool_calls(text: &str) -> String {
    strip_dsml_tool_calls(&strip_minimax_xml_tool_calls(text))
}

pub fn extract_text_tool_calls(text: &str) -> Vec<Value> {
    let mut calls = extract_minimax_xml_tool_calls(text);
    calls.extend(extract_dsml_tool_calls(text));
    calls
}

pub fn extract_xml_tool_calls(text: &str) -> Vec<Value> {
    if !text.contains("<invoke") {
        return Vec::new();
    }
    let Ok(invoke_re) = Regex::new(r#"(?s)<invoke\s+name=["']([^"']+)["']\s*>(.*?)</invoke>"#)
    else {
        return Vec::new();
    };

    invoke_re
        .captures_iter(text)
        .enumerate()
        .map(|(index, capture)| {
            let name = xml_unescape(
                capture
                    .get(1)
                    .map(|value| value.as_str())
                    .unwrap_or_default(),
            );
            let body = capture
                .get(2)
                .map(|value| value.as_str())
                .unwrap_or_default();
            let arguments =
                normalize_command_run_tool_input(&name, Value::Object(xml_parameters(body)));
            json!({
                "id": format!("xml_tool_call_{index}"),
                "type": "function",
                "function": { "name": name, "arguments": arguments },
            })
        })
        .collect()
}

pub fn strip_xml_tool_calls(text: &str) -> String {
    let Ok(block_re) = Regex::new(r#"(?s)<(?:antml:)?tool_call>.*?</(?:antml:)?tool_call>"#) else {
        return text.to_string();
    };
    let stripped = block_re.replace_all(text, "");
    let Ok(invoke_re) = Regex::new(r#"(?s)<invoke\s+name=["'][^"']+["']\s*>.*?</invoke>"#) else {
        return stripped.trim().to_string();
    };
    invoke_re.replace_all(&stripped, "").trim().to_string()
}

fn extract_minimax_xml_tool_calls(text: &str) -> Vec<Value> {
    if !text.contains("<minimax:tool_call>") && !text.contains("<invoke") {
        return Vec::new();
    }
    extract_xml_tool_calls(text)
        .into_iter()
        .enumerate()
        .map(|(index, mut call)| {
            call["id"] = Value::String(format!("minimax_tool_call_{index}"));
            if let Some(arguments) = call.pointer("/function/arguments").cloned() {
                call["function"]["arguments"] = Value::String(arguments.to_string());
            }
            call
        })
        .collect()
}

fn extract_dsml_tool_calls(text: &str) -> Vec<Value> {
    if !text.contains("<｜DSML｜parameter") {
        return Vec::new();
    }
    let params = extract_dsml_parameters(text);
    if params.is_empty() {
        return Vec::new();
    }
    vec![json!({
        "id": "dsml_tool_call_0",
        "type": "function",
        "function": {
            "name": "command_run",
            "arguments": Value::Object(params),
        },
    })]
}

fn extract_dsml_parameters(text: &str) -> serde_json::Map<String, Value> {
    let mut arguments = serde_json::Map::new();
    let Ok(param_re) = Regex::new(r#"(?s)<｜DSML｜parameter\s+name=["']([^"']+)["'][^>]*>"#)
    else {
        return arguments;
    };
    let matches: Vec<_> = param_re.captures_iter(text).collect();
    for (index, capture) in matches.iter().enumerate() {
        let Some(full) = capture.get(0) else {
            continue;
        };
        let key = xml_unescape(
            capture
                .get(1)
                .map(|value| value.as_str())
                .unwrap_or_default(),
        );
        let value_start = full.end();
        let value_end = matches
            .get(index + 1)
            .and_then(|next| next.get(0))
            .map(|next| next.start())
            .unwrap_or(text.len());
        let raw_value = text[value_start..value_end].trim();
        let value = json_prefix(raw_value)
            .map(parse_xml_parameter_value)
            .unwrap_or_else(|| parse_xml_parameter_value(raw_value));
        arguments.insert(key, value);
    }
    arguments
}

fn strip_minimax_xml_tool_calls(text: &str) -> String {
    let Ok(block_re) = Regex::new(r#"(?s)<minimax:tool_call>.*?</minimax:tool_call>"#) else {
        return text.to_string();
    };
    let stripped = block_re.replace_all(text, "");
    let Ok(invoke_re) = Regex::new(r#"(?s)<invoke\s+name=["'][^"']+["']\s*>.*?</invoke>"#) else {
        return stripped.trim().to_string();
    };
    invoke_re.replace_all(&stripped, "").trim().to_string()
}

fn strip_dsml_tool_calls(text: &str) -> String {
    let Ok(param_re) = Regex::new(r#"(?s)<｜DSML｜parameter\s+name=["'][^"']+["'][^>]*>.*"#)
    else {
        return text.to_string();
    };
    param_re.replace_all(text, "").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{extract_xml_tool_calls, strip_xml_tool_calls, text_tool_calls_value};

    #[test]
    fn normalizes_minimax_xml_tool_call_content() {
        let content = text_tool_calls_value(
            "<minimax:tool_call>\n<invoke name=\"get_file_outline\">\n<parameter name=\"path\">services/mano/src/manas</parameter>\n<parameter name=\"max_results\">3</parameter>\n</invoke>\n</minimax:tool_call>",
        );

        assert_eq!(content[0]["function"]["name"], "get_file_outline");
        assert_eq!(
            content[0]["function"]["arguments"],
            "{\"max_results\":3,\"path\":\"services/mano/src/manas\"}"
        );
    }

    #[test]
    fn normalizes_deepseek_dsml_command_run_content() {
        let content = text_tool_calls_value(
            "<｜DSML｜parameter name=\"commands\" string=\"false\">[{\"command_type\":\"task_status\",\"status\":\"done\",\"task_group\":\"商城前端\"}]",
        );

        assert_eq!(content[0]["function"]["name"], "command_run");
        assert_eq!(
            content[0]["function"]["arguments"]["commands"][0]["command_type"],
            "task_status"
        );
        assert_eq!(
            content[0]["function"]["arguments"]["commands"][0]["status"],
            "done"
        );
    }

    #[test]
    fn text_xml_invoke_becomes_tool_call() {
        let calls = extract_xml_tool_calls(
            "<invoke name=\"command_run\"><parameter name=\"commands\">[{\"command_type\":\"task_status\",\"status\":\"done\",\"task_group\":\"商城前端\"}]</parameter></invoke>",
        );

        assert_eq!(
            calls[0]["function"]["arguments"]["commands"][0]["status"],
            "done"
        );
        assert!(strip_xml_tool_calls("<invoke name=\"x\"></invoke>").is_empty());
    }
}
