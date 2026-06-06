use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Read;
use std::path::Path;

pub const PROMPT: &str = include_str!("prompt.md");
pub const POLICY: &str = include_str!("policy.toml");
pub const SCHEMA: &str = include_str!("schema.json");

#[path = "src/access.rs"]
mod access_control;
#[path = "src/args.rs"]
mod args;
#[path = "src/config.rs"]
mod config;
#[path = "src/document.rs"]
mod document;
#[path = "src/media_image.rs"]
mod media_image;
#[path = "src/output.rs"]
mod output;
#[path = "src/paths.rs"]
mod paths;
#[path = "src/pdf.rs"]
mod pdf;
#[path = "src/previews.rs"]
mod previews;
#[path = "src/processing.rs"]
mod processing;
#[path = "src/runner.rs"]
mod runner;
#[path = "src/types.rs"]
mod types;
#[path = "src/video.rs"]
mod video;

use access_control::access_for_value;
use args::{parse_args_text, parse_args_value};
use output::summary_text;
use paths::workspace_relative_path;
use runner::run_read_media;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Access {
    pub read_paths: Vec<String>,
    pub write_paths: Vec<String>,
    pub workspace_write: bool,
}

pub mod runtime {
    pub mod file_locks {
        pub use crate::Access;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandResponse {
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub output: serde_json::Value,
    pub changes: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Envelope {
    kind: String,
    #[serde(default)]
    payload: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProtocolResponse {
    ok: bool,
    #[serde(default)]
    output: serde_json::Value,
    #[serde(default)]
    success: bool,
    #[serde(default)]
    stderr: String,
    #[serde(default)]
    exit_code: i32,
}

pub fn execute(command_line: &str, session_dir: &Path) -> CommandResponse {
    match run_read_media(parse_args_text(command_line), session_dir) {
        Ok(output) => CommandResponse {
            success: true,
            exit_code: 0,
            stdout: summary_text(&output),
            stderr: String::new(),
            output,
            changes: Vec::new(),
        },
        Err(err) => CommandResponse {
            success: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: err.clone(),
            output: json!({ "error": err }),
            changes: Vec::new(),
        },
    }
}

pub fn access(command_line: &str, session_dir: &Path) -> Access {
    let Ok(args) = parse_args_text(command_line) else {
        return Access::default();
    };
    Access {
        read_paths: args
            .paths
            .iter()
            .filter_map(|path| workspace_relative_path(path, session_dir))
            .map(|path| path.display().to_string())
            .collect(),
        ..Access::default()
    }
}

pub fn main() {
    let mut input = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut input) {
        print_response(protocol_error(format!("failed to read request: {error}")));
        return;
    }
    let envelope: Envelope = match serde_json::from_str(&input) {
        Ok(value) => value,
        Err(error) => {
            print_response(protocol_error(format!("invalid request JSON: {error}")));
            return;
        }
    };
    print_response(handle_envelope(envelope));
}

fn handle_envelope(envelope: Envelope) -> ProtocolResponse {
    match envelope.kind.as_str() {
        "health_check" => ProtocolResponse {
            ok: true,
            output: json!({ "status": "ok" }),
            success: true,
            stderr: String::new(),
            exit_code: 0,
        },
        "capabilities" => ProtocolResponse {
            ok: true,
            output: json!({
                "id": "read_media",
                "supports_macro_command": true,
                "mutating": false,
            }),
            success: true,
            stderr: String::new(),
            exit_code: 0,
        },
        "access" => {
            let session_dir = payload_session_dir(&envelope.payload);
            let output = match envelope
                .payload
                .get("arguments")
                .cloned()
                .unwrap_or_default()
            {
                serde_json::Value::String(input) => {
                    serde_json::to_value(access(&input, &session_dir))
                }
                value => serde_json::to_value(access_for_value(&value, &session_dir)),
            }
            .unwrap_or_else(|error| json!({ "error": error.to_string() }));
            ProtocolResponse {
                ok: true,
                output,
                success: true,
                stderr: String::new(),
                exit_code: 0,
            }
        }
        "execute" => {
            let session_dir = payload_session_dir(&envelope.payload);
            let result = match envelope
                .payload
                .get("arguments")
                .cloned()
                .unwrap_or_default()
            {
                serde_json::Value::String(input) => execute(&input, &session_dir),
                value => match parse_args_value(value)
                    .and_then(|args| run_read_media(Ok(args), &session_dir))
                {
                    Ok(output) => CommandResponse {
                        success: true,
                        exit_code: 0,
                        stdout: summary_text(&output),
                        stderr: String::new(),
                        output,
                        changes: Vec::new(),
                    },
                    Err(error) => CommandResponse {
                        success: false,
                        exit_code: 1,
                        stdout: String::new(),
                        stderr: error.clone(),
                        output: json!({ "error": error }),
                        changes: Vec::new(),
                    },
                },
            };
            ProtocolResponse {
                ok: true,
                output: result.output,
                success: result.success,
                stderr: result.stderr,
                exit_code: result.exit_code,
            }
        }
        _ => protocol_error(format!("unsupported protocol kind: {}", envelope.kind)),
    }
}

fn payload_session_dir(payload: &serde_json::Value) -> std::path::PathBuf {
    payload
        .get("session_dir")
        .and_then(serde_json::Value::as_str)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        })
}

fn protocol_error(message: String) -> ProtocolResponse {
    ProtocolResponse {
        ok: false,
        output: json!({ "error": message }),
        success: false,
        stderr: String::new(),
        exit_code: 1,
    }
}

fn print_response(response: ProtocolResponse) {
    println!(
        "{}",
        serde_json::to_string(&response).unwrap_or_else(|error| {
            format!(
                r#"{{"ok":false,"output":{{"error":"failed to encode response: {error}"}},"success":false,"stderr":"","exit_code":1}}"#
            )
        })
    );
}
