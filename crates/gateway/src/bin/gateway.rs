fn main() {
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
        gateway::web::server::run_server(port)
            .await
            .expect("Server error");
    });
}

fn run_session_log_command() {
    let result = {
        use std::io::Read;
        let mut raw = String::new();
        std::io::stdin()
            .read_to_string(&mut raw)
            .map_err(anyhow::Error::from)
            .and_then(|_| tura_router::session_log_forward::handle_session_log_json(&raw))
    };
    match result {
        Ok(response) => println!(
            "{}",
            serde_json::to_string(&response).expect("session_log response should serialize")
        ),
        Err(error) => {
            eprintln!("session-log router command failed: {error:#}");
            std::process::exit(1);
        }
    }
}
