# How to start

Tura 的启动方式取决于你要不要交互。

## 一次性任务：`tura exec`

```bash
tura exec "Inspect this repository and list the main crates"
```

特点：

- 默认 stdout 只输出最终 assistant 文本，适合脚本。
- 轻量进度输出到 stderr。
- 默认通过 router daemon 运行，不直接把 runtime 绑死在 CLI 进程里。
- prompt 为空时会从 stdin 读取。

例子：

```bash
echo "Summarize the architecture" | tura exec --json
tura exec -C C:/repo -m openai/gpt-5 -p "Fix failing tests"
tura exec zsh "Inspect shell startup behavior"
```

代码引用：`crates/gateway/src/tura_exec/cli.rs` 的 `CliConfig::parse` 和 `print_help`。

## 交互式终端：`tura`

```bash
tura
```

这会打开 TypeScript TUI。TUI 本身不执行模型、不读数据库、不跑工具，它通过 gateway HTTP/SSE 访问后端。

例子：

```bash
tura --cwd C:/repo
tura --initial-session SESSION_ID
tura --plain
tura --rich
```

代码引用：`apps/tui/src/cli.ts` 的 `main`、`parseGlobal`。

## Gateway 和 GUI

本地开发常用脚本：

```powershell
.\scripts\start.ps1 -Gui
```

```bash
./scripts/start.sh --gui
```

GUI 的 gateway URL 优先级：URL 参数 `gatewayUrl`、`localStorage["tura.gatewayUrl"]`、`VITE_TURA_GATEWAY_URL`、默认 `http://127.0.0.1:4126`。

代码引用：

- `apps/gui/app/src/app.tsx`，函数 `App`。
- `apps/tauri/src-tauri/src/main.rs`，函数 `parse_initial_route_params`。
- `crates/gateway/src/web/server.rs`，函数 `build_router`。

## 什么时候用哪个

| 场景 | 推荐入口 |
| --- | --- |
| CI 或脚本 | `tura exec --json` |
| 本地快速问一次 | `tura exec "..."` |
| 需要交互式会话 | `tura` |
| 需要图形界面 | `start.ps1 -Gui` 或 `start.sh --gui` |
| 要调试 gateway API | `tura gateway ...` |
