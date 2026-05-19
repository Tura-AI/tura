use rand::seq::SliceRandom;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct StickerManager {
    sticker_path: String,
    pub sticker_db: HashMap<String, Value>,
}

impl StickerManager {
    pub fn new(sticker_file: &str) -> Self {
        let mut manager = Self {
            sticker_path: sticker_file.to_string(),
            sticker_db: HashMap::new(),
        };
        manager.refresh();
        manager
    }

    pub fn refresh(&mut self) {
        let path = Path::new(&self.sticker_path);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(mut db) = serde_json::from_str::<HashMap<String, Value>>(&content) {
                    // Clean up metadata if present
                    db.remove("_meta_packs");
                    self.sticker_db = db;
                    return;
                }
            }
        }
        self.sticker_db = HashMap::new();
    }

    pub fn get_sticker_id(&self, emoji: &str) -> Option<String> {
        if let Some(Value::Array(options)) = self.sticker_db.get(emoji) {
            if !options.is_empty() {
                let mut rng = rand::thread_rng();
                if let Some(Value::String(sticker_id)) = options.choose(&mut rng) {
                    return Some(sticker_id.clone());
                }
            }
        }
        None
    }
}

#[derive(Debug, PartialEq)]
pub struct ToolEvent {
    pub tool: String,
    pub args: HashMap<String, String>,
    pub consumed_text: Option<String>,
}

pub struct StreamTextManager<'a> {
    sticker_manager: &'a StickerManager,
    regex: Regex,
}

impl<'a> StreamTextManager<'a> {
    pub fn new(sticker_manager: &'a StickerManager) -> Self {
        Self {
            sticker_manager,
            // Case-insensitive regex to catch LLM capitalization mistakes
            regex: Regex::new(r"(?i)\[\s*EMOJI\s*:\s*(react|sticker)\s*:\s*(.*?)\s*\]").unwrap(),
        }
    }

    pub fn flush_buffer(&self, buffer: &mut String) -> Vec<ToolEvent> {
        let mut events = Vec::new();

        while !buffer.is_empty() {
            if let Some(start_idx) = buffer.find('[') {
                // Flush plain text before the tag start first.
                if start_idx > 0 {
                    let prefix = buffer[..start_idx].to_string();
                    events.push(ToolEvent {
                        tool: "stream_text".to_string(),
                        args: HashMap::from([("content".to_string(), prefix.clone())]),
                        consumed_text: Some(prefix.clone()),
                    });
                    *buffer = buffer[start_idx..].to_string();
                    continue;
                }

                if let Some(end_idx) = buffer.find(']') {
                    let tag_str = buffer[..=end_idx].to_string();
                    let mut event = self.build_tag_event(&tag_str);
                    event.consumed_text = Some(tag_str.clone());
                    events.push(event);

                    // Move past the processed tag
                    *buffer = buffer[end_idx + 1..].to_string();
                } else {
                    // Incomplete tag: keep it in buffer
                    break;
                }
            } else {
                // No tag start at all -> flush everything as plain text.
                events.push(ToolEvent {
                    tool: "stream_text".to_string(),
                    args: HashMap::from([("content".to_string(), buffer.clone())]),
                    consumed_text: Some(buffer.clone()),
                });
                buffer.clear();
                break;
            }
        }
        events
    }

    fn build_tag_event(&self, tag_str: &str) -> ToolEvent {
        if let Some(caps) = self.regex.captures(tag_str.trim()) {
            let action = caps.get(1).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
            let emoji_req = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();

            if action == "react" {
                return ToolEvent {
                    tool: "react".to_string(),
                    args: HashMap::from([("emoji".to_string(), emoji_req)]),
                    consumed_text: None,
                };
            }

            if action == "sticker" {
                if let Some(file_id) = self.sticker_manager.get_sticker_id(&emoji_req) {
                    return ToolEvent {
                        tool: "sticker".to_string(),
                        args: HashMap::from([("file_id".to_string(), file_id)]),
                        consumed_text: None,
                    };
                } else {
                    return ToolEvent {
                        tool: "standalone_emoji".to_string(),
                        args: HashMap::from([("emoji".to_string(), emoji_req)]),
                        consumed_text: None,
                    };
                }
            }
        }

        ToolEvent {
            tool: "stream_text".to_string(),
            args: HashMap::from([("content".to_string(), tag_str.to_string())]),
            consumed_text: None,
        }
    }
}