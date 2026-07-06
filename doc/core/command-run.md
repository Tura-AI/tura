# Command run

`command_run` 是 Tura 最重要的工具设计：对模型只暴露一个紧凑工具，内部再批量执行 shell、patch、web、media、task 状态等命令。

## 为什么这样设计

普通多工具 agent 会反复传工具 schema、反复调用工具、反复把大段结果塞回上下文。`command_run` 把多个命令放进一个 batch，并用 `step` 表示依赖关系。

## 基本格式

```json
{
  "commands": [
    { "command_type": "shell_command", "command_line": "rg --files crates/runtime/src", "step": 1 },
    { "command_type": "shell_command", "command_line": "rg -n \"task_status\" crates/runtime/src", "step": 1 },
    { "command_type": "apply_patch", "command_line": "<raw patch body>", "step": 2 },
    { "command_type": "shell_command", "command_line": "cargo test -p runtime --lib", "step": 3 },
    { "command_type": "task_status", "command_line": "{\"status\":\"done\"}", "step": 4 }
  ]
}
```

`step` 的意思是依赖组：同一个 step 内的命令不能互相依赖，可以并行；后一个 step 依赖前一个 step 的结果。

## 常用内部命令

| command_type | 用途 |
| --- | --- |
| `shell_command` | 运行 shell 命令、测试、构建、脚本 |
| `apply_patch` | 修改文件 |
| `web_discover` | 搜索/下载网页、图片、视频、音频资料 |
| `task_status` | 更新任务状态、task_type、compact_context |
| `read_media` | 检查图片、视频、音频、PDF 等媒体 |
| `generate_media` | 生成媒体资源 |

## 例子：先读代码再改

```json
{
  "commands": [
    { "command_type": "shell_command", "command_line": "rg -n \"createSession\" apps/gui/app/src", "step": 1 },
    { "command_type": "shell_command", "command_line": "rg --files apps/gui/app/src", "step": 1 },
    { "command_type": "apply_patch", "command_line": "<focused patch>", "step": 2 },
    { "command_type": "shell_command", "command_line": "bun run --cwd apps/gui/app typecheck", "step": 3 }
  ]
}
```

## 代码引用

- `crates/tools/src/command_run/schema.json`：模型可见 schema。
- `crates/tools/src/command_run/mod.rs`：导出执行入口。
- `crates/tools/src/command_run/handler.rs`：执行、归一化、streaming command run。
- `crates/runtime/src/tool_flow/execute.rs`：runtime 调用工具流程。
- `crates/runtime/src/provider_flow/request_options.rs`：把 `command_run` 注入 provider request。

## 注意

- 不要把依赖前一步输出的命令放在同一个 `step`。
- 写文件优先用 `apply_patch`。
- 长时间服务必须后台启动、写日志、轮询 readiness 和进程退出。
- 验证命令能跑就必须跑；失败不是完成，只是问题换了个名字。
