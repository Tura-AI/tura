# CLI parameters

Tura 有两套 CLI 参数：Rust CLI 的 `tura exec`，以及 TypeScript TUI CLI 的 `tura run/session/provider/...`。

## `tura exec`

基本格式：

```bash
tura exec [OPTIONS] [PROMPT...]
tura exec bash [OPTIONS] [PROMPT...]
tura exec zsh [OPTIONS] [PROMPT...]
tura exec shll [OPTIONS] [PROMPT...]
```

常用参数：

| 参数 | 作用 | 例子 |
| --- | --- | --- |
| `-C, --cwd PATH` | 指定 workspace | `tura exec -C . "Inspect"` |
| `-m, --model MODEL` | 覆盖模型；裸模型名会按 OpenAI 处理 | `-m openai/gpt-5` |
| `-p, --priority` | 启用 priority/acceleration 路由 | `-p` |
| `-a, --agent-id ID` | 选择 agent | `-a balanced` |
| `--session-id ID` | 复用指定 session id | `--session-id cli-demo` |
| `--goal` | 让 CLI 会话持续到 `task_status` 标记 done/question | `--goal` |
| `--no-op` | 禁用操作手册注入，除非 goal/reflection 覆盖 | `--no-op` |
| `--json` | stdout 输出 JSONL events | `--json` |
| `--quiet, --silent` | 抑制 stderr 进度 | `--quiet` |
| `--output-last-message PATH` | 把最终 assistant 文本写入文件 | `--output-last-message out.txt` |
| `--model-reasoning-effort LEVEL` | 设置 reasoning effort | `--model-reasoning-effort high` |
| `--planning auto|on|off` | 控制 planning | `--planning on` |
| `--bash, --zsh, --shll` | 强制 command_run shell surface | `--zsh` |
| `--sandbox` | 限制 command_run 写入/workdir 到 workspace | `--sandbox` |
| `-c, --config KEY=VALUE` | runtime override | `-c command_run_shell=zsh` |

例子：

```bash
tura exec -C . -m openai/gpt-5 -p --model-reasoning-effort high "Fix tests"
tura exec --quiet "Return only the final answer"
echo "Summarize architecture" | tura exec --json
```

代码引用：`crates/gateway/src/tura_exec/cli.rs` 的 `print_help`、`CliConfig::parse`。

## TUI CLI 全局参数

基本格式：

```bash
tura [OPTIONS]
tura [OPTIONS] run [PROMPT...]
tura [OPTIONS] session list
tura [OPTIONS] provider list
```

全局参数：

| 参数 | 作用 |
| --- | --- |
| `--cwd PATH` | 发给 gateway 的 workspace directory |
| `--gateway-url URL` | 指定 gateway 地址 |
| `--initial-session ID` | TUI 打开时选中 session |
| `--json` | 支持 JSON 的命令输出 JSON |
| `--color auto|always|never` | 控制终端颜色 |
| `--lang zh-CN|en` | 显示语言 |
| `--plain` | 强制 L1 纯文本渲染 |
| `--rich` | 强制 L3 富终端渲染 |
| `--verbose` | 打印 gateway 请求 |

代码引用：`apps/tui/src/cli.ts` 的 `parseGlobal`，帮助文本在 `apps/tui/src/locales/help.en.json`。

## `tura run`

`tura run` 通过 gateway 创建或复用 session，并等待结果。

```bash
tura run --model openai/gpt-5 --output ndjson "Fix the failing test"
tura run --session SESSION_ID "Continue from the last result"
tura zsh "Use zsh behavior for command tools"
```

常用参数：`--session`、`--model`、`--agent`、`--session-type`、`--model-variant`、`--model-acceleration`、`--no-model-acceleration`、`--bash`、`--zsh`、`--shel`、`--output text|json|ndjson`、`--stream`、`--no-stream`、`--timeout SEC`、`-c KEY=VALUE`。

代码引用：`apps/tui/src/commands/run.ts` 的 `runPrompt`、`promptPayload`。

## 子命令速查

| 命令 | 用途 |
| --- | --- |
| `session` | 列出、查看、更新、abort session |
| `config` | 读取/修改 workspace session config 和 model tier |
| `provider` | provider 列表、登录、auth 状态、logout |
| `agent` | agent 列表、创建、更新、删除、模型设置 |
| `persona` | persona 列表、创建、更新、删除 |
| `project` | workspace/project 操作 |
| `file` | 文件列表、读取、打开、定位 |
| `command` | command registry 列表和执行 |
| `inspect` | gateway 状态、路径、sessions、messages |
| `gateway` | 原始 gateway CLI protocol |
| `completion` | shell completion |
