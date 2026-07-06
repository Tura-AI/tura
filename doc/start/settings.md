# Settings

Tura 的设置分几层，不要混在一起看。混在一起会很快变成配置沼泽，沼泽通常不回消息。

## 1. npm/release 设置

这些设置影响 `npm/tura.mjs` 如何找到 release binary：

| 环境变量 | 作用 |
| --- | --- |
| `TURA_NPM_SKIP_CLI_REGISTRATION` | 安装时跳过 PATH/profile 注册 |
| `TURA_NPM_RELEASE_ARCHIVE` | 使用指定 release archive 安装 |
| `TURA_RELEASE_BIN_DIR` | 指定 release binary 目录 |
| `TURA_PROJECT_ROOT` | 指定项目根目录 |
| `TURA_PROVIDER_CONFIG` | 指定 provider config 文件 |

代码引用：`npm/tura.mjs` 的 `installedReleaseDir`。

## 2. Runtime/CLI 设置

`tura exec` 的配置通过 CLI 参数和环境变量进入 runtime。

例子：

```bash
tura exec --goal --planning on -c command_run_shell=zsh "Port this CLI carefully"
```

常见 runtime override：

- `model_reasoning_effort`
- `max_tokens`
- `model_max_tokens`
- `planning=auto|on|off`
- `command_run_shell=bash|zsh|shll`

代码引用：`crates/gateway/src/tura_exec/cli.rs` 的 `apply_config_arg`、`parse_planning_mode`。

## 3. Workspace session config

TUI CLI 的 `config` 命令读写 gateway 的 `/session/config`：

```bash
tura config get
tura config get model
tura config set model=openai/gpt-5 agent=balanced
```

代码引用：

- `apps/tui/src/commands/config.ts`，函数 `configCommand`。
- `crates/gateway/src/api/session.rs`，函数 `get_session_config`、`patch_session_config`。

## 4. Provider model tier config

Provider 和模型目录来自 `crates/provider/config/provider_config.json`。TUI CLI 可以查看或修改 model tier：

```bash
tura config model-tiers --json
tura config model-tier thinking
tura config model-tier thinking openai/gpt-5
```

代码引用：

- `crates/provider/config/provider_config.json`。
- `apps/tui/src/commands/config.ts`，函数 `parseProviderModel`。
- `crates/gateway/src/api/global.rs`，函数 `get_tura_config`、`put_tura_config`。

## 5. GUI 设置

GUI 设置通过 gateway API 读取和修改，不直接读 `.env`、provider config、session DB。

Gateway URL 优先级：

1. URL 参数 `?gatewayUrl=<url>`。
2. `localStorage["tura.gatewayUrl"]`。
3. `VITE_TURA_GATEWAY_URL`。
4. 默认 `http://127.0.0.1:4126`。

代码引用：`apps/gui/app/src/app.tsx` 的 `App`，`apps/gui/sdk/gateway/src` 的 Gateway SDK。
