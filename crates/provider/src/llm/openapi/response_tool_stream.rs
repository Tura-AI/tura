use crate::tura_llm::ProviderStreamEvent;
use serde_json::Value;

pub(crate) fn codex_event_tool_calls(events: &[Value]) -> Vec<Value> {
    let mut collector = CodexToolCallStreamCollector::default();
    let mut calls = Vec::new();
    for event in events {
        calls.extend(collector.push_event(event));
    }
    calls.extend(collector.finish());
    calls
}

#[derive(Default)]
pub(crate) struct CodexToolCallStreamCollector {
    active: Option<String>,
    entries: Vec<CodexToolCallEntry>,
}

#[derive(Default)]
struct CodexToolCallEntry {
    id: String,
    call_id: String,
    name: String,
    arguments: String,
    emitted: bool,
}

impl CodexToolCallStreamCollector {
    pub(crate) fn push_event(&mut self, event: &Value) -> Vec<Value> {
        if let Some(item) = event.get("item") {
            if item.get("type").and_then(Value::as_str) == Some("function_call") {
                self.upsert_item(item);
            }
        }

        match event.get("type").and_then(Value::as_str) {
            Some("response.function_call_arguments.delta") => {
                if let (Some(id), Some(delta)) = (
                    self.event_tool_id(event),
                    event.get("delta").and_then(Value::as_str),
                ) {
                    if let Some(entry) = self.entry_mut(&id) {
                        entry.arguments.push_str(delta);
                    }
                }
                Vec::new()
            }
            Some("response.function_call_arguments.done") => {
                let id = self.event_tool_id(event);
                if let (Some(id), Some(arguments)) =
                    (id, event.get("arguments").and_then(Value::as_str))
                {
                    if let Some(entry) = self.entry_mut(&id) {
                        entry.arguments = arguments.to_string();
                    }
                    return self.emit_ready(&id);
                }
                Vec::new()
            }
            Some("response.output_item.done") => self
                .active
                .clone()
                .map(|id| self.emit_ready(&id))
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn finish(&mut self) -> Vec<Value> {
        let ids = self
            .entries
            .iter()
            .map(|entry| entry.id.clone())
            .collect::<Vec<_>>();
        ids.into_iter()
            .flat_map(|id| self.emit_ready(&id))
            .collect()
    }

    fn upsert_item(&mut self, item: &Value) {
        let id = item
            .get("id")
            .or_else(|| item.get("call_id"))
            .and_then(Value::as_str)
            .unwrap_or("codex_tool_call")
            .to_string();
        let call_id = item
            .get("call_id")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .unwrap_or(id.as_str())
            .to_string();
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let arguments = item
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        self.active = Some(id.clone());
        if let Some(entry) = self.entry_mut(&id) {
            if !call_id.is_empty() {
                entry.call_id = call_id;
            }
            if !name.is_empty() {
                entry.name = name;
            }
            if !arguments.is_empty() {
                entry.arguments = arguments;
            }
        } else {
            self.entries.push(CodexToolCallEntry {
                id,
                call_id,
                name,
                arguments,
                emitted: false,
            });
        }
    }

    fn entry_mut(&mut self, id: &str) -> Option<&mut CodexToolCallEntry> {
        self.entries
            .iter_mut()
            .find(|entry| entry.id == id || entry.call_id == id)
    }

    fn event_tool_id(&self, event: &Value) -> Option<String> {
        event
            .get("item_id")
            .or_else(|| event.get("call_id"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| self.active.clone())
    }

    fn emit_ready(&mut self, id: &str) -> Vec<Value> {
        let Some(entry) = self.entry_mut(id) else {
            return Vec::new();
        };
        if entry.emitted
            || entry.name.is_empty()
            || serde_json::from_str::<Value>(&entry.arguments).is_err()
        {
            return Vec::new();
        }
        entry.emitted = true;
        let call = super::codex_tool_call_value(
            &entry.call_id,
            &entry.name,
            Value::String(entry.arguments.clone()),
        );
        super::ready_streaming_tool_call(call).into_iter().collect()
    }
}

#[derive(Default)]
pub(crate) struct CodexCommandRunCommandCollector {
    active: Option<String>,
    entries: Vec<CodexCommandRunCommandEntry>,
}

#[derive(Default)]
struct CodexCommandRunCommandEntry {
    id: String,
    call_id: String,
    name: String,
    arguments: String,
    emitted_commands: usize,
}

impl CodexCommandRunCommandCollector {
    pub(crate) fn push_event(&mut self, event: &Value) -> Vec<ProviderStreamEvent> {
        if let Some(item) = event.get("item") {
            if item.get("type").and_then(Value::as_str) == Some("function_call") {
                self.upsert_item(item);
            }
        }

        match event.get("type").and_then(Value::as_str) {
            Some("response.function_call_arguments.delta") => {
                if let (Some(id), Some(delta)) = (
                    self.event_tool_id(event),
                    event.get("delta").and_then(Value::as_str),
                ) {
                    if let Some(entry) = self.entry_mut(&id) {
                        entry.arguments.push_str(delta);
                        return Self::emit_ready_commands(entry);
                    }
                }
                Vec::new()
            }
            Some("response.function_call_arguments.done") => {
                if let (Some(id), Some(arguments)) = (
                    self.event_tool_id(event),
                    event.get("arguments").and_then(Value::as_str),
                ) {
                    if let Some(entry) = self.entry_mut(&id) {
                        entry.arguments = arguments.to_string();
                        return Self::emit_ready_commands(entry);
                    }
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn upsert_item(&mut self, item: &Value) {
        let id = item
            .get("id")
            .or_else(|| item.get("call_id"))
            .and_then(Value::as_str)
            .unwrap_or("codex_tool_call")
            .to_string();
        let call_id = item
            .get("call_id")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .unwrap_or(id.as_str())
            .to_string();
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let arguments = item
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        self.active = Some(id.clone());
        if let Some(entry) = self.entry_mut(&id) {
            if !call_id.is_empty() {
                entry.call_id = call_id;
            }
            if !name.is_empty() {
                entry.name = name;
            }
            if !arguments.is_empty() {
                entry.arguments = arguments;
            }
        } else {
            self.entries.push(CodexCommandRunCommandEntry {
                id,
                call_id,
                name,
                arguments,
                emitted_commands: 0,
            });
        }
    }

    fn entry_mut(&mut self, id: &str) -> Option<&mut CodexCommandRunCommandEntry> {
        self.entries
            .iter_mut()
            .find(|entry| entry.id == id || entry.call_id == id)
    }

    fn event_tool_id(&self, event: &Value) -> Option<String> {
        event
            .get("item_id")
            .or_else(|| event.get("call_id"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| self.active.clone())
    }

    fn emit_ready_commands(entry: &mut CodexCommandRunCommandEntry) -> Vec<ProviderStreamEvent> {
        if entry.name != "command_run" {
            return Vec::new();
        }
        let commands = complete_command_run_command_objects(&entry.arguments);
        if commands.len() <= entry.emitted_commands {
            return Vec::new();
        }
        let start = entry.emitted_commands;
        entry.emitted_commands = commands.len();
        commands
            .into_iter()
            .enumerate()
            .skip(start)
            .map(
                |(command_index, command)| ProviderStreamEvent::CommandRunCommandReady {
                    tool_call_id: entry.call_id.clone(),
                    command_index,
                    command,
                },
            )
            .collect()
    }
}

fn complete_command_run_command_objects(arguments: &str) -> Vec<Value> {
    let Some(array_start) = find_commands_array_start(arguments) else {
        return Vec::new();
    };
    let mut commands = Vec::new();
    let mut in_string = false;
    let mut escape = false;
    let mut depth = 0_i32;
    let mut object_start = None;

    for (offset, ch) in arguments[array_start + 1..].char_indices() {
        let index = array_start + 1 + offset;
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    object_start = Some(index);
                }
                depth += 1;
            }
            '}' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(start) = object_start.take() {
                            if let Ok(value) =
                                serde_json::from_str::<Value>(&arguments[start..=index])
                            {
                                commands.push(value);
                            }
                        }
                    }
                }
            }
            ']' if depth == 0 => break,
            _ => {}
        }
    }

    commands
}

fn find_commands_array_start(arguments: &str) -> Option<usize> {
    let mut in_string = false;
    let mut escape = false;
    let mut key_start = None;
    let mut last_key = None::<String>;

    for (index, ch) in arguments.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            if ch == '\\' {
                escape = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
                if let Some(start) = key_start.take() {
                    if let Ok(key) = serde_json::from_str::<String>(&arguments[start..=index]) {
                        last_key = Some(key);
                    }
                }
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                key_start = Some(index);
            }
            '[' if last_key.as_deref() == Some("commands") => return Some(index),
            ':' | ' ' | '\n' | '\r' | '\t' => {}
            _ => {
                if ch != ',' {
                    last_key = None;
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        codex_event_tool_calls, complete_command_run_command_objects, find_commands_array_start,
        CodexCommandRunCommandCollector, CodexToolCallStreamCollector,
    };
    use crate::tura_llm::ProviderStreamEvent;
    use serde_json::json;

    fn command_index(event: &ProviderStreamEvent) -> usize {
        match event {
            ProviderStreamEvent::CommandRunCommandReady { command_index, .. } => *command_index,
            ProviderStreamEvent::ProviderOutputStarted | ProviderStreamEvent::TextDelta { .. } => {
                panic!("unexpected provider event: {event:?}")
            }
        }
    }

    #[test]
    fn complete_command_objects_handles_nested_json_strings_and_incomplete_tail() {
        let arguments = r#"{
            "commands": [
                {"step":1,"command_type":"shell_command","command_line":"echo {one}"},
                {"step":2,"payload":{"text":"value with } brace","items":[1,2,3]}},
                {"step":3,"command_line":"incomplete""#;

        let commands = complete_command_run_command_objects(arguments);

        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0]["command_line"], "echo {one}");
        assert_eq!(commands[1]["payload"]["text"], "value with } brace");
        assert_eq!(commands[1]["payload"]["items"][2], 3);
    }

    #[test]
    fn commands_array_start_ignores_commands_inside_string_values() {
        let arguments = r#"{"note":"the word \"commands\" appears here","commands":[{"step":1}]}"#;

        let start = find_commands_array_start(arguments).expect("commands array");

        assert_eq!(&arguments[start..start + 2], "[{");
        assert_eq!(complete_command_run_command_objects(arguments).len(), 1);
    }

    #[test]
    fn tool_call_collector_emits_once_when_done_and_finish_repeat() {
        let mut collector = CodexToolCallStreamCollector::default();
        assert!(collector
            .push_event(&json!({
                "type": "response.output_item.added",
                "item": {
                    "id": "fc_once",
                    "call_id": "call_once",
                    "type": "function_call",
                    "name": "command_run"
                }
            }))
            .is_empty());
        let ready = collector.push_event(&json!({
            "type": "response.function_call_arguments.done",
            "item_id": "fc_once",
            "arguments": "{\"commands\":[{\"step\":1,\"command\":\"pwd\"}]}"
        }));
        let duplicate_done = collector.push_event(&json!({
            "type": "response.function_call_arguments.done",
            "item_id": "fc_once",
            "arguments": "{\"commands\":[{\"step\":1,\"command\":\"pwd\"}]}"
        }));

        assert_eq!(ready.len(), 1);
        assert!(duplicate_done.is_empty());
        assert!(collector.finish().is_empty());
        assert_eq!(ready[0]["id"], "call_once");
        assert_eq!(ready[0]["function"]["name"], "command_run");
    }

    #[test]
    fn tool_call_collector_waits_for_name_and_valid_json() {
        let mut collector = CodexToolCallStreamCollector::default();
        collector.push_event(&json!({
            "type": "response.output_item.added",
            "item": {"id": "fc_missing_name", "type": "function_call", "arguments": ""}
        }));

        assert!(collector
            .push_event(&json!({
                "type": "response.function_call_arguments.delta",
                "item_id": "fc_missing_name",
                "delta": "{\"commands\":["
            }))
            .is_empty());
        assert!(collector.finish().is_empty());

        assert!(collector
            .push_event(&json!({
                "type": "response.output_item.added",
                "item": {
                    "id": "fc_missing_name",
                    "type": "function_call",
                    "name": "command_run"
                }
            }))
            .is_empty());
        let ready = collector.push_event(&json!({
            "type": "response.function_call_arguments.done",
            "item_id": "fc_missing_name",
            "arguments": "{\"commands\":[{\"step\":1}]}"
        }));

        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn command_run_collector_ignores_non_command_run_tools() {
        let mut collector = CodexCommandRunCommandCollector::default();
        collector.push_event(&json!({
            "type": "response.output_item.added",
            "item": {
                "id": "fc_read",
                "call_id": "call_read",
                "type": "function_call",
                "name": "read_media"
            }
        }));
        let events = collector.push_event(&json!({
            "type": "response.function_call_arguments.done",
            "item_id": "fc_read",
            "arguments": "{\"path\":\"image.png\"}"
        }));

        assert!(events.is_empty());
    }

    #[test]
    fn command_run_collector_emits_only_new_complete_commands() {
        let mut collector = CodexCommandRunCommandCollector::default();
        collector.push_event(&json!({
            "type": "response.output_item.added",
            "item": {
                "id": "fc_commands",
                "call_id": "call_commands",
                "type": "function_call",
                "name": "command_run",
                "arguments": ""
            }
        }));
        let first = collector.push_event(&json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_commands",
            "delta": "{\"commands\":[{\"step\":1,\"command\":\"one\"},"
        }));
        let repeat = collector.push_event(&json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_commands",
            "delta": ""
        }));
        let second = collector.push_event(&json!({
            "type": "response.function_call_arguments.done",
            "item_id": "fc_commands",
            "arguments": "{\"commands\":[{\"step\":1,\"command\":\"one\"},{\"step\":2,\"command\":\"two\"}]}"
        }));

        assert_eq!(first.len(), 1);
        assert_eq!(command_index(&first[0]), 0);
        assert!(repeat.is_empty());
        assert_eq!(second.len(), 1);
        assert_eq!(command_index(&second[0]), 1);
    }

    #[test]
    fn codex_event_tool_calls_keeps_multiple_call_ids_separate() {
        let events = vec![
            json!({
                "type": "response.output_item.added",
                "item": {
                    "id": "fc_a",
                    "call_id": "call_a",
                    "type": "function_call",
                    "name": "command_run",
                    "arguments": ""
                }
            }),
            json!({
                "type": "response.output_item.added",
                "item": {
                    "id": "fc_b",
                    "call_id": "call_b",
                    "type": "function_call",
                    "name": "read_media",
                    "arguments": ""
                }
            }),
            json!({
                "type": "response.function_call_arguments.done",
                "item_id": "fc_a",
                "arguments": "{\"commands\":[{\"step\":1,\"command\":\"pwd\"}]}"
            }),
            json!({
                "type": "response.function_call_arguments.done",
                "item_id": "fc_b",
                "arguments": "{\"path\":\"image.png\"}"
            }),
        ];

        let calls = codex_event_tool_calls(&events);

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0]["id"], "call_a");
        assert_eq!(calls[1]["id"], "call_b");
        assert_eq!(calls[0]["function"]["name"], "command_run");
        assert_eq!(calls[1]["function"]["name"], "read_media");
    }
}
