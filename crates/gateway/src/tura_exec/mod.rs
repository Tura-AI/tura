mod cli;
mod env;
mod output;
mod router;
mod session;

use std::io::{self, Write};

use runtime::state_machine::session_management::SessionInput;
use serde_json::json;

use self::cli::{print_help, wants_help, CliConfig};
use self::env::configure_runtime_env;
use self::output::{
    emit_jsonl, final_message_text, write_jsonl, write_last_message, write_turn_log_stderr,
};
use self::router::run_via_router;
use self::session::{ensure_session_db_owner, reject_busy_session};

pub fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if wants_help(&args) {
        print_help();
        return Ok(0);
    }
    let config = CliConfig::parse(args)?;
    configure_runtime_env(&config);

    let prompt = config.prompt()?;
    let session_id = config
        .session_id
        .clone()
        .unwrap_or_else(|| format!("cli-{}", uuid::Uuid::new_v4()));
    if config.json {
        emit_jsonl(&json!({"type": "thread.started", "thread_id": session_id}))?;
        emit_jsonl(&json!({"type": "turn.started"}))?;
        io::stdout()
            .flush()
            .map_err(|err| format!("failed to flush stdout: {err}"))?;
    }

    // Default: thin client. Dispatch the turn to the detached `tura_router`
    // daemon (which owns session_db + spawns the runtime worker), then render
    // from the persisted session. The CLI links no runtime/DB executor.
    if !config.embedded {
        return run_via_router(&config, &session_id, &prompt);
    }

    // `--embedded`: in-process runtime (codex-style), still connected to the
    // per-home single session_db owner. The CLI never opens its own database.
    ensure_session_db_owner();
    std::env::set_var("TURA_GATEWAY_CALLBACKS", "0");
    std::env::set_var("TURA_RUNTIME_ERRORS_FATAL", "1");
    if config.session_id.is_some() {
        reject_busy_session(&session_id, config.json)?;
    }
    let result = runtime::mano::process_from_gateway_session_in_directory(
        session_id.clone(),
        SessionInput {
            user_input: prompt,
            file_input: Vec::new(),
            agent: config.agent.clone(),
            runtime_context: None,
            planning_mode_override: config.planning_mode,
        },
        config.cwd.clone(),
    )?;

    if let Some(path) = config.last_message_path.as_ref() {
        write_last_message(path, &final_message_text(&result.session.session_log))?;
    }

    if config.log {
        write_turn_log_stderr(
            &result.session.session_log,
            Some(result.session.session_started_at.timestamp_millis()),
        )?;
    }

    if config.json {
        write_jsonl(&result.session.session_log, &session_id, &config.cwd, false)?;
    } else {
        println!("{}", final_message_text(&result.session.session_log));
    }

    Ok(0)
}
