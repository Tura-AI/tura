#[tokio::main]
async fn main() {
    if std::env::var("TURA_ROLE").ok().as_deref() == Some("runtime_worker") {
        if let Err(error) = tokio::task::spawn_blocking(gateway::runtime_worker::run)
            .await
            .expect("runtime worker task panicked")
        {
            eprintln!("runtime worker exited with error: {error}");
            std::process::exit(1);
        }
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

    gateway::web::server::run_server(port)
        .await
        .expect("Server error");
}
