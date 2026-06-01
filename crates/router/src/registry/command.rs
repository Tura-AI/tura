//! Command registry owned by the router.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct CommandRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub source: String,
    pub template: Option<String>,
    pub subtask: bool,
    pub hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteCommandRequest {
    pub command: String,
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteCommandResponse {
    pub output: String,
}

impl CommandRegistry {
    pub fn list(&self, directory: Option<&str>) -> Vec<CommandSpec> {
        discover_commands(directory)
    }

    pub fn execute(
        &self,
        directory: Option<&str>,
        payload: ExecuteCommandRequest,
    ) -> ExecuteCommandResponse {
        let command_name = payload.command.trim().trim_start_matches('/').to_string();
        let command = discover_commands(directory)
            .into_iter()
            .find(|command| command.name == command_name);
        let output = match command.and_then(|command| command.template) {
            Some(template) => render_command_template(&template, payload.args.as_deref().unwrap_or_default()),
            None => format!(
                "Command `{}` is not configured. Add a markdown or JSON command under .tura/commands, .opencode/command, .opencode/commands, command, or commands.",
                command_name
            ),
        };
        ExecuteCommandResponse { output }
    }
}

fn discover_commands(directory: Option<&str>) -> Vec<CommandSpec> {
    let mut commands = Vec::new();
    for directory in command_directories(directory) {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(command) = command_from_file(&path) {
                    commands.push(command);
                }
            }
        }
    }
    commands.sort_by(|left, right| left.name.cmp(&right.name));
    commands.dedup_by(|left, right| left.name == right.name);
    commands
}

fn command_directories(directory: Option<&str>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(directory) = directory.map(str::trim).filter(|value| !value.is_empty()) {
        roots.push(PathBuf::from(directory));
    }
    if let Ok(current_directory) = std::env::current_dir() {
        roots.push(current_directory);
    }

    let suffixes = [
        [".tura", "commands"].as_slice(),
        [".opencode", "command"].as_slice(),
        [".opencode", "commands"].as_slice(),
        ["command"].as_slice(),
        ["commands"].as_slice(),
    ];

    let mut directories = Vec::new();
    for root in roots {
        for suffix in suffixes {
            let mut directory = root.clone();
            for part in suffix {
                directory.push(part);
            }
            directories.push(directory);
        }
    }
    directories
}

fn command_from_file(path: &Path) -> Option<CommandSpec> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    if !matches!(extension.as_str(), "md" | "txt" | "json") {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    if extension == "json" {
        return command_from_json(path, &content);
    }
    let name = path.file_stem()?.to_string_lossy().to_string();
    let description = first_command_description(&content).unwrap_or_else(|| name.clone());
    Some(CommandSpec {
        name,
        description,
        agent: None,
        model: None,
        source: "command".to_string(),
        template: Some(content),
        subtask: false,
        hints: vec![],
    })
}

fn command_from_json(path: &Path, content: &str) -> Option<CommandSpec> {
    let value: serde_json::Value = serde_json::from_str(content).ok()?;
    let fallback_name = path.file_stem()?.to_string_lossy().to_string();
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&fallback_name)
        .to_string();
    let description = value
        .get("description")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("summary").and_then(serde_json::Value::as_str))
        .unwrap_or(&name)
        .to_string();
    Some(CommandSpec {
        name,
        description,
        agent: value
            .get("agent")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        model: value
            .get("model")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        source: "command".to_string(),
        template: value
            .get("template")
            .and_then(serde_json::Value::as_str)
            .or_else(|| value.get("prompt").and_then(serde_json::Value::as_str))
            .map(str::to_string),
        subtask: value
            .get("subtask")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        hints: value
            .get("hints")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn first_command_description(content: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.trim_start_matches('#').trim().to_string())
        .filter(|line| !line.is_empty())
}

fn render_command_template(template: &str, args: &[String]) -> String {
    let joined_args = args.join(" ");
    let mut output = template
        .replace("{{args}}", &joined_args)
        .replace("$ARGUMENTS", &joined_args)
        .replace("{args}", &joined_args);
    for (index, arg) in args.iter().enumerate() {
        output = output.replace(&format!("${}", index + 1), arg);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("tura-router-command-{name}-{suffix}"))
    }

    #[test]
    fn scans_markdown_commands_from_tura_directory() {
        let root = temp_root("markdown");
        let commands_dir = root.join(".tura").join("commands");
        fs::create_dir_all(&commands_dir).expect("create command directory");
        fs::write(
            commands_dir.join("hello.md"),
            "# Say hello\nHello $1 with $ARGUMENTS",
        )
        .expect("write markdown command");

        let registry = CommandRegistry;
        let commands = registry.list(root.to_str());
        let command = commands
            .iter()
            .find(|command| command.name == "hello")
            .expect("hello command should be discovered");
        assert_eq!(command.description, "Say hello");

        let response = registry.execute(
            root.to_str(),
            ExecuteCommandRequest {
                command: "/hello".to_string(),
                args: Some(vec!["Ada".to_string(), "Lovelace".to_string()]),
            },
        );
        assert!(response.output.contains("Hello Ada"));
        assert!(response.output.contains("Ada Lovelace"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scans_json_commands_from_opencode_directory() {
        let root = temp_root("json");
        let commands_dir = root.join(".opencode").join("commands");
        fs::create_dir_all(&commands_dir).expect("create command directory");
        fs::write(
            commands_dir.join("build.json"),
            serde_json::json!({
                "name": "build",
                "description": "Build project",
                "agent": "coding",
                "model": "gpt-5",
                "template": "Build {{args}}",
                "subtask": true,
                "hints": ["compile", "test"]
            })
            .to_string(),
        )
        .expect("write json command");

        let registry = CommandRegistry;
        let commands = registry.list(root.to_str());
        let command = commands
            .iter()
            .find(|command| command.name == "build")
            .expect("build command should be discovered");
        assert_eq!(command.description, "Build project");
        assert_eq!(command.agent.as_deref(), Some("coding"));
        assert_eq!(command.model.as_deref(), Some("gpt-5"));
        assert!(command.subtask);
        assert_eq!(command.hints, vec!["compile", "test"]);

        let response = registry.execute(
            root.to_str(),
            ExecuteCommandRequest {
                command: "build".to_string(),
                args: Some(vec!["workspace".to_string()]),
            },
        );
        assert_eq!(response.output, "Build workspace");

        let _ = fs::remove_dir_all(root);
    }
}
