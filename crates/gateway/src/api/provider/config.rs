use std::fs;
use std::io;
use std::path::{Path as FsPath, PathBuf};

pub(crate) fn provider_config_path() -> PathBuf {
    std::env::var("TURA_PROVIDER_CONFIG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| runtime_root().and_then(provider_config_in_root))
        .unwrap_or_else(|| {
            source_provider_config_in_root(&std::env::current_dir().unwrap_or_default())
        })
}

fn runtime_root() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("TURA_PROJECT_ROOT")
        .map(PathBuf::from)
        .filter(|path| path.exists())
    {
        return Some(root);
    }
    std::env::current_dir().ok().and_then(|current| {
        current
            .ancestors()
            .find(|candidate| {
                candidate
                    .join("config")
                    .join("provider_config.json")
                    .is_file()
                    || candidate
                        .join("crates")
                        .join("provider")
                        .join("config")
                        .join("provider_config.json")
                        .is_file()
            })
            .map(FsPath::to_path_buf)
    })
}

fn provider_config_in_root(root: PathBuf) -> Option<PathBuf> {
    let release_config = root.join("config").join("provider_config.json");
    if release_config.is_file() {
        return Some(release_config);
    }
    let source_config = source_provider_config_in_root(&root);
    source_config.is_file().then_some(source_config)
}

fn source_provider_config_in_root(root: &FsPath) -> PathBuf {
    root.join("crates")
        .join("provider")
        .join("config")
        .join("provider_config.json")
}

pub(super) fn config_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            tura_llm_rust::TuraConfig::default()
                .get(key)
                .filter(|value| !value.trim().is_empty())
        })
}

pub(super) fn upsert_env_value(path: &FsPath, key: &str, value: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut lines = fs::read_to_string(path)
        .map(|content| content.lines().map(ToString::to_string).collect::<Vec<_>>())
        .unwrap_or_default();
    let prefix = format!("{key}=");
    let next = format!("{key}={}", quote_env_value(value));
    let mut replaced = false;
    for line in &mut lines {
        if line.trim_start().starts_with(&prefix) {
            *line = next.clone();
            replaced = true;
            break;
        }
    }
    if !replaced {
        lines.push(next);
    }
    let mut content = lines.join("\n");
    content.push('\n');
    fs::write(path, content)
}

fn quote_env_value(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::provider_config_path;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn provider_config_path_uses_release_project_root_config() {
        let _guard = ENV_LOCK.lock().expect("provider config env lock");
        let _provider = EnvRestore::remove("TURA_PROVIDER_CONFIG");
        let project = temp_root("release-provider-config");
        let config = project.join("config").join("provider_config.json");
        std::fs::create_dir_all(config.parent().expect("config parent")).expect("config dir");
        std::fs::write(&config, "{}").expect("provider config");
        let _project = EnvRestore::set("TURA_PROJECT_ROOT", project.as_os_str());

        assert_eq!(provider_config_path(), config);

        let _ = std::fs::remove_dir_all(project);
    }

    fn temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("tura-{name}-{suffix}"))
    }

    struct EnvRestore {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvRestore {
        fn remove(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, previous }
        }

        fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match self.previous.as_ref() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}
