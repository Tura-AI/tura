use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use router_contract::{CommandSpec, ExecuteCommandRequest as RouterExecuteCommandRequest};

use crate::contracts::{Command, ExecuteCommandRequest, ExecuteCommandResponse};
use crate::mock::global_store;
use crate::router_client::RouterClient;

pub async fn list_commands() -> Response {
    let request = router_contract::ListCommandsRequest {
        directory: global_store().get_current_directory(),
    };
    match RouterClient::global().list_commands(request) {
        Ok(response) => Json(
            response
                .commands
                .into_iter()
                .map(command_from_router)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => router_failure(error),
    }
}

pub async fn execute_command(Json(payload): Json<ExecuteCommandRequest>) -> Response {
    let request = RouterExecuteCommandRequest {
        directory: global_store().get_current_directory(),
        command: payload.command,
        args: payload.args,
    };
    match RouterClient::global().execute_command(request) {
        Ok(response) => Json(ExecuteCommandResponse {
            output: response.output,
        })
        .into_response(),
        Err(error) => router_failure(error),
    }
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

fn router_failure(error: anyhow::Error) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        format!("router registry request failed: {error}"),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_projection_preserves_the_external_http_shape() {
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
}
