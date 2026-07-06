# Commands

Commands 有两层含义：一层是 `command_run` 的内部命令，另一层是用户自定义 slash command/command registry。

## command_run 内部命令

这些命令由 `crates/tools` 管理。模型只看到 `command_run`，但 `command_run` 内部会按 `command_type` 找到具体 handler。

常见命令：

- `shell_command`
- `apply_patch`
- `web_discover`
- `read_media`
- `generate_media`
- `task_status`

代码引用：

- `crates/tools/src/commands`。
- `crates/tools/src/commands/mod.rs`，函数 `canonical_command`。
- `crates/tools/src/command_run/handler.rs`，函数 `normalize_command_value_for_execution`。

## 用户 command registry

用户可以在 workspace 下放命令文件，gateway/router 会发现它们。

支持目录：

```text
.tura/commands
.opencode/command
.opencode/commands
command
commands
```

支持文件类型：`.md`、`.txt`、`.json`。

Markdown 命令例子：

```md
# Review current diff

Review the current git diff. Focus on bugs, regressions, and missing tests.
Arguments: $ARGUMENTS
```

JSON 命令例子：

```json
{
  "name": "review",
  "description": "Review current changes",
  "agent": "coding",
  "model": "openai/gpt-5",
  "template": "Review this scope: {{args}}",
  "subtask": true,
  "hints": ["bugs", "tests"]
}
```

运行：

```bash
tura command list --json
tura command run review "apps/gui"
```

代码引用：

- `crates/router/src/registry/command.rs`，函数 `discover_commands`、`command_from_file`、`command_from_json`、`render_command_template`。
- `apps/tui/src/commands/command-registry.ts`，函数 `commandRegistryCommand`。
