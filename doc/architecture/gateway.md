# Gateway architecture

Gateway 是 HTTP/SSE 前端服务。TUI、GUI、Tauri 都通过它访问 session、provider、agent、persona、file、command、service status 等能力。

## 主要职责

- 暴露 HTTP API。
- 提供 SSE event stream。
- 管理 session CRUD、prompt_async、abort。
- 连接 provider auth API。
- 调用 router 执行 agent turn。
- 服务构建后的 GUI 静态文件。

## 路由入口

所有主要 HTTP 路由在 `build_router` 里声明。

关键 API：

| API | 用途 |
| --- | --- |
| `GET /global/health` | 健康检查 |
| `GET /event` | 全局 SSE |
| `GET/POST /session` | session 列表和创建 |
| `POST /session/{id}/prompt_async` | 发送 prompt |
| `POST /session/{id}/abort` | abort turn |
| `GET /provider` | provider 列表 |
| `GET/POST /agent` | agent registry |
| `GET/POST /persona` | persona registry |
| `GET/POST /command` | command registry |
| `GET /service/status` | 服务状态 |

代码引用：`crates/gateway/src/web/server.rs` 的 `build_router`。

## Gateway binary

`gateway` binary 启动时会：

- 配置 release runtime env。
- 选择 listen port。
- 获取 gateway process lock。
- 启动 global router process。
- 启动 HTTP server。
- 根据情况启用 tray。

代码引用：`crates/gateway/src/bin/gateway.rs` 的 `main`、`run_gateway_server`。

## GUI 静态资源

Gateway 可以服务构建后的 GUI。它会按候选目录寻找 `index.html`，包括 `TURA_GUI_DIST` 指定目录。

代码引用：`crates/gateway/src/web/server.rs` 的 `gui_dist_dir`、`gui_dist_candidates`。
