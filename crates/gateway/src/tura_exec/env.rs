use std::path::{Path, PathBuf};

use super::cli::CliConfig;

pub(crate) fn configure_runtime_env(config: &CliConfig) {
    std::env::set_var("TURA_FRONTEND_SOURCE", "cli");
    if let Some(model) = config.model.as_deref() {
        std::env::set_var("TURA_SESSION_MODEL_OVERRIDE", normalize_model(model));
    }
    if let Some(reasoning) = config.reasoning_effort.as_deref() {
        std::env::set_var("TURA_SESSION_REASONING_EFFORT", reasoning);
    }
    if config.priority {
        std::env::set_var("TURA_SESSION_ACCELERATION_ENABLED", "1");
    }
    if let Some(max_tokens) = config.max_tokens {
        std::env::set_var("TURA_SESSION_MAX_TOKENS", max_tokens.to_string());
    }
    if let Some(shell) = config.command_run_shell.as_deref() {
        std::env::set_var("TURA_COMMAND_RUN_SHELL", shell);
    }
    if config.command_run_sandbox {
        std::env::set_var("TURA_COMMAND_RUN_SANDBOX", "1");
    } else {
        std::env::remove_var("TURA_COMMAND_RUN_SANDBOX");
    }
    configure_release_runtime_env();
    configure_progress_env(config);
}

pub(crate) fn normalize_model(model: &str) -> String {
    if model.contains('/') {
        model.to_string()
    } else {
        format!("openai/{model}")
    }
}

fn configure_progress_env(config: &CliConfig) {
    if config.json {
        std::env::set_var("TURA_CLI_LIVE_JSONL", "1");
        std::env::remove_var("TURA_CLI_PROGRESS");
    } else {
        std::env::remove_var("TURA_CLI_LIVE_JSONL");
        if config.quiet {
            std::env::remove_var("TURA_CLI_PROGRESS");
        } else {
            std::env::set_var("TURA_CLI_PROGRESS", "1");
        }
    }
}

fn project_root_from_exe() -> String {
    std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(find_project_root_from)
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .as_deref()
                .and_then(find_project_root_from)
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .display()
        .to_string()
}

fn configure_release_runtime_env() {
    let root = project_root_from_exe();
    std::env::set_var("TURA_PROJECT_ROOT", &root);
    if std::env::var_os("TURA_PROVIDER_CONFIG").is_none() {
        let provider_config = PathBuf::from(&root)
            .join("config")
            .join("provider_config.json");
        if provider_config.exists() {
            std::env::set_var("TURA_PROVIDER_CONFIG", provider_config);
        }
    }
    if std::env::var_os("TURA_ENV_PATH").is_none() {
        let env_path = PathBuf::from(&root).join(".env");
        if env_path.exists() {
            std::env::set_var("TURA_ENV_PATH", env_path);
        }
    }
}

fn find_project_root_from(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    start
        .ancestors()
        .find(|candidate| {
            candidate.join("agents").join("src").is_dir()
                && (candidate.join("personas").join("src").is_dir()
                    || candidate
                        .join("config")
                        .join("provider_config.json")
                        .exists())
        })
        .map(Path::to_path_buf)
}
