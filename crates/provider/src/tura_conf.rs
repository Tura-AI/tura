use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, RwLockReadGuard};

use dotenvy::{from_path_iter, vars};
use tracing::{debug, warn};

#[derive(Debug)]
pub struct TuraConfig {
    env_path: PathBuf,
    values: Arc<RwLock<HashMap<String, String>>>,
}

impl TuraConfig {
    pub fn new(env_file: &str) -> Self {
        let project_root = runtime_project_root().unwrap_or_else(|| {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            manifest_dir
                .parent()
                .and_then(Path::parent)
                .map(Path::to_path_buf)
                .unwrap_or(manifest_dir)
        });

        let env_path = if let Ok(from_env) = env::var("TURA_ENV_PATH") {
            let path = PathBuf::from(from_env);
            if path.exists() {
                path
            } else {
                project_root.join(env_file)
            }
        } else {
            project_root.join(env_file)
        };

        let this = Self {
            env_path,
            values: Arc::new(RwLock::new(HashMap::new())),
        };
        this.reload();
        this
    }

    pub fn reload(&self) {
        let values = self.load_values();
        let mut guard = match self.values.write() {
            Ok(guard) => guard,
            Err(err) => {
                warn!("configuration cache lock was poisoned while reloading env file");
                err.into_inner()
            }
        };
        *guard = values;
    }

    fn load_values(&self) -> HashMap<String, String> {
        let mut values = HashMap::new();
        if self.env_path.exists() {
            match from_path_iter(&self.env_path) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok((key, value)) => {
                                values.insert(key, value);
                            }
                            Err(err) => {
                                warn!(error = %err, path = %self.env_path.display(), "failed to parse env entry");
                            }
                        }
                    }
                    debug!(path = %self.env_path.display(), "configuration loaded");
                }
                Err(err) => {
                    warn!(error = %err, path = %self.env_path.display(), "failed to load env file");
                }
            }
        } else {
            debug!(path = %self.env_path.display(), "root dotenv not found");
        }

        values.extend(vars());
        values
    }

    fn read_values(&self) -> RwLockReadGuard<'_, HashMap<String, String>> {
        match self.values.read() {
            Ok(guard) => guard,
            Err(err) => {
                warn!("configuration cache lock was poisoned while reading env values");
                err.into_inner()
            }
        }
    }

    fn values_snapshot(&self) -> HashMap<String, String> {
        self.read_values().clone()
    }

    pub fn env_values(&self) -> HashMap<String, String> {
        self.values_snapshot()
    }

    pub fn env_path(&self) -> &Path {
        &self.env_path
    }

    pub fn get_available_keys(&self) -> Vec<String> {
        self.read_values()
            .iter()
            .filter_map(|(k, v)| {
                if v.trim().len() > 1 {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn get_all_keys(&self) -> Vec<String> {
        self.read_values().keys().cloned().collect()
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let upper = key.to_uppercase();
        env::var(&upper)
            .ok()
            .or_else(|| self.read_values().get(&upper).cloned())
    }

    pub fn docker_run(&self) -> bool {
        let val = self
            .get("DOCKER_RUN")
            .unwrap_or_else(|| "false".to_string())
            .trim()
            .to_ascii_lowercase();
        matches!(val.as_str(), "1" | "true" | "yes")
    }

    pub fn require(&self, name: &str) -> Result<String, crate::tura_llm::TuraError> {
        self.get(name)
            .ok_or_else(|| crate::tura_llm::TuraError::Config {
                message: format!(
                    "Configuration key '{}' not found (checked path: {})",
                    name.to_uppercase(),
                    self.env_path.display()
                ),
            })
    }
}

impl Clone for TuraConfig {
    fn clone(&self) -> Self {
        Self {
            env_path: self.env_path.clone(),
            values: Arc::new(RwLock::new(self.values_snapshot())),
        }
    }
}

fn runtime_project_root() -> Option<PathBuf> {
    if let Ok(root) = env::var("TURA_PROJECT_ROOT") {
        let root = PathBuf::from(root);
        if root.exists() {
            return Some(root);
        }
    }
    let exe = env::current_exe().ok()?;
    let bin_dir = exe.parent()?;
    if bin_dir.join("agents").join("src").is_dir() || bin_dir.join("config").is_dir() {
        return Some(bin_dir.to_path_buf());
    }
    bin_dir.parent().map(Path::to_path_buf)
}

impl Default for TuraConfig {
    fn default() -> Self {
        Self::new(".env")
    }
}

#[cfg(test)]
mod tests {
    use super::TuraConfig;
    use std::ffi::OsString;

    struct EnvRestore {
        key: &'static str,
        value: Option<OsString>,
    }

    impl EnvRestore {
        fn remove(key: &'static str) -> Self {
            let value = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, value }
        }

        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self {
                key,
                value: previous,
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            if let Some(value) = &self.value {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn get_reads_uppercase_environment_keys() {
        std::env::set_var("TURA_CONF_TEST_KEY", "configured");
        let conf = TuraConfig::new(".env.missing-for-test");

        assert_eq!(
            conf.get("tura_conf_test_key").as_deref(),
            Some("configured")
        );

        std::env::remove_var("TURA_CONF_TEST_KEY");
    }

    #[test]
    fn reload_refreshes_cached_dotenv_values() {
        let _key = EnvRestore::remove("TURA_CONF_RELOAD_TEST_KEY");
        let temp = tempfile::tempdir().expect("temp env dir");
        let env_path = temp.path().join(".env");
        std::fs::write(&env_path, "TURA_CONF_RELOAD_TEST_KEY=first\n").expect("write first dotenv");
        let env_path = env_path.to_string_lossy().to_string();
        let _env_path = EnvRestore::set("TURA_ENV_PATH", &env_path);
        let conf = TuraConfig::new(".env.missing-for-test");

        assert_eq!(
            conf.get("TURA_CONF_RELOAD_TEST_KEY").as_deref(),
            Some("first")
        );

        std::fs::write(&env_path, "TURA_CONF_RELOAD_TEST_KEY=second\n")
            .expect("write second dotenv");
        assert_eq!(
            conf.get("TURA_CONF_RELOAD_TEST_KEY").as_deref(),
            Some("first")
        );

        conf.reload();

        assert_eq!(
            conf.get("TURA_CONF_RELOAD_TEST_KEY").as_deref(),
            Some("second")
        );
    }

    #[test]
    fn require_reports_checked_env_path_when_missing() {
        let _env_path = EnvRestore::remove("TURA_ENV_PATH");
        let _project_root = EnvRestore::remove("TURA_PROJECT_ROOT");
        let conf = TuraConfig::new(".env.missing-for-test");
        let err = conf
            .require("definitely_missing_tura_key")
            .expect_err("missing key should error");

        assert!(err.to_string().contains("DEFINITELY_MISSING_TURA_KEY"));
        assert!(err.to_string().contains(".env.missing-for-test"));
    }
}
