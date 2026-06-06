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
