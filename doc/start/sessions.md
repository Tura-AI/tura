# Sessions

Session 是 Tura 保存一次任务上下文、消息、任务状态、工具结果和运行时状态的单位。CLI、TUI、GUI 都围绕 session 工作。

## 创建 session

一次性 CLI 会自动创建 session：

```bash
tura exec "Inspect this repository"
```

TUI CLI 通过 gateway 创建 session：

```bash
tura run "Fix the failing unit test"
```

GUI 新建对话也是创建 session，只是走的是 HTTP API。

代码引用：

- `crates/gateway/src/api/session.rs`，函数 `create_session`、`create_session_value`。
- `apps/tui/src/commands/run.ts`，函数 `runPromptWithShellEnv`。

## 复用 session

Rust CLI：

```bash
tura exec --session-id my-session "Continue the task"
```

TUI CLI：

```bash
tura resume SESSION_ID "Continue from the previous result"
tura run --session SESSION_ID "Add a follow-up check"
```

## 列出和查看 session

```bash
tura session list --json
tura session show SESSION_ID --json
tura inspect messages SESSION_ID --json
```

Gateway API：

```text
GET /session?directory=<workspace>&includeChildren=true
GET /session/{sessionID}
GET /session/{sessionID}/message
```

代码引用：`crates/gateway/src/web/server.rs` 的 `build_router`，`crates/gateway/src/api/session.rs` 的 `list_sessions_value`、`get_session_value`。

## Session 状态

常见状态包括 idle、busy、interrupted/error 等。Gateway 会探测 busy session 的 runtime liveness：如果 router 探测不到活跃 worker，会把 session 标记 interrupted。

代码引用：`crates/gateway/src/api/session.rs` 的 `refresh_busy_session_liveness`、`mark_session_interrupted_from_gateway_probe`。

## 任务状态和 session 的关系

`task_status` 不只是给用户看的进度，它会更新 session 的 `task_plan`、`task_type`、operation manual 注入状态和 session name。

例子：

```json
{
  "status": "doing",
  "task_group": "storefront frontend",
  "task_type": ["frontend"]
}
```

代码引用：`crates/runtime/src/tool_flow/task_status.rs` 的 `apply_tool_result_session_state_update`、`apply_status_result`。
