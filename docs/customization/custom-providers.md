# Custom providers

Adding a provider starts with knowing which layout you are changing. This guide
covers both views:

- **release view**: you are using a built Tura release directory;
- **source view**: you are running or developing from the repository checkout.

Providers are the services Tura can call at runtime. For LLM work, the important
parts are the provider catalog, model tier routes, base URLs, and credentials.
For media, search, or other services, the catalog can also describe service
metadata, but metadata alone does not create a runtime adapter. A catalog entry
is a description, not an implementation with excellent confidence.

## Files and resolution order

Tura resolves the provider config in this order:

| Priority | Path |
| --- | --- |
| 1 | `TURA_PROVIDER_CONFIG`, when set and non-empty |
| 2 | `<TURA_PROJECT_ROOT>/config/provider_config.json` |
| 3 | `<TURA_PROJECT_ROOT>/crates/provider/config/provider_config.json` |
| 4 | `crates/provider/config/provider_config.json` under the current repo root |

Secrets are resolved from process environment variables plus the `.env` file.
`TURA_ENV_PATH` can point to a specific `.env`; otherwise Tura uses
`<TURA_PROJECT_ROOT>/.env` or the source root `.env`.

Do not commit real keys into `provider_config.json`. Put secrets in `.env`, in
the process environment, or set them through the provider settings UI/CLI.

## Release view

In a normal release build, `scripts/build-release.*` copies the provider config
to:

```text
<release-root>/config/provider_config.json
```

Recommended release layout:

```text
<release-root>/
  tura.exe                         # or tura / backend binaries
  config/
    provider_config.json
  .env                             # optional local secrets file
```

Start Tura with the release root as the project root:

```powershell
$env:TURA_PROJECT_ROOT = "C:\\path\\to\\tura-release"
$env:TURA_ENV_PATH = "C:\\path\\to\\tura-release\\.env"
tura provider list
```

On sh-like shells:

```sh
export TURA_PROJECT_ROOT=/path/to/tura-release
export TURA_ENV_PATH=/path/to/tura-release/.env
tura provider list
```

If you want to keep provider configuration outside the release directory, set an
explicit config path:

```powershell
$env:TURA_PROVIDER_CONFIG = "D:\\tura-config\\provider_config.json"
```

That override wins over every root-relative config path.

## Source view

From a source checkout, the default provider config is:

```text
crates/provider/config/provider_config.json
```

Recommended source workflow:

```powershell
$env:TURA_PROJECT_ROOT = "C:\\Users\\you\\Documents\\tura"
$env:TURA_ENV_PATH = "C:\\Users\\you\\Documents\\tura\\.env"
cargo run -p gateway --bin tura_exec -- "Say hello"
```

If you are testing a variant, do not edit the shared config unless that is the
change you intend to commit. Use `TURA_PROVIDER_CONFIG`:

```powershell
$env:TURA_PROVIDER_CONFIG = "C:\\tmp\\tura-provider-config.json"
cargo test -p gateway provider_config_path_uses_release_project_root_config
```

## Configuration shape

`provider_config.json` has these important top-level sections:

| Section | Purpose |
| --- | --- |
| `model_catalog` | Display metadata and model lists used by GUI/TUI pickers. |
| `provider_auth` | Saved auth metadata written by provider auth flows. Prefer env vars for secrets. |
| `provider_base_url` | Runtime base URL per provider id. |
| `provider_enums` | Catalog vocabulary for domains, capabilities, auth methods, and statuses. |
| `provider_latency` | Timeout policy. |
| `routes` | Ordered fallback routes by model tier, such as `thinking` and `fast`. |

Runtime model calls use `routes`. The provider/model picker uses
`model_catalog`. Keep both in sync when adding a provider that should be visible
and callable.

## Import a models.dev snapshot

Tura does not fetch a provider catalog during normal startup. To discover
additional OpenAI-compatible providers without editing the bundled config,
download the models.dev API snapshot and opt in with
`TURA_MODELS_DEV_CATALOG`:

```sh
curl --fail --location https://models.dev/api.json --output /path/to/models-dev-api.json
export TURA_MODELS_DEV_CATALOG=/path/to/models-dev-api.json
```

The importer keeps existing Tura routes and providers authoritative. It admits
only records using `@ai-sdk/openai-compatible` with an absolute HTTP(S) API,
declared key environment variables, and text-input/text-output models with
positive limits that do not request a native protocol, custom body, or custom
headers. Unsupported modality/protocol records are skipped. Imported models
use the generic Chat Completions adapter; native Responses, Anthropic Messages,
and Google-native records remain excluded. Malformed admitted records fail the
explicit load; provider IDs already owned by Tura are skipped rather than
silently replaced.

Use the provider/model ID from the snapshot in `TURA_SESSION_MODEL_OVERRIDE`,
including any provider prefix present in the upstream model ID. For example,
the ClinePass model `cline-pass/deepseek-v4-flash` is selected as:

```sh
CLINE_API_KEY="..." \
TURA_MODELS_DEV_CATALOG=/path/to/models-dev-api.json \
TURA_SESSION_MODEL_OVERRIDE=cline-pass/deepseek-v4-flash \
tura
```

The models.dev record declares `CLINE_API_KEY`; use that canonical name in the
local `.env` file.

The imported model metadata records the source URL and SHA-256 digest, and the
runtime checks all declared key environment names after static Tura auth
mappings.

## Add an OpenAI-compatible provider

This is the common case for local gateways, proxy services, and OpenAI-compatible
model hosts.

Add a provider entry:

```json
{
  "model_catalog": {
    "providers": {
      "local_openai": {
        "display_name": "Local OpenAI Compatible",
        "runtime_provider": "openai-compatible",
        "api_style": "openapi",
        "base_url": "http://127.0.0.1:11434/v1",
        "token_env": "LOCAL_OPENAI_API_KEY",
        "env": ["LOCAL_OPENAI_API_KEY"],
        "domains": ["llm"],
        "capabilities": ["llm.chat", "llm.tool_call"],
        "auth_methods": ["api_key"],
        "models": {
          "fast": ["local-model"],
          "thinking": ["local-model"]
        },
        "status": "local"
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

Then put the key in `.env`:

```text
LOCAL_OPENAI_API_KEY="your-key-or-placeholder"
```

For local servers that ignore authentication, use a harmless placeholder if the
adapter expects a key. Do not leave the variable unset unless you have verified
the adapter permits that.

## Add a model to an existing provider

To expose a new model in pickers, add it under the provider's catalog model tier:

```json
{
  "model_catalog": {
    "providers": {
      "openai": {
        "models": {
          "thinking": ["gpt-example-pro"],
          "fast": ["gpt-example-mini"]
        }
      }
    }
  }
}
```

To make the runtime use it by default, update the route:

```json
{
  "routes": {
    "thinking": {
      "default_temperature": 0.2,
      "providers": [
        { "provider": "openai", "model": "gpt-example-pro" }
      ]
    }
  }
}
```

If an agent has `provider.current_model` set, that exact model overrides the tier
route. Remove or update the agent override if route changes appear ignored.

## Add a new non-compatible provider

Catalog metadata is not enough. A callable provider needs a runtime adapter.

Use an existing adapter when possible:

| Need | Use `runtime_provider` |
| --- | --- |
| OpenAI-compatible chat endpoint | `openai-compatible` |
| Anthropic-compatible endpoint | `anthropic` if supported by the local adapter set |
| Google Gemini endpoint | `google` |
| AWS Bedrock endpoint | `bedrock` |

If there is no adapter for the API style, source work is required in the provider
crate. Add the adapter, wire it into the provider dispatch path, then add catalog
and route entries. A catalog-only provider may appear in settings but will fail
when called. Annoying, but honest.

## Credentials

Prefer environment variables:

```text
OPENAI_API_KEY="..."
ANTHROPIC_API_KEY="..."
LOCAL_OPENAI_API_KEY="..."
```

Useful commands:

```sh
tura provider list
tura provider status openai
tura provider set-auth openai --key "$OPENAI_API_KEY" --type api
tura provider logout openai
```

The exact variable name should match the provider catalog's `token_env` or `env`
fields.

## Validation

After changing provider config, run:

```sh
tura provider list
tura config model-tiers
tura config model-tier thinking
tura exec "Say hello using the current model"
```

From source, also run a focused provider/gateway test when the change touches
config resolution or gateway settings:

```sh
cargo test -q -p gateway provider
```

## Common failures

| Symptom | Likely cause |
| --- | --- |
| Provider missing from picker | Missing `model_catalog.providers.<id>` or no model under the selected tier. |
| Runtime still uses old model | Agent `provider.current_model` overrides the tier route. |
| Unknown provider | Missing `provider_base_url.<id>` or missing runtime adapter. |
| Auth not connected | Missing `.env` value, wrong `token_env`, expired saved auth, or wrong `TURA_ENV_PATH`. |
| Release works from one directory only | `TURA_PROJECT_ROOT` is not set, so root discovery depends on current directory/executable location. |
