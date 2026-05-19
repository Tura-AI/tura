use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};

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
}

impl ToolContext {
    pub fn new(session_dir: PathBuf) -> Self {
        Self {
            session_dir,
            cancellation: CancellationToken::new(),
            execution_gate: Arc::new(RwLock::new(())),
            events: Arc::new(std::sync::Mutex::new(Vec::new())),
            hooks: Arc::new(std::sync::Mutex::new(ToolHooks::default())),
            current_call_id: None,
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

    fn supports_parallel_tool_calls(&self) -> bool {
        false
    }

    async fn is_mutating(&self, call: &ToolCall, ctx: &ToolContext) -> bool;

    async fn handle(
        &self,
        call: ToolCall,
        ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError>;
}

pub struct ToolRouter {
    shell: crate::commands::shell_command::ShellCommandHandler,
    bash: crate::commands::bash::BashHandler,
    apply_patch: crate::commands::apply_patch::ApplyPatchHandler,
}

impl ToolRouter {
    pub fn new() -> Self {
        Self {
            shell: crate::commands::shell_command::ShellCommandHandler,
            bash: crate::commands::bash::BashHandler,
            apply_patch: crate::commands::apply_patch::ApplyPatchHandler,
        }
    }

    pub fn resolve_command_tool_name(&self, command: &str) -> Option<&'static str> {
        match crate::commands::canonical_command(command).as_str() {
            "shell_command" => Some("shell_command"),
            "bash" => Some("bash"),
            "apply_patch" => Some("apply_patch"),
            _ => None,
        }
    }

    pub fn handler(&self, tool_name: &str) -> Option<&dyn ToolHandler> {
        match tool_name {
            "shell_command" => Some(&self.shell),
            "bash" => Some(&self.bash),
            "apply_patch" => Some(&self.apply_patch),
            _ => None,
        }
    }

    pub fn tool_supports_parallel(&self, call: &ToolCall) -> bool {
        self.handler(&call.tool_name)
            .map(|handler| handler.supports_parallel_tool_calls())
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
        let mutating = force_exclusive || handler.is_mutating(&call, &ctx).await;
        let call_ctx = ctx.with_call_id(call.call_id.clone());
        let mut result = if mutating {
            let gate = Arc::clone(&ctx.execution_gate);
            let _guard = gate.write().await;
            handler.handle(call.clone(), call_ctx.clone()).await?
        } else {
            let gate = Arc::clone(&ctx.execution_gate);
            let _guard = gate.read().await;
            handler.handle(call.clone(), call_ctx.clone()).await?
        };
        if let Some(post) = ctx.hooks().post {
            post(&call, &mut result)?;
        }
        ctx.record_event(ToolRuntimeEvent::ToolFinished {
            call_id: call.call_id.clone(),
            tool_name: call.tool_name.clone(),
            success: result.success_for_logging(),
        });
        Ok(AnyToolResult {
            call_id: call.call_id,
            payload: call.payload,
            result,
        })
    }
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}
