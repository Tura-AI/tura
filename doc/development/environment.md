# Environment

Tura 的环境变量分为 release/npm、runtime、gateway、provider、GUI/TUI 几类。

## npm/release

| 变量 | 用途 |
| --- | --- |
| `TURA_RELEASE_BIN_DIR` | release binary 目录 |
| `TURA_PROJECT_ROOT` | 项目根目录 |
| `TURA_PROVIDER_CONFIG` | provider config 路径 |
| `TURA_NPM_RELEASE_ARCHIVE` | npm install 使用的 release archive |
| `TURA_NPM_SKIP_CLI_REGISTRATION` | npm install 时跳过 CLI 注册 |

代码引用：`npm/tura.mjs`。

## Runtime/CLI

| 变量 | 用途 |
| --- | --- |
| `TURA_COMMAND_RUN_SHELL` | 指定 command_run shell surface |
| `TURA_NO_OP_MANUAL` | 禁用 operation manual 注入 |
| `TURA_GATEWAY_CALLBACKS` | 控制 gateway callbacks |
| `TURA_RUNTIME_ERRORS_FATAL` | runtime 错误 fatal 模式 |

代码引用：`crates/gateway/src/tura_exec/mod.rs`、`crates/runtime/src/state_machine/session_management.rs`。

## Provider

常见 provider env：

- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `OPENROUTER_API_KEY`
- `LOG_PATH`
- `OPENAI_LOGIN`
- `CODEX_HOME`

Provider config 里每个 provider 也声明自己的 `env` 和 `token_env`。

代码引用：`crates/provider/config/provider_config.json`。

## Gateway/GUI

| 变量 | 用途 |
| --- | --- |
| `PORT` | gateway listen port |
| `TURA_GUI_DIST` | gateway 服务 GUI 静态文件的目录 |
| `VITE_TURA_GATEWAY_URL` | GUI dev/build 使用的 gateway URL |

代码引用：`crates/gateway/src/bin/gateway.rs`、`crates/gateway/src/web/server.rs`。

## TUI

| 变量 | 用途 |
| --- | --- |
| `TURA_TUI_INITIAL_SESSION_ID` | TUI 初始 session |
| `TURA_TUI_MOCK` | mock 模式 |
| `TURA_DEV` | dev 模式 |

代码引用：`apps/tui/src/cli.ts` 的 `parseGlobal`。
