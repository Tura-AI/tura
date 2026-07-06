# Runtime architecture

Runtime 是 Tura 的 agent 执行层。它负责 session 创建/恢复、agent 激活、prompt 组装、provider turn、tool 调用、任务状态更新和最终回复。

## Runtime 做什么

- 根据 session 输入创建 `SessionManagement`。
- 加载 agent 配置和 runtime prompt manual。
- 组装 provider request。
- 消费 provider 返回的 tool call。
- 通过 tools 执行 `command_run`。
- 把任务状态、usage、message、tool result 写回 session。

## Runtime 不做什么

| 不属于 runtime | 归属 |
| --- | --- |
| Provider auth/OAuth/token 刷新 | `crates/provider` |
| HTTP API | `crates/gateway` |
| Registry 和 worker 调度 | `crates/router` |
| shell/file lock/sandbox 细节 | `crates/tools` |
| SQLite owner | `crates/session_log` |

## Worker 入口

`tura_runtime` 是 per-session worker。Router 启动它，任务完成后退出。

代码引用：

- `crates/runtime/src/bin/tura_runtime.rs`，函数 `main`。
- `crates/runtime/src/worker.rs`，函数 `run`。
- `crates/runtime/src/mano`：gateway session 处理。
- `crates/runtime/src/manas`：agent loop 和 child dispatch。

## Prompt 和 task type

Runtime 根据 session 的 `task_type` 注入操作手册。手册会被写入 session log，压缩后也能补回。

代码引用：`crates/runtime/src/prompt_style/runtime_prompt_manual.rs` 的 `append_missing_runtime_prompt_manuals`。

## Turn loop

Runtime 的 turn loop 分成 provider step、tool step、retry/finalization 等阶段。这样做的好处是：provider 错误、tool 错误、没有最终回答、任务未完成，都能分别处理，而不是变成一团“模型没回好”。

代码引用：

- `crates/runtime/src/turn_loop/provider_step.rs`。
- `crates/runtime/src/turn_loop/tool_step.rs`。
- `crates/runtime/src/turn_loop/finalization.rs`。
- `crates/runtime/src/turn_loop/retry_policy.rs`。
