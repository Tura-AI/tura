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
    fn tool_name(&self) -> &'static str;

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

pub struct ToolRouter {
    shell: crate::commands::shell_command::ShellCommandHandler,
    bash: crate::commands::bash::BashHandler,
    zsh: crate::commands::zsh::ZshHandler,
    apply_patch: crate::commands::apply_patch::ApplyPatchHandler,
    compact_context: crate::commands::compact_context::CompactContextHandler,
    planning: crate::commands::planning::PlanningHandler,
    image_generate: ExternalCommandHandler,
    read_media: ExternalCommandHandler,
    web_discover: ExternalCommandHandler,
}

impl ToolRouter {
    pub fn new() -> Self {
        Self {
            shell: crate::commands::shell_command::ShellCommandHandler,
            bash: crate::commands::bash::BashHandler,
            zsh: crate::commands::zsh::ZshHandler,
            apply_patch: crate::commands::apply_patch::ApplyPatchHandler,
            compact_context: crate::commands::compact_context::CompactContextHandler,
            planning: crate::commands::planning::PlanningHandler,
            image_generate: ExternalCommandHandler::new("image_generate", true, false),
            read_media: ExternalCommandHandler::new("read_media", false, true),
            web_discover: ExternalCommandHandler::new("web_discover", false, true),
        }
    }

    pub fn resolve_command_tool_name(&self, command: &str) -> Option<&'static str> {
        match crate::commands::canonical_command(command).as_str() {
            "shell_command" => Some("shell_command"),
            "bash" => Some("bash"),
            "zsh" => Some("zsh"),
            "apply_patch" => Some("apply_patch"),
            "compact_context" => Some("compact_context"),
            "planning" if planning_command_enabled() => Some("planning"),
            "image_generate" => Some("image_generate"),
            "read_media" => Some("read_media"),
            "web_discover" => Some("web_discover"),
            _ => None,
        }
    }

    pub fn handler(&self, tool_name: &str) -> Option<&dyn ToolHandler> {
        match tool_name {
            "shell_command" => Some(&self.shell),
            "bash" => Some(&self.bash),
            "zsh" => Some(&self.zsh),
            "apply_patch" => Some(&self.apply_patch),
            "compact_context" => Some(&self.compact_context),
            "planning" if planning_command_enabled() => Some(&self.planning),
            "image_generate" => Some(&self.image_generate),
            "read_media" => Some(&self.read_media),
            "web_discover" => Some(&self.web_discover),
            _ => None,
        }
    }

    pub fn tool_supports_macro_command(&self, call: &ToolCall) -> bool {
        self.handler(&call.tool_name)
            .map(|handler| handler.supports_macro_command())
            .unwrap_or(false)
    }

    pub async fn dispatch(
        &self,
        call: ToolCall,
        ctx: ToolContext,
        force_exclusive: bool,
    ) -> Result<AnyToolResult, ToolError> {
        let handler = self.handler(&call.tool_name).ok_or_else(|| {
            ToolError::RespondToModel(format!(
                "unsupported command_run command: {}",
                call.tool_name
            ))
        })?;
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
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}

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
    command_id: &'static str,
    mutating: bool,
    macro_command: bool,
}

impl ExternalCommandHandler {
    pub fn new(command_id: &'static str, mutating: bool, macro_command: bool) -> Self {
        Self {
            command_id,
            mutating,
            macro_command,
        }
    }
}

const DEFAULT_EXTERNAL_COMMAND_TIMEOUT_MS: u64 = 15_000;
const IMAGE_GENERATE_EXTERNAL_COMMAND_TIMEOUT_MS: u64 = 100_000;

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
    match crate::commands::canonical_command(command_id).as_str() {
        "image_generate" => IMAGE_GENERATE_EXTERNAL_COMMAND_TIMEOUT_MS,
        _ => DEFAULT_EXTERNAL_COMMAND_TIMEOUT_MS,
    }
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
    fn tool_name(&self) -> &'static str {
        self.command_id
    }

    fn supports_macro_command(&self) -> bool {
        self.macro_command
    }

    async fn is_mutating(&self, _call: &ToolCall, _ctx: &ToolContext) -> bool {
        self.mutating
    }

    async fn access(&self, call: &ToolCall, ctx: &ToolContext) -> file_locks::Access {
        let mut arguments = call.payload.code_mode_input();
        let timeout = take_external_command_timeout(self.command_id, &mut arguments);
        let response = crate::external::launcher::invoke_with_timeout(
            self.command_id,
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
        let timeout = take_external_command_timeout(self.command_id, &mut arguments);
        let output = crate::external::launcher::execute_with_timeout(
            self.command_id,
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct StaticHandler {
        name: &'static str,
        mutating: bool,
        macro_command: bool,
    }

    #[async_trait::async_trait]
    impl ToolHandler for StaticHandler {
        fn tool_name(&self) -> &'static str {
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
            std::time::Duration::from_millis(15_000)
        );

        let mut zero = json!({"timeout_secs": 0});
        assert_eq!(
            take_external_command_timeout("read_media", &mut zero),
            std::time::Duration::from_millis(1)
        );
        assert_eq!(zero, json!({}));
    }

    #[test]
    fn image_generate_external_command_timeout_defaults_to_100_seconds() {
        let mut missing = json!({"prompt": "logo"});

        assert_eq!(
            take_external_command_timeout("image_generate", &mut missing),
            std::time::Duration::from_millis(100_000)
        );
        assert_eq!(missing, json!({"prompt": "logo"}));
    }

    #[test]
    fn tool_router_resolves_documented_command_aliases_and_macro_support() {
        let router = ToolRouter::new();
        let active_shell = crate::commands::active_shell_command_name();

        assert_eq!(
            router.resolve_command_tool_name("shell_command"),
            Some(active_shell)
        );
        assert_eq!(
            router.resolve_command_tool_name("shell-command"),
            Some(active_shell)
        );
        assert_eq!(router.resolve_command_tool_name("bash"), Some(active_shell));
        assert_eq!(router.resolve_command_tool_name("zsh"), Some(active_shell));
        assert_eq!(
            router.resolve_command_tool_name("apply_patch"),
            Some("apply_patch")
        );
        assert_eq!(
            router.resolve_command_tool_name("compact_context"),
            Some("compact_context")
        );
        assert_eq!(
            router.resolve_command_tool_name("image_generate"),
            Some("image_generate")
        );
        assert_eq!(
            router.resolve_command_tool_name("read_media"),
            Some("read_media")
        );
        assert_eq!(
            router.resolve_command_tool_name("web_discover"),
            Some("web_discover")
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
        let router = ToolRouter::new();
        assert_eq!(router.resolve_command_tool_name("planning"), None);
        assert!(router.handler("planning").is_none());

        for value in ["1", "true", "yes", "on", " TRUE "] {
            std::env::set_var("TURA_FORCE_PLANNING", value);
            assert_eq!(
                router.resolve_command_tool_name("planning"),
                Some("planning")
            );
            assert!(router.handler("planning").is_some());
        }
        std::env::set_var("TURA_FORCE_PLANNING", "false");
        std::env::set_var("TURA_FORCE_EXECUTE_TOOLS_PLANNING", "yes");
        assert_eq!(
            router.resolve_command_tool_name("planning"),
            Some("planning")
        );

        restore_env("TURA_FORCE_PLANNING", previous_force);
        restore_env("TURA_FORCE_EXECUTE_TOOLS_PLANNING", previous_execute);
    }

    #[tokio::test]
    async fn dispatch_unknown_tool_responds_to_model_without_recording_events() {
        let router = ToolRouter::new();
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
        let router = ToolRouter::new();
        let context = ToolContext::new(PathBuf::from("workspace"));
        context.set_pre_hook(|call| {
            Err(ToolError::RespondToModel(format!(
                "blocked {}",
                call.tool_name
            )))
        });
        let call = ToolCall {
            tool_name: "compact_context".to_string(),
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: json!({}),
            },
        };

        let error = router
            .dispatch(call, context.clone(), false)
            .await
            .expect_err("pre hook should fail");

        assert!(
            matches!(error, ToolError::RespondToModel(message) if message == "blocked compact_context")
        );
        assert!(context.events().is_empty());
    }

    #[tokio::test]
    async fn dispatch_records_start_finish_and_post_hook_can_change_success() {
        let router = ToolRouter::new();
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
            tool_name: "compact_context".to_string(),
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: json!({"summary":"checkpoint after successful dispatch"}),
            },
        };

        let result = router
            .dispatch(call, context.clone(), false)
            .await
            .expect("compact_context dispatch");

        assert_eq!(result.call_id, "call-1");
        assert_eq!(result.result.success, Some(false));
        assert_eq!(result.result.body, json!({"hooked": true}));
        assert_eq!(post_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            context.events(),
            vec![
                ToolRuntimeEvent::ToolStarted {
                    call_id: "call-1".to_string(),
                    tool_name: "compact_context".to_string(),
                },
                ToolRuntimeEvent::ToolFinished {
                    call_id: "call-1".to_string(),
                    tool_name: "compact_context".to_string(),
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
}
