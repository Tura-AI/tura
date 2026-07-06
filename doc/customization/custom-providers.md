# Custom providers

Provider 配置主要来自 `crates/provider/config/provider_config.json`。如果只是设置 API key，用 CLI；如果要新增 provider/model，再改 config。

## 设置认证

```bash
tura provider set-auth openai --key "$OPENAI_API_KEY"
tura provider login openai
tura provider status openai
```

代码引用：`apps/tui/src/commands/provider.ts` 的 `providerCommand`、`parseProviderAuthArgs`。

## 新增 provider 的配置形状

在 `model_catalog.providers` 下添加 provider：

```json
{
  "display_name": "Example Provider",
  "api_style": "openai_compatible",
  "auth_methods": ["api_key"],
  "base_url": "https://api.example.com/v1",
  "env": ["EXAMPLE_API_KEY"],
  "capabilities": ["llm.chat", "llm.tool_call"],
  "models": {
    "thinking": [
      {
        "id": "example-reasoner",
        "name": "example-reasoner",
        "tool_call": true,
        "reasoning": true,
        "modalities": { "input": ["text"], "output": ["text"] }
      }
    ]
  },
  "runtime_provider": "example",
  "token_env": "EXAMPLE_API_KEY"
}
```

## 代码边界

Provider crate 负责：

- route lookup。
- auth/token resolution。
- OAuth。
- streaming/non-streaming call。
- response normalization。
- tool-call normalization。
- usage/cost/logging。

代码引用：

- `crates/provider/src/lib.rs`。
- `crates/provider/src/tura_llm.rs`。
- `crates/provider/src/response_extraction.rs`。
- `crates/gateway/src/api/provider/catalog.rs`。

## 注意

不要把 secret 写进公开配置文件。配置里放 env key 名称，真正的 key 放环境变量或 auth store。
