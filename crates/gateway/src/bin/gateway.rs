use std::path::{Path, PathBuf};

fn main() {
    configure_release_runtime_env();

    if std::env::args().nth(1).as_deref() == Some("session-log") {
        run_session_log_command();
        return;
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("gateway tokio runtime should start");

    if std::env::var("TURA_ROLE").ok().as_deref() == Some("runtime_worker") {
        runtime.block_on(async {
            if let Err(error) = tokio::task::spawn_blocking(gateway::runtime_worker::run)
                .await
                .expect("runtime worker task panicked")
            {
                eprintln!("runtime worker exited with error: {error}");
                std::process::exit(1);
            }
        });
        return;
    }

    if std::env::var("OPENAI_LOGIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        std::env::set_var("OPENAI_LOGIN", "oauth");
    }

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "4096".to_string())
        .parse::<u16>()
        .unwrap_or(4096);

    runtime.block_on(async {
        if let Err(error) = gateway::router_process::start_global_router_process() {
            eprintln!("gateway failed to start persistent router: {error:#}");
            std::process::exit(1);
        }
        gateway::web::server::run_server(port)
            .await
            .expect("Server error");
    });
}

fn configure_release_runtime_env() {
    let root = project_root_from_exe();
    if std::env::var_os("TURA_PROJECT_ROOT").is_none() {
        std::env::set_var("TURA_PROJECT_ROOT", &root);
    }
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

fn project_root_from_exe() -> PathBuf {
    std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(find_release_root_from)
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .as_deref()
                .and_then(find_release_root_from)
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn find_release_root_from(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    start
        .ancestors()
        .find(|candidate| {
            candidate.join("agents").join("src").is_dir()
                || candidate.join("personas").join("src").is_dir()
                || candidate
                    .join("config")
                    .join("provider_config.json")
                    .exists()
        })
        .map(Path::to_path_buf)
}

fn run_session_log_command() {
    let result = {
        use std::io::Read;
        let mut raw = String::new();
        std::io::stdin()
            .read_to_string(&mut raw)
            .map_err(anyhow::Error::from)
            .and_then(|_| serde_json::from_str(raw.trim()).map_err(anyhow::Error::from))
            .and_then(|command| {
                gateway::session_db_client::SessionDbClient::discover()?.call(command)
            })
    };
    match result {
        Ok(response) => println!(
            "{}",
            serde_json::to_string(&response).expect("session_log response should serialize")
        ),
        Err(error) => {
            eprintln!("session-log session_db command failed: {error:#}");
            std::process::exit(1);
        }
    }
}
