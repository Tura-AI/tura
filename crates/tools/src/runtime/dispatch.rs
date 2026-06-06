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
        let _file_guard = file_locks::acquire(&access);
        handler.handle(call.clone(), call_ctx).await?
    } else {
        let gate = Arc::clone(&ctx.execution_gate);
        let _guard = gate.read().await;
        let _file_guard = file_locks::acquire(&access);
        handler.handle(call.clone(), call_ctx).await?
    };
    Ok(AnyToolResult {
        call_id: call.call_id,
        payload: call.payload,
        result,
    })
}
