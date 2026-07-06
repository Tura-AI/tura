# Session DB architecture

Session DB 是 Tura 的持久记忆层。它保存 session、消息、task management、workspace session history 和 durable write queue。

## 进程角色

`tura_session_db` 是 SQLite store 的 owner。其他进程通过 socket 访问它，不应该直接写 session DB。

代码引用：

- `crates/session_log/src/bin/tura_session_db.rs`，函数 `main`。
- `crates/session_log/src/store.rs`，结构 `SessionLogStore`，函数 `open_default`、`open`。
- `crates/session_log/src/service.rs`，函数 `run_socket_service`。

## 数据位置

`SessionLogStore` 使用 `index.sqlite3` 作为 index DB 文件。默认目录由 `tura_path::default_db_dir()` 决定。

架构文档里还说明：workspace 的完整 session log 可以保存在 `<workspace>/.tura/session_log.sqlite3`，而 per-home socket/locks/index 隔离。

## Gateway 和 runtime 怎么用

Gateway 不直接写 `.tura/sessions/*.json`。它通过 session_log client 写 session info、message、todo、parent links。Runtime 恢复 gateway session 时也通过 session_log client 读取。

代码引用：

- `crates/gateway/src/api/session.rs`，函数 `write_session_log_command`。
- `crates/runtime/src/session_bootstrap/persisted.rs`，函数 `load_persisted_gateway_session`。
- `crates/runtime/src/session_log_client.rs`。

## 查询入口

Gateway HTTP：

```text
GET /session-log/workspaces
GET /session-log/sessions?workspace=C%3A%2Frepo&page=0&page_size=50
GET /session-log/{sessionID}/records?page=0&page_size=100
```

代码引用：`crates/gateway/src/web/server.rs` 的 `build_router`。
