use super::tool::{AnyToolResult, ToolCall, ToolContext, ToolError, ToolHandler};
use crate::runtime::file_locks;
use std::sync::Arc;

pub async fn dispatch_handler(
    handler: &dyn ToolHandler,
    call: ToolCall,
    ctx: ToolContext,
    force_exclusive: bool,
) -> Result<AnyToolResult, ToolError> {
    let mutating = force_exclusive || handler.is_mutating(&call, &ctx).await;
    let access = if force_exclusive {
        file_locks::Access {
            workspace_write: true,
            ..file_locks::Access::default()
        }
    } else {
        handler.access(&call, &ctx).await
    };
    let call_ctx = ctx.with_call_id(call.call_id.clone());
    let result = if mutating {
        let gate = Arc::clone(&ctx.execution_gate);
        let _guard = gate.write().await;
        let _file_guard = acquire_file_lock(access).await?;
        handler.handle(call.clone(), call_ctx).await?
    } else {
        let gate = Arc::clone(&ctx.execution_gate);
        let _guard = gate.read().await;
        let _file_guard = acquire_file_lock(access).await?;
        handler.handle(call.clone(), call_ctx).await?
    };
    Ok(AnyToolResult {
        call_id: call.call_id,
        payload: call.payload,
        result,
    })
}

async fn acquire_file_lock(
    access: file_locks::Access,
) -> Result<file_locks::LockGuard<'static>, ToolError> {
    tokio::task::spawn_blocking(move || file_locks::acquire(&access))
        .await
        .map_err(|err| ToolError::Fatal(format!("file lock task failed: {err}")))
}
