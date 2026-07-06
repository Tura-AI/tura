# Providers

Provider 是模型和外部服务的配置层。它负责模型目录、认证、OAuth、token 刷新、provider 调用、响应归一化和 provider 日志。

## 查看 provider

```bash
tura provider list
tura provider list --json
tura provider status
tura provider status openai
```

代码引用：

- `apps/tui/src/commands/provider.ts`，函数 `providerCommand`。
- `crates/gateway/src/api/provider/catalog.rs`，函数 `list_providers_value`。

## 设置 API key

```bash
tura provider set-auth openai --key "$OPENAI_API_KEY"
tura provider set-auth anthropic --key "$ANTHROPIC_API_KEY"
```

也可以传 JSON 或 JSON 文件：

```bash
tura provider set-auth openai --auth '{"type":"api","key":"sk-..."}'
```

代码引用：`apps/tui/src/commands/provider.ts` 的 `parseProviderAuthArgs`。

## OAuth 登录

```bash
tura provider login openai
tura provider login github-copilot --no-open
```

OAuth 支持和 callback 由 gateway/provider API 处理。

代码引用：

- `crates/gateway/src/api/provider/oauth_support.rs`。
- `crates/gateway/src/api/provider/oauth_exchange.rs`。
- `crates/gateway/src/api/provider.rs`，函数 `oauth_authorize`、`oauth_callback`、`provider_auth_logout_value`。

## Provider config 文件

模型目录在：

```text
crates/provider/config/provider_config.json
```

里面按 provider 声明 display name、base URL、auth method、env、capabilities、models、runtime provider 等。

例子结构：

```json
{
  "model_catalog": {
    "providers": {
      "anthropic": {
        "base_url": "https://api.anthropic.com/v1",
        "env": ["ANTHROPIC_API_KEY"],
        "models": { "thinking": [] }
      }
    }
  }
}
```

## Provider 日志

Provider 调用日志默认写到：

```text
log/provider/YYYY-MM-DD/*.json
```

`LOG_PATH` 可以覆盖日志目录。

代码引用：`crates/provider/src/logging.rs`，`crates/provider/src/metrics.rs`。
