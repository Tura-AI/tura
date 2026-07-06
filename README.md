# Tura

Tura 是一个面向长周期工程任务的本地 AI 编程系统。它不是单纯的聊天框，而是一套由 Rust 运行时、`command_run` 批处理工具、持久会话、运行时操作手册、Provider 路由、CLI/TUI/GUI 客户端组成的完整执行链路。

`i18n.js` 里的网页文案把 Tura 描述成一个用于真实仓库任务的开源 coding agent：减少 token 浪费、强调长任务 benchmark、从结果倒推原因、管理运行时上下文、以测试和验证结束任务。真实代码也基本按这个方向组织：前端保持薄，核心能力放在 runtime、router、gateway、provider、tools、agents、personas、session_log 这些边界清楚的模块里。

## 快速开始

通过 npm 安装：

```bash
npm install -g tura-ai
tura exec "Inspect this workspace and explain the architecture"
```

在源码仓库里开发：

```powershell
.\scripts\install.ps1
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
tura exec "Summarize the repo"
```

```bash
./scripts/install.sh
./scripts/build-release.sh
./scripts/register-cli.sh
tura exec "Summarize the repo"
```

## Tura 主要解决什么

| 常见问题 | Tura 的做法 | 文档入口 |
| --- | --- | --- |
| 工具调用太碎、schema 太重 | 对模型只暴露一个 `command_run`，内部用 `step` 批量执行命令 | [Command run](doc/core/command-run.md) |
| Prompt 越跑越肿 | 按 `task_type` 注入运行时操作手册，并写入会话记录 | [Runtime prompt](doc/core/runtime-prompt.md) |
| 长任务容易忘目标 | 用 `task_status`、session DB、context compaction、完成审计固定任务状态 | [Task status](doc/core/task-status.md), [Context management](doc/core/context-management.md) |
| 前端直接碰后端细节 | CLI/TUI/GUI 只走 gateway/router API，runtime 和 tools 留在 Rust crate | [Architecture](doc/development/architecture.md) |
| 自定义能力容易变成乱改 | Provider、agent、persona、command、runtime prompt 都有明确文件和 API 入口 | [Customization](doc/customization/custom-agents.md) |

## 系统链路

```text
npm/tura.mjs -> release binary `tura`
  -> tura exec / tura_exec CLI front
  -> tura_router daemon
  -> tura_runtime worker
  -> command_run / provider calls / session_log

GUI/TUI -> tura_gateway HTTP/SSE -> tura_router -> runtime workers
```

关键代码位置：

- npm 启动器：`npm/tura.mjs`，函数 `installedReleaseDir`、`platformReleaseDir`。
- Rust CLI：`crates/gateway/src/tura_exec/cli.rs`，函数 `print_help`、`CliConfig::parse`。
- Gateway HTTP API：`crates/gateway/src/web/server.rs`，函数 `build_router`。
- Router 命令分发：`crates/router/src/cli.rs`，函数 `run_router_command`。
- Runtime 操作手册：`crates/runtime/src/prompt_style/runtime_prompt_manual.rs`，函数 `available_manuals`、`append_missing_runtime_prompt_manuals`。
- Tool 批处理：`crates/tools/src/command_run/schema.json`、`crates/tools/src/command_run/mod.rs`。
- Agents：`agents/src/store.rs`，函数 `discover_agents`、`save_dynamic_agent`。
- Personas：`personas/src/store.rs`，函数 `discover_personas`、`save_dynamic_persona`。

## 文档目录

重整后的文档在 [doc](doc/SUMMARY.md)。

- [安装](doc/start/install.md)
- [如何启动](doc/start/how-to-start.md)
- [CLI 参数](doc/start/cli-parameters.md)
- [设置](doc/start/settings.md)
- [会话](doc/start/sessions.md)
- [Providers](doc/start/providers.md)
- [核心概念](doc/core/task-status.md)
- [架构](doc/architecture/runtime.md)
- [自定义](doc/customization/custom-commands.md)
- [开发](doc/development/testing.md)

GitBook 打包文件会生成在 `doc/gitbook/zip/tura-docs.zip`。

## License

AGPL-3.0-or-later。见 `LICENSE`。
