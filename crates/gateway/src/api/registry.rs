use std::path::PathBuf;

pub(crate) fn project_root() -> PathBuf {
    project_root_for_router_cli()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn project_root_for_router_cli() -> Option<String> {
    if let Ok(root) = std::env::var("TURA_PROJECT_ROOT") {
        let root = PathBuf::from(root);
        if root.exists() {
            return Some(root.display().to_string());
        }
    }
    std::env::current_dir().ok().and_then(|current| {
        current
            .ancestors()
            .find(|candidate| {
                (candidate.join("Cargo.toml").exists() && candidate.join("crates").exists())
                    || (candidate.join("agents").join("src").is_dir()
                        && candidate.join("personas").join("src").is_dir())
            })
            .map(|path| path.display().to_string())
    })
}
