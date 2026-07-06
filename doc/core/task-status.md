# Task status

`task_status` 是 Tura 管理长任务状态的内部命令。它通常通过 `command_run` 调用，不是单独暴露给模型的大工具。

## 它解决什么

长任务常见问题是：做着做着忘了目标，或者测试没跑完就宣布完成。`task_status` 把任务状态写进 session，让 runtime、TUI、GUI 和后续回合都能看到当前状态。

## 字段

| 字段 | 用途 | 例子 |
| --- | --- | --- |
| `status` | 当前任务状态：`doing`、`question`、`done` | `"doing"` |
| `task_group` | 简短任务领域，用于 session name/plan summary | `"documentation system"` |
| `task_type` | 运行时操作手册类型 | `["debug"]` |
| `compact_context` | 上下文压缩交接摘要 | `"已完成 X，下一步 Y"` |

## 例子

开始一个 debug 任务：

```json
{
  "status": "doing",
  "task_group": "order service",
  "task_type": ["debug"]
}
```

需要用户回答：

```json
{
  "status": "question"
}
```

完成任务：

```json
{
  "status": "done"
}
```

上下文压缩：

```json
{
  "task_group": "runtime docs",
  "compact_context": "目标是完成 doc 文档重组。已完成 start/core 分类，下一步补 architecture/customization/development 并打包。"
}
```

## 代码如何处理

`task_status` 结果由 runtime 读取并应用到 `SessionManagement`：

- `crates/runtime/src/tool_flow/task_status.rs`，函数 `apply_tool_result_session_state_update`。
- `crates/runtime/src/tool_flow/task_status.rs`，函数 `apply_status_result`。
- `crates/runtime/src/tool_flow/task_status.rs`，函数 `apply_task_group`。
- `crates/runtime/src/tool_flow/task_status.rs`，函数 `complete_active_task`、`mark_active_task_doing`、`question_active_task`。

当 `task_type` 出现时，runtime 会调用 `runtime_prompt_manual::append_missing_runtime_prompt_manuals` 注入对应操作手册。

## 注意

`done` 不是“我觉得差不多了”。Tura 的规则是：所有明确要求、文件、测试、验证、打包物都完成并检查后，才能标记 `done`。文档任务也一样，不能写完几页就假装 GitBook 已经存在。宇宙还没那么好骗。
