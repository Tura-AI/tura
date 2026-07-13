# Providers

Providers are the services Tura can call at runtime. For coding sessions, the
most important providers are LLM providers such as Codex, OpenAI, Anthropic,
Google, OpenRouter, Qwen, DeepSeek, xAI, Moonshot, Mistral, Hugging Face,
Replicate, and Bedrock. The provider catalog also describes non-LLM service
providers such as search, storage, messaging, productivity, cloud, payment, and
media APIs. Configuration tells Tura where a service is and how to reach it; it
does not, by itself, prove that every path works in production.

This page owns first-run authentication and model selection for the CLI, TUI,
and GUI. It also explains provider types, configuration storage, model tiers,
and the relationship between agents, providers, and models.

## First run: configure an LLM provider

Installing Tura does not configure a remote model. Before the first prompt, you
must authenticate at least one callable LLM provider and select a model that
belongs to that provider. Starting `tura` is safe before authentication because
it opens the TUI without sending a model request.

Authentication and selection are separate:

1. **Authenticate** with an API key/token or a supported OAuth login.
2. **Select a model** from that authenticated provider for the workspace, model
   tier, agent, or one CLI invocation.
3. **Verify** the auth status, then send a small test prompt through the selected
   provider/model.

A provider showing **Connected** does not by itself change the current model.
Likewise, selecting a model does not create credentials. Both steps are needed.

Common LLM credential names are:

| Provider choice    | Provider id   | Typical authentication                        | Credential variable                                                          |
| ------------------ | ------------- | --------------------------------------------- | ---------------------------------------------------------------------------- |
| Codex subscription | `codex`       | OAuth login                                   | Managed by the login flow; the access token is stored under `OPENAI_API_KEY` |
| OpenAI API         | `openai`      | API key                                       | `OPENAI_API_KEY`                                                             |
| Anthropic API      | `anthropic`   | API key                                       | `ANTHROPIC_API_KEY`                                                          |
| Claude Code        | `claude-code` | OAuth login or supported local auth discovery | `CLAUDE_CODE_OAUTH_TOKEN`                                                    |
| Google API         | `google`      | API key or supported OAuth login              | `GOOGLE_API_KEY`                                                             |
| Gemini API         | `gemini`      | API key or supported OAuth login              | `GEMINI_API_KEY`                                                             |
| OpenRouter         | `openrouter`  | API key                                       | `OPENROUTER_API_KEY`                                                         |
| DeepSeek           | `deepseek`    | API key                                       | `DEEPSEEK_API_KEY`                                                           |

The provider list is broader than this table. Use the TUI or GUI provider detail
to see the exact auth methods and credential variable for a provider. Catalog
entries without a runtime adapter are not enough to run a model.

### TUI setup (recommended for first run)

1. Run `tura` with no prompt.
2. If no LLM provider is configured, Tura opens **Settings > Provider**
   automatically. Otherwise enter `/settings` and open **Provider**, or enter
   `/provider` directly.
3. Select an LLM provider with Up/Down and press Enter. The detail page shows
   each supported auth method and its credential variable.
4. For an API key/token, select the key method, press Enter, type the secret into
   the dedicated settings input, and press Enter again. Tura validates the
   credential before saving it. The value is visible while you type, so do not
   share or record the terminal during this step.
5. For OAuth, select **OAuth login**, finish the browser/device flow, and return
   to Tura. Automatic callbacks are detected; manual flows ask for the returned
   code or token.
6. Return to **Settings > Model** and select a concrete `provider/model` pair
   from the provider you just authenticated. The command equivalent inside the
   TUI is `/model provider/model`.
7. Confirm that the provider is connected and the selected model is shown, then
   return to chat and send the first prompt.

You can reopen a provider directly with `/provider PROVIDER_ID`. The chat command
`/provider set-auth PROVIDER_ID --key KEY` also exists, but the settings flow is
preferable because it does not put the literal key in a slash command.

### GUI setup

1. Start `tura_gui`, or start `tura_gateway` and open its local URL. With no
   configured LLM provider, the GUI opens **Settings > Providers** automatically.
2. Keep the domain filter on **LLM**, or search for the provider. Select it under
   **Unconfigured providers**.
3. For an API key/token, use the provider's credential field and click **Save**.
   The field is masked by default but reveals its value while focused or hovered.
   The dialog validates the credential and shows the resulting status. When
   available, **Provider API page** opens the provider's key-management page.
4. For OAuth, click **OAuth login**, complete the external flow, and enter a code
   only when the dialog requests one. Wait for the provider status to become
   connected.
5. Open **Settings > Default model config** and choose a model from that provider
   for the `thinking` and/or `fast` tier. This also updates the current workspace
   model. Use **Settings > Agents** only when you intentionally want an agent to
   carry its own provider/model override.
6. Return to the conversation and send the first prompt.

### CLI setup

First inspect the available provider ids and auth state:

```bash
tura provider list
tura provider status openai
```

For an API provider, acquire a key from that provider, place it in a temporary
shell variable, and let `set-auth` validate and persist it. Do not paste a real
secret into documentation, scripts, or a committed file.

PowerShell example:

```powershell
$secret = Read-Host "OpenAI API key" -AsSecureString
$env:OPENAI_API_KEY = [System.Net.NetworkCredential]::new("", $secret).Password
tura provider set-auth openai --key $env:OPENAI_API_KEY --type api
Remove-Item Env:OPENAI_API_KEY
```

macOS/Linux shell example:

```bash
read -rsp 'OpenAI API key: ' OPENAI_API_KEY; printf '\n'
tura provider set-auth openai --key "$OPENAI_API_KEY" --type api
unset OPENAI_API_KEY
```

`set-auth` saves only after validation succeeds. It writes the provider's
configured credential variable to the `.env` resolved by `TURA_ENV_PATH`, or to
the runtime project root `.env` by default. That file is ignored by this
repository. If exposing a token in process arguments is unacceptable on your
machine, use the TUI settings flow or GUI credential field instead and keep the
screen private while entering it.

For a provider with supported OAuth, use its login flow instead of `set-auth`:

```bash
tura provider login codex
tura provider status codex
```

Next list model choices and select one. Replace `MODEL_ID` with an id printed by
the preceding command:

```bash
tura config model-tier thinking
tura config set model=openai/MODEL_ID
tura provider status openai
tura exec -m openai/MODEL_ID "Reply with OK and identify the active model"
```

The commands have different scopes:

| Command                                      | Scope                                                         |
| -------------------------------------------- | ------------------------------------------------------------- |
| `tura config set model=PROVIDER/MODEL`       | Current workspace model selection.                            |
| `tura config model-tier TIER PROVIDER/MODEL` | Shared default for that tier and current workspace selection. |
| `tura agent model AGENT PROVIDER/MODEL`      | Explicit model override stored on that agent.                 |
| `tura exec -m PROVIDER/MODEL "prompt"`       | One direct CLI invocation.                                    |
| `tura run -m PROVIDER/MODEL "prompt"`        | One gateway-backed CLI invocation.                            |

If the final test reaches a different provider, check the workspace model and
the selected agent's explicit model. Credentials make a provider available;
they do not silently rewrite routing.

## Configuration files

The main provider configuration is `provider_config.json`.

Tura resolves that file in this order:

| Priority | Location                                                                                |
| -------- | --------------------------------------------------------------------------------------- |
| 1        | `TURA_PROVIDER_CONFIG`, when set                                                        |
| 2        | `<TURA_PROJECT_ROOT>/config/provider_config.json`, for release layouts                  |
| 3        | `<TURA_PROJECT_ROOT>/crates/provider/config/provider_config.json`, for source checkouts |
| 4        | `crates/provider/config/provider_config.json` under the current repo root               |

Environment values are read from the process environment and from the root
`.env` file. `TURA_ENV_PATH` can point at a different `.env` file.

Do not put secrets in docs or committed examples. Use environment variables,
the GUI provider settings, or `tura provider set-auth` for credentials.

## Provider configuration shape

`provider_config.json` has these top-level sections:

| Section             | Purpose                                                                                       |
| ------------------- | --------------------------------------------------------------------------------------------- |
| `model_catalog`     | Provider metadata and available models grouped by model tier.                                 |
| `provider_auth`     | Stored auth metadata written by provider login or set-auth flows.                             |
| `provider_base_url` | Runtime base URLs or endpoint defaults by provider id.                                        |
| `provider_enums`    | Allowed catalog vocabulary for domains, capabilities, API styles, auth methods, and statuses. |
| `provider_latency`  | Timeout policy for provider calls.                                                            |
| `routes`            | Ordered model-tier routes used by runtime calls.                                              |

The runtime reads `routes` to call models. The GUI and CLI model pickers read
`model_catalog` to show selectable providers and models.

## Provider types

Provider type is described by catalog fields rather than one hard-coded enum.
The important fields are:

| Field              | Meaning                                                                                                                                |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| `runtime_provider` | Runtime adapter used for calls. Examples: `openai`, `anthropic`, `google`, `openrouter`, `bedrock`, `brave_search`.                    |
| `api_style`        | Request protocol or service style. Examples: `openai`, `anthropic`, `google`, `bedrock`, `rest`, `openapi`, `aws`.                     |
| `domains`          | Product category. Examples: `llm`, `search`, `media_generation`, `productivity`, `infrastructure`, `communication`.                    |
| `capabilities`     | What the provider can do, such as `llm.chat`, `llm.tool_call`, `llm.embedding`, `search.web`, `media.generation`, or `storage.object`. |
| `auth_methods`     | Supported auth styles, such as `api_key`, `oauth`, `device_code`, `browser_token`, `aws_credentials`, or service-specific keys.        |
| `models`           | Models grouped under tiers such as `thinking`, `fast`, `embedding_high`, and `embedding_low`.                                          |

Common groups:

| Group                          | Examples                                                                                                              | Typical use                                           |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| LLM chat/tool providers        | `codex`, `openai`, `anthropic`, `google`, `openrouter`, `qwen`, `deepseek`, `xai`, `moonshotai`, `mistral`, `bedrock` | Agent turns, reasoning, tool calls, streaming.        |
| Embedding providers            | `openai`, `codex`, `google`, `cohere`, `qwen`, `openrouter`, `together`, `huggingface`                                | Retrieval and embedding routes.                       |
| Search providers               | `brave_search`, `bing`, `google_search`, `serpapi`, `tavily`, `exa`, `jina`                                           | Web, image, news, crawl, or answer search.            |
| Media providers                | `replicate`, `elevenlabs`, `fal`, `stability`, `google`, `alibaba_cloud`                                              | Image, speech, transcription, or media generation.    |
| Productivity and code services | `github`, `gitlab`, `jira`, `notion`, `slack`, `feishu`, `linear`                                                     | Repository, issue, workspace, and collaboration APIs. |
| Infrastructure services        | `aws`, `azure`, `cloudflare`, `aliyun_oss`, `docker_hub`                                                              | Cloud, storage, deployment, and operations APIs.      |

Only providers with runtime code and credentials are callable. Catalog-only
entries can still appear as service metadata before a command integration uses
them.

## Credential storage and auth command reference

Provider auth can come from environment variables or from `provider_auth` in the
provider config file. For the complete first-run sequence, including model
selection, use [First run](#first-run-configure-an-llm-provider).

Inspect providers:

```bash
tura provider list
tura provider status openai
```

Set an API key already held in a shell variable:

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

The provider catalog tells you which variables are expected through `env` and
`token_env`. Use Tura's auth flows when persistence is required; a process-only
environment variable disappears when that shell exits.

## Model tiers and routes

Model tiers are named routes. A route is an ordered list of provider/model
candidates. Runtime calls try the first provider, then fall back to the next one
when the failure is retryable.

The bundled tiers are:

| Tier             | Use                                          |
| ---------------- | -------------------------------------------- |
| `thinking`       | Main reasoning and high-quality coding work. |
| `fast`           | Lower-latency turns and lighter work.        |
| `embedding_high` | Higher-quality embeddings.                   |
| `embedding_low`  | Cheaper or faster embeddings.                |

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

| Agent provider field                                                | Meaning                                                                                                                                                                 |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `default_model_tier`                                                | Tier name to use when the agent has no explicit current model. Usually `thinking` or `fast`.                                                                            |
| `tura_llm_name`                                                     | Legacy/runtime tier name. It is kept aligned with `default_model_tier`.                                                                                                 |
| `current_model`                                                     | Optional explicit `provider/model` override for that agent.                                                                                                             |
| `model_reasoning_effort`                                            | Reasoning level passed with the runtime request: `low`, `medium`, `high`, `xhigh`, or `max`; `max` is sent only to GPT-5.6 models and maps to `xhigh` for older models. |
| `model_acceleration_enabled`                                        | Enables priority model routing for the agent when true.                                                                                                                 |
| `service_tier`                                                      | Usually `priority` when acceleration is enabled.                                                                                                                        |
| `temperature`, `stream`, `max_tokens`, `tool_choice`, `time_out_ms` | Runtime call options.                                                                                                                                                   |

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
      "providers": [{ "provider": "local_openai", "model": "local-model" }]
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

| Symptom                                  | Likely cause                                                                               |
| ---------------------------------------- | ------------------------------------------------------------------------------------------ |
| Provider missing from picker             | Missing `model_catalog.providers.<id>` or no model under the selected tier.                |
| Model tier does not change runtime model | Agent has `provider.current_model` set, overriding the tier.                               |
| Unknown provider error                   | Missing `provider_base_url.<id>` or no runtime adapter for that provider id.               |
| Auth shows not connected                 | Missing env var, missing `provider_auth` entry, expired OAuth token, or wrong auth method. |
| Model hidden in GUI/CLI                  | Catalog model has `visible: false`, or the picker filters Claude-like model ids.           |
