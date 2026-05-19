pub mod runtime_fallback;
pub mod task_continuity;
pub mod task_delivered;
pub mod tool_progress;
pub mod user_new_command;

#[derive(Debug, Default)]
pub struct PromptBuilder {
    parts: Vec<String>,
}

impl PromptBuilder {
    pub fn new() -> Self {
        Self { parts: Vec::new() }
    }

    pub fn part(mut self, content: impl AsRef<str>) -> Self {
        let content = content.as_ref().trim();
        if !content.is_empty() {
            self.parts.push(content.to_string());
        }
        self
    }

    pub fn section(mut self, name: &str, value: impl AsRef<str>) -> Self {
        let value = value.as_ref().trim();
        if !value.is_empty() {
            self.parts.push(format!("{name}:\n{value}"));
        }
        self
    }

    pub fn optional_section(self, name: &str, value: Option<&str>) -> Self {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            Some(value) => self.section(name, value),
            None => self,
        }
    }

    pub fn render(self) -> String {
        self.parts.join("\n\n")
    }
}
