use crate::api::types::Command;
use crate::mock::global_store;
use axum::Json;
use tura_router::registry::command::{
    CommandRegistry, CommandSpec, ExecuteCommandRequest as RouterExecuteCommandRequest,
};

pub async fn list_commands() -> Json<Vec<Command>> {
    let directory = global_store().get_current_directory();
    Json(
        CommandRegistry
            .list(directory.as_deref())
            .into_iter()
            .map(command_from_router)
            .collect(),
    )
}

pub async fn execute_command(
    Json(payload): Json<ExecuteCommandRequest>,
) -> Json<ExecuteCommandResponse> {
    let directory = global_store().get_current_directory();
    let response = CommandRegistry.execute(
        directory.as_deref(),
        RouterExecuteCommandRequest {
            command: payload.command,
            args: payload.args,
        },
    );
    Json(ExecuteCommandResponse {
        output: response.output,
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExecuteCommandRequest {
    pub command: String,
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ExecuteCommandResponse {
    pub output: String,
}

fn command_from_router(command: CommandSpec) -> Command {
    Command {
        name: command.name,
        description: command.description,
        agent: command.agent,
        model: command.model,
        source: command.source,
        template: command.template,
        subtask: command.subtask,
        hints: command.hints,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn command_from_router_preserves_all_router_fields() {
        let command = command_from_router(CommandSpec {
            name: "refactor".to_string(),
            description: "Refactor the selected module".to_string(),
            agent: Some("coding".to_string()),
            model: Some("gpt-5.5-codex".to_string()),
            source: ".tura/commands/refactor.md".to_string(),
            template: Some("Refactor {{args}}".to_string()),
            subtask: true,
            hints: vec!["requires-worktree".to_string(), "writes-files".to_string()],
        });

        assert_eq!(command.name, "refactor");
        assert_eq!(command.description, "Refactor the selected module");
        assert_eq!(command.agent.as_deref(), Some("coding"));
        assert_eq!(command.model.as_deref(), Some("gpt-5.5-codex"));
        assert_eq!(command.source, ".tura/commands/refactor.md");
        assert_eq!(command.template.as_deref(), Some("Refactor {{args}}"));
        assert!(command.subtask);
        assert_eq!(command.hints, vec!["requires-worktree", "writes-files"]);
    }

    #[tokio::test]
    async fn execute_command_reports_unknown_commands_without_panicking() {
        let response = execute_command(Json(ExecuteCommandRequest {
            command: "/missing-command".to_string(),
            args: Some(vec!["one".to_string(), "two".to_string()]),
        }))
        .await;

        assert!(response
            .output
            .contains("Command `missing-command` is not configured"));
        assert!(response.output.contains(".tura/commands"));
    }

    #[tokio::test]
    async fn list_and_execute_command_use_current_workspace_directory() {
        let temp = TempDir::new().expect("temp workspace");
        let commands_dir = temp.path().join(".tura").join("commands");
        std::fs::create_dir_all(&commands_dir).expect("create commands dir");
        std::fs::write(
            commands_dir.join("snake.md"),
            "Run snake task with {{args}} in this workspace.",
        )
        .expect("write command");
        let _directory_guard =
            CurrentDirectoryGuard::set(temp.path().to_string_lossy().to_string());

        let commands = list_commands().await;
        let snake = commands
            .iter()
            .find(|command| command.name == "snake")
            .expect("snake command should be discovered");
        assert_eq!(snake.source, "command");
        assert_eq!(
            snake.template.as_deref(),
            Some("Run snake task with {{args}} in this workspace.")
        );

        let response = execute_command(Json(ExecuteCommandRequest {
            command: "snake".to_string(),
            args: Some(vec!["debug".to_string(), "tui".to_string()]),
        }))
        .await;
        assert!(response.output.contains("Run snake task with"));
        assert!(response.output.contains("debug"));
        assert!(response.output.contains("tui"));
    }

    struct CurrentDirectoryGuard {
        previous: Option<String>,
    }

    impl CurrentDirectoryGuard {
        fn set(directory: String) -> Self {
            let previous = global_store().get_current_directory();
            global_store().set_current_directory(directory);
            Self { previous }
        }
    }

    impl Drop for CurrentDirectoryGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(directory) => global_store().set_current_directory(directory),
                None => global_store().clear_current_directory(),
            }
        }
    }
}
