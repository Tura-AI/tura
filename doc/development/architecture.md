# Development architecture

开发时先记住边界：不要让前端绕过 gateway，不要让 runtime 处理 provider auth，不要让 gateway 直接写 session DB，不要让 router 组 prompt。

## 包和 crate

| 路径 | 包/角色 |
| --- | --- |
| `crates/gateway` | HTTP/SSE gateway，binary `gateway` / `tura_exec` |
| `crates/router` | router daemon、registry、runtime worker dispatch |
| `crates/runtime` | agent runtime worker |
| `crates/tools` | `command_run` 和工具执行 |
| `crates/provider` | provider/model/auth/response normalization |
| `crates/session_log` | SQLite session owner |
| `agents` | agent config/prompt store |
| `personas` | persona config/prompt/media store |
| `apps/tui` | TypeScript terminal client |
| `apps/gui` | Bun/Solid/Vite GUI |
| `apps/tauri` | desktop wrapper |

代码引用：`Cargo.toml` workspace members。

## 推荐开发流程

1. 先确定问题归属：gateway、router、runtime、tools、provider、UI，还是 session DB。
2. 找对应测试，不要上来全仓乱改。
3. 修改最小边界。
4. 跑对应测试。
5. 如果改了行为或入口，更新 `doc/`。

## 边界例子

| 需求 | 应该改哪里 |
| --- | --- |
| 新增 CLI 参数 | `crates/gateway/src/tura_exec/cli.rs` 或 `apps/tui/src/cli.ts` |
| 新增 gateway API | `crates/gateway/src/web/server.rs` 和 `crates/gateway/src/api` |
| 新增 agent registry 行为 | `crates/router/src/registry/agent.rs` |
| 新增工具命令 | `crates/tools/src/commands` |
| 新增 provider model | `crates/provider/config/provider_config.json` |
| GUI 显示 tool result | `apps/gui/app/src/conversation` |

## 代码标准

Rust crate 开启了 `#![deny(clippy::unwrap_used)]` 和 `#![forbid(unsafe_code)]` 的地方不要绕。TypeScript 前端用现有 format/typecheck/test 命令。
