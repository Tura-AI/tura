# Start overview

这一组文档只解决一个问题：怎么把 Tura 跑起来。

Tura 有两个最常用入口：

1. `tura exec`：Rust CLI 前端。适合脚本、一次性 prompt、benchmark 和需要 stdout 稳定输出的场景。
2. `tura`：TypeScript 终端 UI。适合交互式会话，它通过 gateway 通信。

GUI 由 gateway 服务已构建的 web app，或者通过 Tauri wrapper 启动。

## 第一次运行

```bash
npm install -g tura-ai
tura exec "Inspect this workspace and summarize the architecture"
```

源码仓库开发：

```powershell
.\scripts\install.ps1
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
tura exec "Inspect this workspace"
```

## 背后启动了什么

`tura exec` 默认是 thin client：它把任务交给 `tura_router`，router 管 session DB owner，并启动 `tura_runtime` worker。`--embedded` 可以让 runtime 在 CLI 进程内执行，但常规路径是 router-backed。

代码引用：

- `crates/gateway/src/tura_exec/mod.rs`，函数 `run`。
- `crates/gateway/src/tura_exec/router.rs`，函数 `run_via_router`。
- `crates/router/src/main.rs`，函数 `main`。
- `crates/runtime/src/bin/tura_runtime.rs`，函数 `main`。

## 下一步

- [Install](install.md)
- [How to start](how-to-start.md)
- [CLI parameters](cli-parameters.md)
