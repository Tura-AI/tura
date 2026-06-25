use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, RwLock};

use super::file_locks;

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub tool_name: String,
    pub call_id: String,
    pub payload: ToolPayload,
}

#[derive(Clone, Debug)]
pub enum ToolPayload {
    Function { arguments: Value },
    Freeform { input: String },
}

impl ToolPayload {
    pub fn code_mode_input(&self) -> Value {
        match self {
            Self::Function { arguments } => arguments.clone(),
            Self::Freeform { input } => Value::String(input.clone()),
        }
    }
}

#[derive(Clone)]
pub struct ToolContext {
    pub session_dir: PathBuf,
    pub cancellation: CancellationToken,
    pub execution_gate: Arc<RwLock<()>>,
    events: Arc<std::sync::Mutex<Vec<ToolRuntimeEvent>>>,
    hooks: Arc<std::sync::Mutex<ToolHooks>>,
    current_call_id: Option<String>,
    lock_scope: Option<String>,
}

impl ToolContext {
    pub fn new(session_dir: PathBuf) -> Self {
        Self::new_with_lock_scope(session_dir, None)
    }

    pub fn new_with_lock_scope(session_dir: PathBuf, lock_scope: Option<String>) -> Self {
        Self {
            session_dir,
            cancellation: CancellationToken::new(),
            execution_gate: Arc::new(RwLock::new(())),
            events: Arc::new(std::sync::Mutex::new(Vec::new())),
            hooks: Arc::new(std::sync::Mutex::new(ToolHooks::default())),
            current_call_id: None,
            lock_scope: normalize_lock_scope(lock_scope),
        }
    }

    pub fn child(&self) -> Self {
        Self {
            session_dir: self.session_dir.clone(),
            cancellation: self.cancellation.child_token(),
            execution_gate: Arc::clone(&self.execution_gate),
            events: Arc::clone(&self.events),
            hooks: Arc::clone(&self.hooks),
            current_call_id: self.current_call_id.clone(),
            lock_scope: self.lock_scope.clone(),
        }
    }

    pub fn with_call_id(&self, call_id: String) -> Self {
        let mut ctx = self.child();
        ctx.current_call_id = Some(call_id);
        ctx
    }

    pub fn current_call_id(&self) -> Option<&str> {
        self.current_call_id.as_deref()
    }

    pub fn lock_scope(&self) -> Option<&str> {
        self.lock_scope.as_deref()
    }

    pub fn record_event(&self, event: ToolRuntimeEvent) {
        let mut events = self.events.lock().unwrap_or_else(|err| err.into_inner());
        events.push(event);
    }

    pub fn events(&self) -> Vec<ToolRuntimeEvent> {
        self.events
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    pub fn set_pre_hook<F>(&self, hook: F)
    where
        F: Fn(&ToolCall) -> Result<(), ToolError> + Send + Sync + 'static,
    {
        let mut hooks = self.hooks.lock().unwrap_or_else(|err| err.into_inner());
        hooks.pre = Some(Arc::new(hook));
    }

    pub fn set_post_hook<F>(&self, hook: F)
    where
        F: Fn(&ToolCall, &mut FunctionToolOutput) -> Result<(), ToolError> + Send + Sync + 'static,
    {
        let mut hooks = self.hooks.lock().unwrap_or_else(|err| err.into_inner());
        hooks.post = Some(Arc::new(hook));
    }

    fn hooks(&self) -> ToolHooks {
        self.hooks
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }
}

fn normalize_lock_scope(lock_scope: Option<String>) -> Option<String> {
    lock_scope
        .map(|scope| scope.trim().to_string())
        .filter(|scope| !scope.is_empty())
}

#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    notify: Arc<Notify>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn child_token(&self) -> Self {
        self.clone()
    }

    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        self.notify.notified().await;
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct FunctionToolOutput {
    pub body: Value,
    pub success: Option<bool>,
}

impl FunctionToolOutput {
    pub fn from_value(body: Value, success: Option<bool>) -> Self {
        Self { body, success }
    }

    pub fn success_for_logging(&self) -> bool {
        self.success.unwrap_or(true)
    }

    pub fn code_mode_result(&self) -> Value {
        self.body.clone()
    }
}

#[derive(Clone, Debug)]
pub struct AnyToolResult {
    pub call_id: String,
    pub payload: ToolPayload,
    pub result: FunctionToolOutput,
}

#[derive(Debug)]
pub enum ToolError {
    Fatal(String),
    RespondToModel(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fatal(message) | Self::RespondToModel(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ToolError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolRuntimeEvent {
    ToolStarted {
        call_id: String,
        tool_name: String,
    },
    OutputDelta {
        call_id: String,
        stream: String,
        text: String,
    },
    ToolFinished {
        call_id: String,
        tool_name: String,
        success: bool,
    },
}

type PreToolHook = Arc<dyn Fn(&ToolCall) -> Result<(), ToolError> + Send + Sync>;
type PostToolHook =
    Arc<dyn Fn(&ToolCall, &mut FunctionToolOutput) -> Result<(), ToolError> + Send + Sync>;

#[derive(Clone, Default)]
struct ToolHooks {
    pre: Option<PreToolHook>,
    post: Option<PostToolHook>,
}

#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync {
    fn tool_name(&self) -> &str;

    fn supports_macro_command(&self) -> bool {
        false
    }

    async fn is_mutating(&self, call: &ToolCall, ctx: &ToolContext) -> bool;

    async fn access(&self, call: &ToolCall, ctx: &ToolContext) -> file_locks::Access {
        if self.is_mutating(call, ctx).await {
            file_locks::Access {
                workspace_write: true,
                ..file_locks::Access::default()
            }
        } else {
            file_locks::Access::default()
        }
    }

    async fn handle(
        &self,
        call: ToolCall,
        ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError>;
}

pub struct CommandRouter {
    repo_root: Option<PathBuf>,
    shell: crate::commands::shell_command::ShellCommandHandler,
    bash: crate::commands::bash::BashHandler,
    zsh: crate::commands::zsh::ZshHandler,
    apply_patch: crate::commands::apply_patch::ApplyPatchHandler,
    planning: crate::commands::planning::PlanningHandler,
}

impl CommandRouter {
    pub fn new() -> Self {
        Self {
            repo_root: None,
            shell: crate::commands::shell_command::ShellCommandHandler,
            bash: crate::commands::bash::BashHandler,
            zsh: crate::commands::zsh::ZshHandler,
            apply_patch: crate::commands::apply_patch::ApplyPatchHandler,
            planning: crate::commands::planning::PlanningHandler,
        }
    }

    pub fn new_in_root(repo_root: PathBuf) -> Self {
        Self {
            repo_root: Some(repo_root),
            ..Self::new()
        }
    }

    pub fn resolve_command_tool_name(&self, command: &str) -> Option<String> {
        match crate::commands::canonical_command(command).as_str() {
            "shell_command" => Some("shell_command".to_string()),
            "bash" => Some("bash".to_string()),
            "zsh" => Some("zsh".to_string()),
            "apply_patch" => Some("apply_patch".to_string()),
            "planning" if planning_command_enabled() => Some("planning".to_string()),
            command_name => self
                .external_manifest(command_name)
                .map(|manifest| manifest.id),
        }
    }

    pub fn handler(&self, tool_name: &str) -> Option<&dyn ToolHandler> {
        match tool_name {
            "shell_command" => Some(&self.shell),
            "bash" => Some(&self.bash),
            "zsh" => Some(&self.zsh),
            "apply_patch" => Some(&self.apply_patch),
            "planning" if planning_command_enabled() => Some(&self.planning),
            _ => None,
        }
    }

    pub fn tool_supports_macro_command(&self, call: &ToolCall) -> bool {
        if let Some(handler) = self.handler(&call.tool_name) {
            return handler.supports_macro_command();
        }
        self.external_manifest(&call.tool_name)
            .map(|manifest| manifest.supports_macro_command)
            .unwrap_or(false)
    }

    pub async fn command_is_mutating(&self, call: &ToolCall, ctx: &ToolContext) -> bool {
        if let Some(handler) = self.handler(&call.tool_name) {
            return handler.is_mutating(call, ctx).await;
        }
        self.external_manifest(&call.tool_name)
            .map(|manifest| manifest.mutating)
            .unwrap_or(false)
    }

    pub fn default_timeout_ms_for_command(&self, command: &str) -> Option<u64> {
        let command_name = crate::commands::canonical_command(command);
        self.external_manifest(&command_name)
            .map(|manifest| manifest.default_timeout_ms)
    }

    pub async fn dispatch(
        &self,
        call: ToolCall,
        ctx: ToolContext,
        force_exclusive: bool,
    ) -> Result<AnyToolResult, ToolError> {
        let external_handler;
        let handler = if let Some(handler) = self.handler(&call.tool_name) {
            handler
        } else if self.external_manifest(&call.tool_name).is_some() {
            external_handler = ExternalCommandHandler::new(call.tool_name.clone())?;
            &external_handler as &dyn ToolHandler
        } else {
            return Err(ToolError::RespondToModel(format!(
                "unsupported command_run command: {}",
                call.tool_name
            )));
        };
        if let Some(pre) = ctx.hooks().pre {
            pre(&call)?;
        }
        ctx.record_event(ToolRuntimeEvent::ToolStarted {
            call_id: call.call_id.clone(),
            tool_name: call.tool_name.clone(),
        });
        let mut dispatched =
            super::dispatch::dispatch_handler(handler, call.clone(), ctx.clone(), force_exclusive)
                .await?;
        let mut result = dispatched.result;
        if let Some(post) = ctx.hooks().post {
            post(&call, &mut result)?;
        }
        ctx.record_event(ToolRuntimeEvent::ToolFinished {
            call_id: call.call_id.clone(),
            tool_name: call.tool_name.clone(),
            success: result.success_for_logging(),
        });
        dispatched.result = result;
        Ok(dispatched)
    }

    fn external_manifest(&self, command_name: &str) -> Option<crate::registry::CommandManifest> {
        let root = self
            .repo_root
            .clone()
            .or_else(crate::external::client::repo_root)?;
        let manifest = crate::registry::manifest_for(&root, command_name)?;
        manifest.is_external_cli().then_some(manifest)
    }
}

impl Default for CommandRouter {
    fn default() -> Self {
        Self::new()
    }
}

pub type ToolRouter = CommandRouter;

fn planning_command_enabled() -> bool {
    ["TURA_FORCE_PLANNING", "TURA_FORCE_EXECUTE_TOOLS_PLANNING"]
        .iter()
        .any(|name| {
            std::env::var(name)
                .ok()
                .map(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
                .unwrap_or(false)
        })
}

pub struct ExternalCommandHandler {
    command_id: String,
    manifest: crate::registry::CommandManifest,
}

impl ExternalCommandHandler {
    pub fn new(command_id: String) -> Result<Self, ToolError> {
        let metadata = crate::external::client::metadata_for(&command_id).ok_or_else(|| {
            ToolError::RespondToModel(format!("unsupported external command: {command_id}"))
        })?;
        Ok(Self {
            command_id: metadata.command_id,
            manifest: metadata.manifest,
        })
    }
}

fn take_external_command_timeout(command_id: &str, arguments: &mut Value) -> Duration {
    let Some(object) = arguments.as_object_mut() else {
        return Duration::from_millis(default_external_command_timeout_ms(command_id));
    };
    let timeout_ms = take_u64_field(object, &["timeout_ms", "timeoutMs"])
        .or_else(|| {
            take_u64_field(object, &["timeout_secs", "timeoutSecs"])
                .map(|seconds| seconds.saturating_mul(1000))
        })
        .unwrap_or_else(|| default_external_command_timeout_ms(command_id))
        .max(1);
    Duration::from_millis(timeout_ms)
}

fn default_external_command_timeout_ms(command_id: &str) -> u64 {
    CommandRouter::new()
        .default_timeout_ms_for_command(command_id)
        .unwrap_or(15_000)
}

fn take_u64_field(object: &mut serde_json::Map<String, Value>, keys: &[&str]) -> Option<u64> {
    for key in keys {
        if let Some(value) = object.remove(*key) {
            return value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()));
        }
    }
    None
}

#[async_trait::async_trait]
impl ToolHandler for ExternalCommandHandler {
    fn tool_name(&self) -> &str {
        &self.command_id
    }

    fn supports_macro_command(&self) -> bool {
        self.manifest.supports_macro_command
    }

    async fn is_mutating(&self, _call: &ToolCall, _ctx: &ToolContext) -> bool {
        self.manifest.mutating
    }

    async fn access(&self, call: &ToolCall, ctx: &ToolContext) -> file_locks::Access {
        let mut arguments = call.payload.code_mode_input();
        let timeout = take_external_command_timeout(&self.command_id, &mut arguments);
        let response = crate::external::launcher::invoke_with_timeout(
            &self.command_id,
            "access",
            serde_json::json!({
                "arguments": arguments,
                "session_dir": ctx.session_dir.display().to_string(),
                "call_id": call.call_id,
            }),
            timeout,
        )
        .await;
        response
            .ok()
            .and_then(|response| serde_json::from_value(response.output).ok())
            .unwrap_or_default()
    }

    async fn handle(
        &self,
        call: ToolCall,
        ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError> {
        let mut arguments = call.payload.code_mode_input();
        let timeout = take_external_command_timeout(&self.command_id, &mut arguments);
        let output = crate::external::launcher::execute_with_timeout(
            &self.command_id,
            arguments,
            &ctx.session_dir,
            &call.call_id,
            timeout,
        )
        .await?;
        Ok(FunctionToolOutput::from_value(output, Some(true)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct StaticHandler {
        name: &'static str,
        mutating: bool,
        macro_command: bool,
    }

    #[async_trait::async_trait]
    impl ToolHandler for StaticHandler {
        fn tool_name(&self) -> &str {
            self.name
        }

        fn supports_macro_command(&self) -> bool {
            self.macro_command
        }

        async fn is_mutating(&self, _call: &ToolCall, _ctx: &ToolContext) -> bool {
            self.mutating
        }

        async fn handle(
            &self,
            _call: ToolCall,
            _ctx: ToolContext,
        ) -> Result<FunctionToolOutput, ToolError> {
            Ok(FunctionToolOutput::from_value(
                json!({"ok": true}),
                Some(true),
            ))
        }
    }

    #[test]
    fn tool_payload_code_mode_input_preserves_function_json_and_freeform_text() {
        let function = ToolPayload::Function {
            arguments: json!({"command":"cargo test","timeout_secs":30}),
        };
        let freeform = ToolPayload::Freeform {
            input: "*** Begin Patch\n*** End Patch".to_string(),
        };

        assert_eq!(
            function.code_mode_input(),
            json!({"command":"cargo test","timeout_secs":30})
        );
        assert_eq!(
            freeform.code_mode_input(),
            json!("*** Begin Patch\n*** End Patch")
        );
    }

    #[test]
    fn function_tool_output_defaults_success_for_logging_and_returns_body_for_code_mode() {
        let implicit = FunctionToolOutput::from_value(json!({"result": "ok"}), None);
        assert!(implicit.success_for_logging());
        assert_eq!(implicit.code_mode_result(), json!({"result": "ok"}));

        let explicit_failure = FunctionToolOutput::from_value(json!({"error": "bad"}), Some(false));
        assert!(!explicit_failure.success_for_logging());
        assert_eq!(explicit_failure.code_mode_result(), json!({"error": "bad"}));
    }

    #[test]
    fn tool_context_child_shares_events_hooks_gate_and_cancellation_but_keeps_call_id() {
        let context =
            ToolContext::new(PathBuf::from("workspace")).with_call_id("call-1".to_string());
        let child = context.child();

        assert_eq!(child.session_dir, PathBuf::from("workspace"));
        assert_eq!(child.current_call_id(), Some("call-1"));
        assert_eq!(child.lock_scope(), None);
        child.record_event(ToolRuntimeEvent::ToolStarted {
            call_id: "call-1".to_string(),
            tool_name: "shell_command".to_string(),
        });
        assert_eq!(context.events().len(), 1);

        context.cancellation.cancel();
        assert!(child.cancellation.is_cancelled());
    }

    #[tokio::test]
    async fn cancellation_token_waiters_are_notified_and_late_waiters_return_immediately() {
        let token = CancellationToken::new();
        let waiter_token = token.child_token();
        let waiter = tokio::spawn(async move {
            waiter_token.cancelled().await;
            waiter_token.is_cancelled()
        });

        assert!(!token.is_cancelled());
        token.cancel();
        assert!(waiter.await.expect("waiter task"));
        token.cancelled().await;
    }

    #[tokio::test]
    async fn default_access_tracks_mutating_workspace_write_rule() {
        let context = ToolContext::new(PathBuf::from("workspace"));
        let call = ToolCall {
            tool_name: "dummy".to_string(),
            call_id: "call".to_string(),
            payload: ToolPayload::Function {
                arguments: json!({}),
            },
        };
        let read_only = StaticHandler {
            name: "dummy",
            mutating: false,
            macro_command: false,
        };
        let mutating = StaticHandler {
            name: "dummy",
            mutating: true,
            macro_command: false,
        };

        assert!(!read_only.access(&call, &context).await.workspace_write);
        assert!(mutating.access(&call, &context).await.workspace_write);
    }

    #[test]
    fn external_command_timeout_is_taken_from_arguments_without_forwarding() {
        let mut arguments = json!({
            "path": "image.png",
            "timeout_ms": 2500
        });

        let timeout = take_external_command_timeout("read_media", &mut arguments);

        assert_eq!(timeout, std::time::Duration::from_millis(2500));
        assert_eq!(arguments, json!({"path": "image.png"}));
    }

    #[test]
    fn external_command_timeout_defaults_and_rounds_up_to_one_millisecond() {
        let mut missing = json!({"path": "image.png"});
        assert_eq!(
            take_external_command_timeout("read_media", &mut missing),
            std::time::Duration::from_millis(60_000)
        );

        let mut zero = json!({"timeout_secs": 0});
        assert_eq!(
            take_external_command_timeout("read_media", &mut zero),
            std::time::Duration::from_millis(1)
        );
        assert_eq!(zero, json!({}));
    }

    #[test]
    fn generate_media_external_command_timeout_defaults_to_100_seconds() {
        let mut missing = json!({"prompt": "logo"});

        assert_eq!(
            take_external_command_timeout("generate_media", &mut missing),
            std::time::Duration::from_millis(100_000)
        );
        assert_eq!(missing, json!({"prompt": "logo"}));
    }

    #[test]
    fn command_router_resolves_external_cli_commands_from_registry_manifest() {
        let root = temp_root("router-registry");
        write_external_manifest(
            &root,
            "custom_cli",
            "tura-command-custom-cli",
            true,
            false,
            4321,
        );

        let router = CommandRouter::new_in_root(root.clone());
        let call = ToolCall {
            tool_name: "custom_cli".to_string(),
            call_id: "custom".to_string(),
            payload: ToolPayload::Function {
                arguments: json!({}),
            },
        };

        assert_eq!(
            router.resolve_command_tool_name("custom_cli"),
            Some("custom_cli".to_string())
        );
        assert!(router.handler("custom_cli").is_none());
        assert!(router.tool_supports_macro_command(&call));
        assert_eq!(
            router.default_timeout_ms_for_command("custom_cli"),
            Some(4321)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tool_router_resolves_documented_command_aliases_and_macro_support() {
        let router = CommandRouter::new();
        let active_shell = crate::commands::active_shell_command_name();

        assert_eq!(
            router.resolve_command_tool_name("shell_command"),
            Some(active_shell.to_string())
        );
        assert_eq!(
            router.resolve_command_tool_name("shell-command"),
            Some(active_shell.to_string())
        );
        assert_eq!(
            router.resolve_command_tool_name("bash"),
            Some(active_shell.to_string())
        );
        assert_eq!(
            router.resolve_command_tool_name("zsh"),
            Some(active_shell.to_string())
        );
        assert_eq!(
            router.resolve_command_tool_name("apply_patch"),
            Some("apply_patch".to_string())
        );
        assert_eq!(router.resolve_command_tool_name("compact_context"), None);
        assert_eq!(
            router.resolve_command_tool_name("generate_media"),
            Some("generate_media".to_string())
        );
        assert_eq!(
            router.resolve_command_tool_name("read_media"),
            Some("read_media".to_string())
        );
        assert_eq!(
            router.resolve_command_tool_name("web_discover"),
            Some("web_discover".to_string())
        );
        assert_eq!(router.resolve_command_tool_name("missing"), None);

        let read_media = ToolCall {
            tool_name: "read_media".to_string(),
            call_id: "read".to_string(),
            payload: ToolPayload::Function {
                arguments: json!({}),
            },
        };
        let shell = ToolCall {
            tool_name: "shell_command".to_string(),
            call_id: "shell".to_string(),
            payload: ToolPayload::Function {
                arguments: json!({}),
            },
        };
        assert!(router.tool_supports_macro_command(&read_media));
        assert!(router.tool_supports_macro_command(&shell));
    }

    #[test]
    fn planning_command_is_hidden_until_explicit_force_env_is_enabled() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let previous_force = std::env::var_os("TURA_FORCE_PLANNING");
        let previous_execute = std::env::var_os("TURA_FORCE_EXECUTE_TOOLS_PLANNING");
        std::env::remove_var("TURA_FORCE_PLANNING");
        std::env::remove_var("TURA_FORCE_EXECUTE_TOOLS_PLANNING");
        let router = CommandRouter::new();
        assert_eq!(router.resolve_command_tool_name("planning"), None);
        assert!(router.handler("planning").is_none());

        for value in ["1", "true", "yes", "on", " TRUE "] {
            std::env::set_var("TURA_FORCE_PLANNING", value);
            assert_eq!(
                router.resolve_command_tool_name("planning"),
                Some("planning".to_string())
            );
            assert!(router.handler("planning").is_some());
        }
        std::env::set_var("TURA_FORCE_PLANNING", "false");
        std::env::set_var("TURA_FORCE_EXECUTE_TOOLS_PLANNING", "yes");
        assert_eq!(
            router.resolve_command_tool_name("planning"),
            Some("planning".to_string())
        );

        restore_env("TURA_FORCE_PLANNING", previous_force);
        restore_env("TURA_FORCE_EXECUTE_TOOLS_PLANNING", previous_execute);
    }

    #[tokio::test]
    async fn dispatch_unknown_tool_responds_to_model_without_recording_events() {
        let router = CommandRouter::new();
        let context = ToolContext::new(PathBuf::from("workspace"));
        let call = ToolCall {
            tool_name: "missing".to_string(),
            call_id: "call-missing".to_string(),
            payload: ToolPayload::Function {
                arguments: json!({}),
            },
        };

        let error = router
            .dispatch(call, context.clone(), false)
            .await
            .expect_err("unknown tool should fail");

        assert!(
            matches!(error, ToolError::RespondToModel(message) if message.contains("unsupported command_run command: missing"))
        );
        assert!(context.events().is_empty());
    }

    #[tokio::test]
    async fn dispatch_pre_hook_failure_stops_before_start_event() {
        let router = CommandRouter::new();
        let context = ToolContext::new(PathBuf::from("workspace"));
        context.set_pre_hook(|call| {
            Err(ToolError::RespondToModel(format!(
                "blocked {}",
                call.tool_name
            )))
        });
        let call = ToolCall {
            tool_name: "shell_command".to_string(),
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: json!({"command":"Write-Output hook"}),
            },
        };

        let error = router
            .dispatch(call, context.clone(), false)
            .await
            .expect_err("pre hook should fail");

        assert!(
            matches!(error, ToolError::RespondToModel(message) if message == "blocked shell_command")
        );
        assert!(context.events().is_empty());
    }

    #[tokio::test]
    async fn dispatch_records_start_finish_and_post_hook_can_change_success() {
        let router = CommandRouter::new();
        let context = ToolContext::new(PathBuf::from("workspace"));
        let post_calls = Arc::new(AtomicUsize::new(0));
        let post_calls_for_hook = Arc::clone(&post_calls);
        context.set_post_hook(move |_call, output| {
            post_calls_for_hook.fetch_add(1, Ordering::SeqCst);
            output.success = Some(false);
            output.body = json!({"hooked": true});
            Ok(())
        });
        let call = ToolCall {
            tool_name: "apply_patch".to_string(),
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: json!({}),
            },
        };

        let result = router
            .dispatch(call, context.clone(), false)
            .await
            .expect("apply_patch dispatch");

        assert_eq!(result.call_id, "call-1");
        assert_eq!(result.result.success, Some(false));
        assert_eq!(result.result.body, json!({"hooked": true}));
        assert_eq!(post_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            context.events(),
            vec![
                ToolRuntimeEvent::ToolStarted {
                    call_id: "call-1".to_string(),
                    tool_name: "apply_patch".to_string(),
                },
                ToolRuntimeEvent::ToolFinished {
                    call_id: "call-1".to_string(),
                    tool_name: "apply_patch".to_string(),
                    success: false,
                },
            ]
        );
    }

    #[test]
    fn tool_error_display_uses_model_visible_message_for_both_error_kinds() {
        assert_eq!(
            ToolError::Fatal("fatal error".to_string()).to_string(),
            "fatal error"
        );
        assert_eq!(
            ToolError::RespondToModel("model error".to_string()).to_string(),
            "model error"
        );
    }

    fn restore_env(key: &str, previous: Option<std::ffi::OsString>) {
        if let Some(value) = previous {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("tura-command-router-{name}-{suffix}"))
    }

    fn write_external_manifest(
        root: &Path,
        id: &str,
        binary: &str,
        supports_macro_command: bool,
        mutating: bool,
        default_timeout_ms: u64,
    ) {
        let directory = root.join("commands").join(id);
        fs::create_dir_all(&directory).expect("create command manifest directory");
        fs::write(
            directory.join("command.toml"),
            format!(
                r#"id = "{id}"
name = "{id}"
description = "{id}"
core = false
category = "test"
execution = "one_shot"
state_machine = "default_command"
supports_macro_command = {supports_macro_command}
mutating = {mutating}
network = false

[runtime]
binary = "{binary}"
entry = ""
language = "rust"

[limits]
default_timeout_ms = {default_timeout_ms}
max_timeout_ms = 300000

[paths]
prompt = "prompt.md"
schema = "schema.json"
policy = "policy.toml"
"#
            ),
        )
        .expect("write command manifest");
    }
}
