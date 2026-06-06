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
