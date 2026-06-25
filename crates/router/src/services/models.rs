use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallContext {
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub input: Value,
}

impl CallContext {
    #[allow(dead_code)]
    pub fn new(method: String, path: String, input: Value) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            method,
            path,
            input,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerEnvelope {
    pub kind: String,
    pub payload: Value,
}

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
    use super::{CallContext, WorkerEnvelope, WorkerSpec};
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn call_context_new_generates_request_id_and_preserves_payload() {
        let input = json!({ "prompt": "hello", "turn_id": "turn-1" });

        let ctx = CallContext::new(
            "runtime.run".to_string(),
            "/runtime".to_string(),
            input.clone(),
        );

        assert!(!ctx.request_id.trim().is_empty());
        assert_eq!(ctx.method, "runtime.run");
        assert_eq!(ctx.path, "/runtime");
        assert_eq!(ctx.input, input);
        assert!(
            uuid::Uuid::parse_str(&ctx.request_id).is_ok(),
            "request id should be a UUID: {}",
            ctx.request_id
        );
    }

    #[test]
    fn call_context_deserializes_explicit_request_id_for_ipc_replay() {
        let ctx: CallContext = serde_json::from_value(json!({
            "request_id": "request-123",
            "method": "execution.cancel",
            "path": "/execution/cancel",
            "input": { "session_id": "session" }
        }))
        .expect("call context json");

        assert_eq!(ctx.request_id, "request-123");
        assert_eq!(ctx.method, "execution.cancel");
        assert_eq!(ctx.input["session_id"], "session");
    }

    #[test]
    fn worker_envelope_uses_kind_and_payload_fields_only() {
        let envelope = WorkerEnvelope {
            kind: "call".to_string(),
            payload: json!({ "input": { "method": "run" } }),
        };

        let encoded = serde_json::to_value(&envelope).expect("envelope json");
        assert_eq!(encoded["kind"], "call");
        assert_eq!(encoded["payload"]["input"]["method"], "run");
        assert!(encoded.get("request_id").is_none());

        let decoded: WorkerEnvelope = serde_json::from_value(encoded).expect("round trip");
        assert_eq!(decoded.kind, "call");
        assert_eq!(decoded.payload["input"]["method"], "run");
    }

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
