# Custom commands

Custom command 是 workspace 里的可复用 prompt 模板。适合把常用流程变成命令，例如 review、release-check、write-tests。

## 放在哪里

Router 会扫描这些目录：

```text
.tura/commands
.opencode/command
.opencode/commands
command
commands
```

## Markdown command

文件：`.tura/commands/review.md`

```md
# Review current changes

Review the current changes. Focus on bugs, regressions, missing tests, and risky assumptions.

Scope: $ARGUMENTS
```

运行：

```bash
tura command run review "apps/gui"
```

## JSON command

文件：`.tura/commands/release-check.json`

```json
{
  "name": "release-check",
  "description": "Run release readiness checks",
  "agent": "coding",
  "model": "openai/gpt-5",
  "template": "Check release readiness for {{args}}. Verify scripts, tests, and docs.",
  "subtask": false,
  "hints": ["release", "tests", "docs"]
}
```

## 模板变量

支持：

- `{{args}}`
- `{args}`
- `$ARGUMENTS`
- `$1`, `$2`, ...

代码引用：`crates/router/src/registry/command.rs` 的 `render_command_template`。

## 列出命令

```bash
tura command list --json
```

代码引用：

- `crates/router/src/registry/command.rs`，函数 `discover_commands`、`command_directories`。
- `crates/gateway/src/api/command.rs`。
