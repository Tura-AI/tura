# Providers

Providers are the services Tura can call at runtime. For coding sessions, the
most important providers are LLM providers such as Codex, OpenAI, Anthropic,
Google, OpenRouter, Qwen, DeepSeek, xAI, Moonshot, Mistral, Hugging Face,
Replicate, and Bedrock. The provider catalog also describes non-LLM service
providers such as search, storage, messaging, productivity, cloud, payment, and
media APIs.

This page explains provider types, where provider configuration lives, how model
tiers choose default models, and how agents relate to providers and models.

## Configuration files

The main provider configuration is `provider_config.json`.

Tura resolves that file in this order:

| Priority | Location |
| --- | --- |
| 1 | `TURA_PROVIDER_CONFIG`, when set |
| 2 | `<TURA_PROJECT_ROOT>/config/provider_config.json`, for release layouts |
| 3 | `<TURA_PROJECT_ROOT>/crates/provider/config/provider_config.json`, for source checkouts |
| 4 | `crates/provider/config/provider_config.json` under the current repo root |

Environment values are read from the process environment and from the root
`.env` file. `TURA_ENV_PATH` can point at a different `.env` file.

Do not put secrets in docs or committed examples. Use environment variables,
the GUI provider settings, or `tura provider set-auth` for credentials.

## Provider configuration shape

`provider_config.json` has these top-level sections:

| Section | Purpose |
| --- | --- |
| `model_catalog` | Provider metadata and available models grouped by model tier. |
| `provider_auth` | Stored auth metadata written by provider login or set-auth flows. |
| `provider_base_url` | Runtime base URLs or endpoint defaults by provider id. |
| `provider_enums` | Allowed catalog vocabulary for domains, capabilities, API styles, auth methods, and statuses. |
| `provider_latency` | Timeout policy for provider calls. |
| `routes` | Ordered model-tier routes used by runtime calls. |

The runtime reads `routes` to call models. The GUI and CLI model pickers read
`model_catalog` to show selectable providers and models.

## Provider types

Provider type is described by catalog fields rather than one hard-coded enum.
The important fields are:

| Field | Meaning |
| --- | --- |
| `runtime_provider` | Runtime adapter used for calls. Examples: `openai`, `anthropic`, `google`, `openrouter`, `bedrock`, `brave_search`. |
| `api_style` | Request protocol or service style. Examples: `openai`, `anthropic`, `google`, `bedrock`, `rest`, `openapi`, `aws`. |
| `domains` | Product category. Examples: `llm`, `search`, `media_generation`, `productivity`, `infrastructure`, `communication`. |
| `capabilities` | What the provider can do, such as `llm.chat`, `llm.tool_call`, `llm.embedding`, `search.web`, `media.generation`, or `storage.object`. |
| `auth_methods` | Supported auth styles, such as `api_key`, `oauth`, `device_code`, `browser_token`, `aws_credentials`, or service-specific keys. |
| `models` | Models grouped under tiers such as `thinking`, `fast`, `embedding_high`, and `embedding_low`. |

Common groups:

| Group | Examples | Typical use |
| --- | --- | --- |
| LLM chat/tool providers | `codex`, `openai`, `anthropic`, `google`, `openrouter`, `qwen`, `deepseek`, `xai`, `moonshotai`, `mistral`, `bedrock` | Agent turns, reasoning, tool calls, streaming. |
| Embedding providers | `openai`, `codex`, `google`, `cohere`, `qwen`, `openrouter`, `together`, `huggingface` | Retrieval and embedding routes. |
| Search providers | `brave_search`, `bing`, `google_search`, `serpapi`, `tavily`, `exa`, `jina` | Web, image, news, crawl, or answer search. |
| Media providers | `replicate`, `elevenlabs`, `fal`, `stability`, `google`, `alibaba_cloud` | Image, speech, transcription, or media generation. |
| Productivity and code services | `github`, `gitlab`, `jira`, `notion`, `slack`, `feishu`, `linear` | Repository, issue, workspace, and collaboration APIs. |
| Infrastructure services | `aws`, `azure`, `cloudflare`, `aliyun_oss`, `docker_hub` | Cloud, storage, deployment, and operations APIs. |

Only providers with runtime code and credentials are callable. Catalog-only
entries can still appear as service metadata before a command integration uses
them.

## Authentication

Provider auth can come from environment variables or from `provider_auth` in the
provider config file.

Inspect providers:

```bash
tura provider list
tura provider status openai
```

Set an API key through the CLI:

```bash
tura provider set-auth openai --key "$OPENAI_API_KEY" --type api
```

Start an OAuth-style login when the provider supports it:

```bash
tura provider login codex
```

Remove saved auth for a provider:

```bash
tura provider logout openai
```

The auth object persisted by Tura has this shape:

```json
{
  "provider_auth": {
    "openai": {
      "type": "api",
      "key": "..."
    }
  }
}
```

Prefer environment variables for long-lived secrets. The provider catalog tells
you which variables are expected through `env` and `token_env`.

## Model tiers and routes

Model tiers are named routes. A route is an ordered list of provider/model
candidates. Runtime calls try the first provider, then fall back to the next one
when the failure is retryable.

The bundled tiers are:

| Tier | Use |
| --- | --- |
| `thinking` | Main reasoning and high-quality coding work. |
| `fast` | Lower-latency turns and lighter work. |
| `embedding_high` | Higher-quality embeddings. |
| `embedding_low` | Cheaper or faster embeddings. |

A route looks like this:

```json
{
  "routes": {
    "thinking": {
      "default_temperature": 0.2,
      "providers": [
        { "provider": "codex", "model": "gpt-5.6" },
        { "provider": "openai", "model": "gpt-5.6-sol" }
      ]
    }
  }
}
```

The first item in `providers` is the default model for that tier. Changing the
first item changes what agents using that tier will use unless the agent has a
specific `current_model` override.

List current tier defaults:

```bash
tura config model-tiers
```

Show selectable models for one tier:

```bash
tura config model-tier thinking
```

Set the default model for a tier:

```bash
tura config model-tier thinking openai/gpt-5.6-sol
```

That command updates the first provider in `routes.thinking.providers` in the
resolved `provider_config.json` and also patches the current workspace session
model to the same provider/model pair.

## Agents, providers, models, and tiers

Agents have their own provider block in `agents/src/<agent>/agent_config.json`.
The key fields are:

| Agent provider field | Meaning |
| --- | --- |
| `default_model_tier` | Tier name to use when the agent has no explicit current model. Usually `thinking` or `fast`. |
| `tura_llm_name` | Legacy/runtime tier name. It is kept aligned with `default_model_tier`. |
| `current_model` | Optional explicit `provider/model` override for that agent. |
| `model_reasoning_effort` | Reasoning level passed with the runtime request: `low`, `medium`, `high`, `xhigh`, or `max`; `max` is sent only to GPT-5.6 models and maps to `xhigh` for older models. |
| `model_acceleration_enabled` | Enables priority model routing for the agent when true. |
| `service_tier` | Usually `priority` when acceleration is enabled. |
| `temperature`, `stream`, `max_tokens`, `tool_choice`, `time_out_ms` | Runtime call options. |

Default relationship:

```text
agent -> default_model_tier -> routes.<tier>.providers[0] -> provider/model
```

Override relationship:

```text
agent.current_model -> provider/model
```

So if `balanced` has `default_model_tier: "thinking"`, it uses the first model
in `routes.thinking.providers`. If `balanced.provider.current_model` is set to
`openai/gpt-5.6-sol`, that exact model wins over the tier default.

Inspect an agent model binding:

```bash
tura agent model balanced
```

Set an explicit model for an agent:

```bash
tura agent model balanced openai/gpt-5.6-sol --reasoning high --priority
```

This updates the agent config by writing `provider.current_model`, preserving the
agent's existing default tier. Remove or edit `current_model` in the agent config
if you want the agent to go back to the tier default.

## Modify provider configuration safely

Prefer this order:

1. Use the GUI settings page for provider auth and model tier selection.
2. Use CLI commands for repeatable changes.
3. Edit `provider_config.json` only when adding providers, changing base URLs,
   editing catalog metadata, or changing fallback order by hand.

Useful CLI commands:

```bash
tura provider list
tura provider status openai
tura provider set-auth openai --key "$OPENAI_API_KEY" --type api
tura config model-tiers
tura config model-tier thinking openai/gpt-5.6-sol
tura agent model balanced openai/gpt-5.6-sol --reasoning high --priority
```

When editing JSON manually:

- Change `provider_base_url.<provider>` to point a runtime provider at a custom
  endpoint.
- Change `routes.<tier>.providers` to reorder fallback candidates.
- Add models under `model_catalog.providers.<provider>.models.<tier>` so the GUI
  and CLI picker can show them.
- Add or adjust provider metadata under `model_catalog.providers.<provider>`.
- Keep secrets out of committed config. Use environment variables or auth flows
  for keys and tokens.

Minimal custom OpenAI-compatible provider example:

```json
{
  "model_catalog": {
    "providers": {
      "local_openai": {
        "display_name": "Local OpenAI Compatible",
        "runtime_provider": "openai",
        "api_style": "openai_compatible",
        "base_url": "http://127.0.0.1:11434/v1",
        "token_env": "LOCAL_OPENAI_API_KEY",
        "env": ["LOCAL_OPENAI_API_KEY"],
        "domains": ["llm"],
        "capabilities": ["llm.chat", "llm.tool_call"],
        "auth_methods": ["api_key"],
        "models": {
          "fast": ["local-model"],
          "thinking": ["local-model"]
        }
      }
    }
  },
  "provider_base_url": {
    "local_openai": "http://127.0.0.1:11434/v1"
  },
  "routes": {
    "fast": {
      "default_temperature": 0.2,
      "providers": [
        { "provider": "local_openai", "model": "local-model" }
      ]
    }
  }
}
```

For a provider that should reuse an existing runtime adapter, set
`runtime_provider` to that adapter and make sure `provider_base_url` contains a
matching provider id. If the adapter itself does not exist, catalog metadata
alone will not make the provider callable. Annoying, but physics remains in
charge.

## Validation checklist

After changing provider config:

```bash
tura provider list
tura config model-tiers
tura config model-tier thinking
tura exec "Say hello using the current model"
```

If a provider fails, check these first:

| Symptom | Likely cause |
| --- | --- |
| Provider missing from picker | Missing `model_catalog.providers.<id>` or no model under the selected tier. |
| Model tier does not change runtime model | Agent has `provider.current_model` set, overriding the tier. |
| Unknown provider error | Missing `provider_base_url.<id>` or no runtime adapter for that provider id. |
| Auth shows not connected | Missing env var, missing `provider_auth` entry, expired OAuth token, or wrong auth method. |
| Model hidden in GUI/CLI | Catalog model has `visible: false`, or the picker filters Claude-like model ids. |
