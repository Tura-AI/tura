#[derive(Debug, Clone)]
pub struct WorkerHandle {
    pub worker_id: String,
}

/// Declarative worker description: executable, startup arguments, and env.
/// Workers are reused by key and can be replaced after liveness checks fail.
#[derive(Debug, Clone)]
pub struct WorkerSpec {
    /// Reuse key, usually at session or agent scope.
    pub key: String,
    /// Logical name used in status output and URLs.
    pub service_name: String,
    pub executable: std::path::PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::WorkerSpec;
    use std::path::PathBuf;

    #[test]
    fn worker_spec_clone_preserves_reuse_key_executable_args_and_env() {
        let spec = WorkerSpec {
            key: "runtime_worker:session".to_string(),
            service_name: "runtime".to_string(),
            executable: PathBuf::from("target/debug/tura_runtime"),
            args: vec!["--serve-worker".to_string(), "--stdio".to_string()],
            env: vec![
                ("TURA_HOME".to_string(), "target/test-home".to_string()),
                ("TURA_DEBUG_RUNTIME".to_string(), "1".to_string()),
            ],
        };

        let cloned = spec.clone();
        assert_eq!(cloned.key, spec.key);
        assert_eq!(cloned.service_name, spec.service_name);
        assert_eq!(cloned.executable, spec.executable);
        assert_eq!(cloned.args, spec.args);
        assert_eq!(cloned.env, spec.env);
    }
}
