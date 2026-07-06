# Graphic user interface architecture

GUI 是 Bun/Solid/Vite 应用，位于 `apps/gui`。它通过 `@tura/gateway-sdk` 访问 gateway，不直接碰 Rust crate、provider、tools 或 session DB。

## 目录

```text
apps/gui/
  app/        # Solid/Vite app
  sdk/gateway # Gateway SDK
  tests/      # unit/e2e/live tests
```

## Gateway URL

GUI 解析 gateway URL 的优先级：

1. `?gatewayUrl=<url>`。
2. `localStorage["tura.gatewayUrl"]`。
3. `VITE_TURA_GATEWAY_URL`。
4. `http://127.0.0.1:4126`。

代码引用：`apps/gui/app/src/app.tsx` 的 `App`。

## 主要职责

- 管理 session tree。
- 显示 conversation 和 tool inspector。
- 管理 workspace/project 选择。
- 调用 provider/agent/persona/config API。
- 订阅 gateway events。

代码引用：

- `apps/gui/app/src/app.tsx`，函数 `App`。
- `apps/gui/app/src/state/global-store.ts`，函数 `initialAppState`、`activeSession`。
- `apps/gui/app/src/conversation/message-tools.ts`。
- `apps/gui/sdk/gateway/src/event-source.ts`，函数 `connectGatewayEvents`。

## Tauri wrapper

Tauri wrapper 可以启动 GUI，并解析：

- `--gateway-url`
- `--workspace` / `--directory` / `--cwd`
- `--session-id` / `--initial-session`

代码引用：`apps/tauri/src-tauri/src/main.rs` 的 `parse_initial_route_params`、`start_gateway`。

## 开发命令

```bash
bun run --cwd apps/gui dev
bun run --cwd apps/gui build
bun run --cwd apps/gui test
```
