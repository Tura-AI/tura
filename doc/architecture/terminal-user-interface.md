# Terminal user interface architecture

TUI 是 TypeScript 终端客户端，位于 `apps/tui`。它既提供交互式终端 UI，也提供 `tura run/session/provider/...` 这类非交互命令。

## 边界

TUI 不直接调用 runtime、provider、tools、session DB。它只通过 gateway HTTP/SSE 工作。

代码引用：

- `apps/tui/src/index.ts`，入口。
- `apps/tui/src/cli.ts`，函数 `main`、`parseGlobal`、`parseRun`。
- `apps/tui/src/gateway/client.ts`，Gateway client。
- `apps/tui/src/tui/app.ts`，交互式 UI。

## 能力

- 发送 prompt。
- 创建/恢复 session。
- 显示 messages 和 tool summary。
- 列出 provider/model。
- 设置 session config。
- provider auth/login/logout。
- 查看 gateway status。

## 渲染层级

TUI 支持三档能力：

| Level | 说明 | 适合环境 |
| --- | --- | --- |
| L1 plain | 纯文本、安全、无颜色 | CI、非 TTY、`TERM=dumb` |
| L2 ANSI | 普通 ANSI 终端 | 常规终端 |
| L3 rich | 更丰富的布局/链接/markdown | 现代终端、xterm.js |

代码引用：`apps/tui/src/tui/capabilities.ts`。

## CLI help 来源

帮助文本在 locale JSON 里，不是硬编码在 README：

```text
apps/tui/src/locales/help.en.json
apps/tui/src/locales/help.zh-CN.json
```

代码引用：`apps/tui/src/i18n-help.ts` 的 `helpPage`。
