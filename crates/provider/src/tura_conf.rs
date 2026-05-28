use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use dotenvy::{dotenv_override, from_path_override, vars};
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct TuraConfig {
    env_path: PathBuf,
    values: HashMap<String, String>,
}

impl TuraConfig {
    pub fn new(env_file: &str) -> Self {
        let current_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src");

        let env_path = if let Ok(from_env) = env::var("TURA_ENV_PATH") {
            let path = PathBuf::from(from_env);
            if path.exists() {
                path
            } else {
                current_dir.parent().unwrap_or(&current_dir).join(env_file)
            }
        } else {
            let candidate1 = current_dir.join(env_file);
            let candidate2 = current_dir.parent().unwrap_or(&current_dir).join(env_file);
            let candidate3 = current_dir
                .parent()
                .and_then(Path::parent)
                .unwrap_or(&current_dir)
                .join(env_file);
            if candidate1.exists() {
                candidate1
            } else if candidate2.exists() {
                candidate2
            } else {
                candidate3
            }
        };

        let mut this = Self {
            env_path,
            values: HashMap::new(),
        };
        this.load();
        this
    }

    fn load(&mut self) {
        if self.env_path.exists() {
            if let Err(err) = from_path_override(&self.env_path) {
                warn!(error = %err, path = %self.env_path.display(), "failed to load env file");
            } else {
                debug!(path = %self.env_path.display(), "configuration loaded");
            }
        } else if let Err(err) = dotenv_override() {
            debug!(error = %err, "default dotenv not loaded");
        } else {
            debug!("configuration loaded from process cwd .env");
        }

        self.values = vars().collect();
    }

    pub fn env_path(&self) -> &Path {
        &self.env_path
    }

    pub fn get_available_keys(&self) -> Vec<String> {
        self.values
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
        self.values.keys().cloned().collect()
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let upper = key.to_uppercase();
        env::var(&upper)
            .ok()
            .or_else(|| self.values.get(&upper).cloned())
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

impl Default for TuraConfig {
    fn default() -> Self {
        Self::new(".env")
    }
}

#[cfg(test)]
mod tests {
    use super::TuraConfig;

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
    fn require_reports_checked_env_path_when_missing() {
        let conf = TuraConfig::new(".env.missing-for-test");
        let err = conf
            .require("definitely_missing_tura_key")
            .expect_err("missing key should error");

        assert!(err.to_string().contains("DEFINITELY_MISSING_TURA_KEY"));
        assert!(err.to_string().contains(".env.missing-for-test"));
    }
}
