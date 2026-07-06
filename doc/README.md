# Overview

Tura 是一个终端优先的 coding agent 系统，目标是处理困难、长周期、需要大量工具调用和验证的真实仓库任务。

网页文案 `i18n.js` 里反复出现四个关键词：Macro CLI、Reasoning、Prompt、TDD。对应到代码里就是：

- Macro CLI：`npm/tura.mjs`、`crates/gateway/src/tura_exec/mod.rs`、`apps/tui/src/cli.ts` 提供多个轻量入口。
- Reasoning：`crates/runtime/src/turn_loop`、`crates/runtime/src/state_machine` 控制运行循环和状态。
- Prompt/context：`crates/runtime/src/prompt_style/runtime_prompt_manual.rs` 按任务类型注入操作手册。
- TDD/验证：`crates/tools/src/command_run/schema.json` 要求批量、分步、少噪音地执行命令和检查。

## 最短理解

Tura 不是一个单进程程序，而是一条执行管线：

```text
用户 -> CLI/TUI/GUI -> gateway/router -> runtime worker -> provider/tools -> session DB
```

CLI 和 GUI 是入口；gateway 暴露 HTTP/SSE；router 负责调度和 registry；runtime 负责 agent 回合；tools 负责命令执行；provider 负责模型调用；session DB 负责持久记录。

## 常用命令

| 目标 | 命令 |
| --- | --- |
| 安装 release 包 | `npm install -g tura-ai` |
| 用 Rust CLI 跑一次任务 | `tura exec "Inspect this repo"` |
| 打开终端 UI | `tura` |
| 通过 gateway/TUI CLI 跑任务 | `tura run "Fix the failing test"` |
| 强制 zsh 命令环境 | `tura exec zsh "Inspect shell setup"` |
| 查看 provider | `tura provider list --json` |
| 查看 sessions | `tura session list --json` |

## 主要代码位置

- `i18n.js`：网页文案和产品定位。
- `npm/tura.mjs`：npm 启动器、release binary 查找、CLI PATH 注册。
- `crates/gateway/src/tura_exec/cli.rs`：Rust CLI 参数和解析。
- `apps/tui/src/locales/help.en.json`：终端客户端帮助文本。
- `crates/gateway/src/web/server.rs`：gateway API 路由表。
- `crates/router/src/registry`：agent、persona、command registry。
- `crates/runtime/src/runtime_prompt`：运行时操作手册 prompt 文件。
- `crates/tools/src/commands`：工具命令处理器和工具 prompt。

## 推荐阅读顺序

- [Install](start/install.md)
- [CLI parameters](start/cli-parameters.md)
- [Command run](core/command-run.md)
- [Runtime architecture](architecture/runtime.md)
- [Development architecture](development/architecture.md)
