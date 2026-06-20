use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailPrompt {
    role: &'static str,
    content: String,
}

impl TailPrompt {
    pub fn new(role: &'static str, content: impl AsRef<str>) -> Option<Self> {
        let content = content.as_ref().trim();
        if role.trim().is_empty() || content.is_empty() {
            return None;
        }
        Some(Self {
            role,
            content: content.to_string(),
        })
    }

    pub fn system(content: impl AsRef<str>) -> Option<Self> {
        Self::new("system", content)
    }

    pub fn user(content: impl AsRef<str>) -> Option<Self> {
        Self::new("user", content)
    }

    pub fn into_message(self) -> Value {
        serde_json::json!({
            "role": self.role,
            "content": self.content,
        })
    }
}

pub fn append_tail_prompt(messages: &mut Vec<Value>, prompt: Option<TailPrompt>) {
    if let Some(prompt) = prompt {
        messages.push(prompt.into_message());
    }
}

pub fn append_tail_message(messages: &mut Vec<Value>, message: Value) {
    if !message.is_null() {
        messages.push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::{append_tail_prompt, TailPrompt};

    #[test]
    fn tail_prompt_preserves_requested_role() {
        let prompt = TailPrompt::new("user", "compact now").expect("prompt");

        let message = prompt.into_message();

        assert_eq!(message["role"], "user");
        assert_eq!(message["content"], "compact now");
    }

    #[test]
    fn append_tail_prompt_appends_to_end_and_skips_empty_content() {
        let mut messages = vec![
            serde_json::json!({"role": "system", "content": "fixed prefix"}),
            serde_json::json!({"role": "user", "content": "task"}),
        ];

        append_tail_prompt(&mut messages, TailPrompt::system("tail instruction"));
        append_tail_prompt(&mut messages, TailPrompt::system("  "));

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["content"], "fixed prefix");
        assert_eq!(messages[2]["role"], "system");
        assert_eq!(messages[2]["content"], "tail instruction");
    }
}
