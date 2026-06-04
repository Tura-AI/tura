# Tura Provider Crate Architecture

Provider is the model access, authentication, routing, usage accounting, and
provider-control crate for Tura. It is used by `crates/runtime` to execute one
model call, and by gateway/config surfaces to inspect provider settings, auth
state, usage, and health.

The Cargo package and library names should stay compatible with Tura:

```text
package = tura-llm-rust
library = tura_llm_rust
```

This crate keeps compatibility with the current Tura provider behavior:

- route-based provider configuration
- `provider_config.json`
- `.env` / `TURA_ENV_PATH`
- `TURA_PROVIDER_CONFIG` (preferred), `TURALLM_CONFIG` (legacy)
- provider call logs under project-root `log/provider/`
- OpenAI-compatible, Google, and Bedrock providers
- usage/cost extraction already present in provider responses
- OpenAI OAuth refresh behavior already present in `tura_llm.rs`

It also adopts the cleaner separation seen in Codex current:

- auth and token storage are separate from provider calls
- model catalog/presets are separate from provider adapters
- request and response normalization are separate
- usage, rate limits, and monitoring are first-class
- settings and path resolution are explicit

## Directory Layout

```text
crates/provider/
  Cargo.toml
  ARCHITECTURE.md
  config/
    provider_config.json
  tests/
    provider_tool_call_live_smoke.rs

  src/
    lib.rs
    mod.rs
    auth_registry.rs
    content_type_fallback.rs 
    response_extraction.rs    
    tura_conf.rs
    tura_llm_conf.rs
    tura_llm.rs
    streaming.rs
    metrics.rs
    logging.rs
    utils/
      mod.rs
    llm/
      mod.rs
      openapi.rs
      google.rs
      bedrock.rs
      providers/
        mod.rs
        bedrock.rs
        codex.rs
        google.rs
        minimax.rs
        openai.rs
```

The auth/config/models/routing/request/response/streaming/usage/logging
subdomains described below are the target separation. The current source tree
keeps API styles under `src/llm/`, provider-specific branches under
`src/llm/providers/`, and cross-API streaming/metrics/utils/logging at provider
crate root.

## Ownership

Provider owns:

- provider configuration loading
- model/provider routing
- provider adapters
- auth state and token refresh
- request validation and normalization
- response normalization
- streaming event normalization
- usage and cost accounting
- call logging
- provider health and rate-limit monitoring
- provider control state

Provider does not own:

- agent turn loop
- context construction
- tool execution
- user-facing session/thread API
- gateway event streaming
- `command_run` command execution

Runtime calls Provider for one model call. Gateway reads Provider settings,
health, usage, and auth state through APIs.

## Provider Call Logs

Provider call logs are file-based diagnostics owned only by this crate. The
default log root is project-root `log/provider/`; override it with `LOG_PATH`.
Files are written under a local-day directory:

```text
log/provider/YYYY-MM-DD/HHMMSS_mmm_<call_id>.json
```

Each file is a JSON `llm_call` record from `src/logging.rs` with:

```text
type
call_id
success
provider
model
base_url
started_at / finished_at / duration_ms
request.messages / request.params / request.response_format
response
metrics
error / traceback
```

Use provider logs to inspect raw model-call request/response behavior,
streaming normalization, provider-specific failures, usage extraction, and
redaction issues. Do not use provider logs as durable session history; session,
task, message, todo, and replay history belong in the `session_log` database.

Fast PowerShell inspection examples:

```powershell
Get-ChildItem log\provider -Recurse -Filter *.json | Sort-Object LastWriteTime -Descending | Select-Object -First 10 FullName
Get-Content log\provider\YYYY-MM-DD\HHMMSS_mmm_callid.json | ConvertFrom-Json | Select-Object provider,model,success,duration_ms,error
```

## Existing Tura Mapping

Current files should be split conceptually as follows when implementation begins:

```text
tura_conf.rs
  -> config/loader
  -> config/settings
  -> storage/config_store

tura_llm_conf.rs
  -> config/loader
  -> routing/routes
  -> config/path_compat

tura_llm.rs
  -> request/*
  -> routing/*
  -> auth/*
  -> providers/*
  -> response/*
  -> usage/*
  -> logging/*

streaming.rs
  -> cross-API provider request and stream timeout helpers

metrics.rs
  -> cross-API usage, cost, and context-utilization extraction

logging.rs
  -> logging/call_log
  -> logging/redaction
  -> logging/retention

utils/
  -> common JSON/schema helpers

llm/openapi.rs
  -> OpenAI-compatible API request and response normalization

llm/google.rs
  -> Google Gemini API request and response normalization

llm/bedrock.rs
  -> Bedrock API request and response normalization

llm/providers/openai.rs
  -> OpenAI API provider entrypoint

llm/providers/codex.rs
  -> Codex / ChatGPT Responses OAuth provider entrypoint

llm/providers/minimax.rs
  -> Minimax provider entrypoint backed by OpenAI-compatible transport

llm/providers/google.rs
  -> Google provider-specific entrypoint backed by llm/google.rs

llm/providers/bedrock.rs
  -> Bedrock provider-specific entrypoint backed by llm/bedrock.rs
```

## Compatibility With `tura_path`

Provider must not invent independent path rules. It should use the path contract
from `tura_path` for project-root-aware paths.

Supported compatibility inputs:

- `TURA_ENV_PATH`
- `TURA_PROVIDER_CONFIG` (preferred), `TURALLM_CONFIG` (legacy)
- project config from `tura_path`
- provider `config/provider_config.json`
- project-root `log/provider/YYYY-MM-DD/...json`

Path ownership:

```text
config/path_compat/
  resolves provider config path
  resolves env path
  resolves log root
  resolves cache root
  maps old provider paths to new paths
```

Rules:

- Environment variables override file config.
- Explicit runtime/session provider config overrides defaults.
- Missing config should return typed errors, not panic.
- Path resolution should be deterministic and testable.
- Log and usage stores should support both legacy provider `log/` and new global
  Tura storage paths.

## Configuration Model

Provider config has three layers:

1. Global defaults
2. Provider profiles/routes
3. Runtime-call overrides

Conceptual config:

```toml
[provider.defaults]
timeout_ms = 480000
service_tier = "auto"
reasoning_effort = "low"
stream = true

[[provider.routes.flagship_thinking.providers]]
provider = "openai"
base_url = "https://api.openai.com/v1"
model = "gpt-5.1-codex"
temperature = 0.2
priority = 100

[[provider.routes.flagship_thinking.providers]]
provider = "google"
base_url = "https://generativelanguage.googleapis.com"
model = "gemini-..."
temperature = 0.2
priority = 50
```

Existing JSON route config remains supported:

```text
config/provider_config.json
```

## Manual Provider Configuration

The bundled provider catalog and routing config lives at:

```text
crates/provider/config/provider_config.json
```

Override the file path only when needed:

- `TURA_PROVIDER_CONFIG`: preferred explicit provider config path.
- `TURALLM_CONFIG`: legacy explicit provider config path.

Runtime environment values are loaded from the project-root `.env` by default.
`TURA_ENV_PATH` can point to another dotenv file, but normal project setup
should keep provider secrets and local runtime values in the root `.env`.

To add a model on an existing provider:

1. Add or update the provider entry under `model_catalog.providers`.
2. Add the model id, display metadata, cost/context values, and capability
   flags expected by the gateway/model picker.
3. Add the model as a candidate in one or more `routes` entries.
4. Keep route names stable so existing agent configs continue to use names such
   as `flagship_thinking`, `thinking`, `fast`, or `instant`.
5. Add or update live smoke tests only when the provider/model can be tested in
   the current environment.

To add a new OpenAI-compatible provider:

1. Add its base URL under `provider_base_url`.
2. Add its models under `model_catalog.providers`.
3. Add route candidates that reference the provider id and model id.
4. Add the expected API key name to auth configuration or document the
   environment variable, normally `{PROVIDER}_API_KEY`.
5. Verify the provider through the OpenAI-compatible transport before creating a
   dedicated adapter.

To add a provider that is not compatible with the existing transports:

1. Add a provider adapter under `src/llm/providers/`.
2. Add any shared request/response normalization under `src/llm/`.
3. Register the adapter in `src/llm/providers/mod.rs`.
4. Keep provider-specific wire parsing inside this crate.
5. Expose normalized text, tool calls, usage, and streaming events through the
   provider crate API consumed by runtime.

Provider config is fixed route/catalog configuration. User preferences such as
the selected session model or selected agent belong in workspace/session config
and should not be merged into `provider_config.json`.

## Auth Architecture

### `auth/api_key/`

API-key provider auth.

Responsibilities:

- resolve keys from env/config/secret store
- support provider-specific key names
- validate presence without leaking values
- expose masked key status

Existing compatible names:

- `OPENAI_API_KEY`
- `{PROVIDER}_API_KEY`
- legacy lowercase variants if needed

### `auth/oauth/`

OAuth login and refresh flows.

Responsibilities:

- start login
- complete login
- refresh tokens
- revoke/logout
- validate token expiry
- expose auth state

OpenAI OAuth compatibility:

- `OPENAI_LOGIN=oauth`
- `OPENAI_API_KEY` as access token
- `OPENAI_REFRESH_TOKEN`
- `OPENAI_TOKEN_EXPIRES`
- base URL allowlist for OAuth token use

OAuth state should not be hidden inside provider call code.

### `auth/login_state/`

Multi-state login model.

```rust
enum AuthState {
    Unknown,
    NotConfigured,
    ApiKeyConfigured,
    OAuthStarting,
    OAuthWaitingForBrowser,
    OAuthWaitingForCallback,
    OAuthAuthenticated,
    OAuthRefreshing,
    Expired,
    Revoking,
    Revoked,
    Failed,
}
```

Auth state owns:

- provider
- account id if known
- login method
- token expiry
- last refresh time
- failure reason
- whether user action is required

### `auth/token_store/`

Token persistence.

Rules:

- Prefer OS keyring/secret store when available.
- Fall back to encrypted or restricted-permission local storage only when needed.
- Never write raw tokens to call logs.
- Token reads/writes must be auditable without exposing secrets.

## Provider State

Provider-level operational state:

```rust
enum ProviderState {
    Unknown,
    Disabled,
    Configured,
    MissingAuth,
    Ready,
    Degraded,
    RateLimited,
    Paused,
    Failed,
}
```

This is different from auth state. A provider may be authenticated but paused,
rate-limited, or degraded.

## Call State

One provider call:

```rust
enum ProviderCallState {
    Created,
    BuildingRequest,
    ResolvingAuth,
    Dispatching,
    WaitingFirstToken,
    Streaming,
    Receiving,
    NormalizingResponse,
    RecordingUsage,
    Logging,
    Succeeded,
    TimedOut,
    Failed,
    Cancelled,
}
```

Runtime may store this as part of `RuntimeManagement`, but Provider owns the
call-level state vocabulary and normalization.

## Routing

### `routing/routes/`

Resolves a named route such as `flagship_thinking` to ordered provider candidates.

Route behavior:

- validate candidates
- apply runtime overrides
- filter disabled providers
- sort by priority/policy

### `routing/fallback/`

Fallback handling.

Rules:

- fallback only when policy permits
- preserve error reason from failed candidate
- preserve usage/latency stats per attempt
- expose which provider/model finally answered

### `routing/policy/`

Routing policy:

- preferred provider
- allowed providers
- service tier
- timeout class
- retry budget
- fallback allowed
- auth required
- rate-limit behavior

## Provider Adapters

Adapters implement provider-specific request and response details.

Provider adapters must:

- accept normalized request structs
- return normalized response structs
- report provider request id when available
- report usage when available
- avoid writing logs directly

Initial adapters:

- `llm/openapi.rs`
- `llm/google.rs`
- `llm/bedrock.rs`
- `llm/providers/openai.rs`
- `llm/providers/codex.rs`
- `llm/providers/minimax.rs`
- `llm/providers/google.rs`
- `llm/providers/bedrock.rs`

## Request Pipeline

```text
Runtime
  -> provider route request
  -> config loader resolves route
  -> routing selects provider candidate
  -> auth resolves credential
  -> request builder normalizes payload
  -> provider adapter sends request
  -> streaming/response normalization
  -> usage/cost calculation
  -> logging and monitoring
  -> normalized ProviderResponse
```

Request builder owns:

- messages/input shape
- model
- temperature
- max tokens
- reasoning effort
- service tier
- stream options
- tool schema payload
- prompt cache key
- store flag
- provider-specific extra fields

## Response Pipeline

Response normalization owns:

- text extraction
- tool call extraction
- reasoning metadata
- finish reason
- provider request id
- raw usage
- normalized usage
- normalized error

ProviderResponse should include:

- normalized content
- text
- tool calls
- usage report
- provider metadata
- raw response reference if stored

### Public response normalization API (consumed by runtime)

Runtime never branches on provider wire format. It calls these provider-crate
functions with the already-normalized content payload from
`normalize_response_content`:

| Function | Purpose |
|---|---|
| `extract_response_text(&Value) -> Option<String>` | Pull plain text from OpenAI string / `text` field / Google `parts[].text` |
| `extract_tool_calls(&Value) -> Vec<ProviderToolCall>` | Pull tool calls from OpenAI `tool_calls` array / Google `parts[].functionCall + thoughtSignature` |
| `strip_thought_blocks(&str) -> String` | Remove `<thought>…</thought>` leakage |
| `prompt_cache_key_supported(provider, base_url) -> bool` | Whether to attach OpenAI prompt-cache key (`TURA_DISABLE_PROMPT_CACHE` honored) |
| `openai_compatible_usage_stream_supported(provider, base_url) -> bool` | Whether the route accepts `stream_options.include_usage` (OpenAI/minimax/qwen/openrouter family) |
| `provider_unsupported_content_type(error_text) -> Option<&'static str>` | Detect provider rejection of `input_image` / `input_audio` / `input_file` from error string |
| `replace_unsupported_content_type_in_messages(messages, content_type) -> usize` | Replace rejected media blocks with `input_text` placeholders; returns replacement count |

`ProviderToolCall { tool_name, arguments, provider_metadata }` is the
normalized tool-call shape; runtime maps it 1:1 into its internal
`ToolCallData` without inspecting `provider_metadata`.

## Streaming

Streaming support is optional per provider but must normalize to shared events.

Streaming event kinds:

- started
- text delta
- reasoning delta
- tool call delta
- usage update
- completed
- failed

Provider-specific SSE/chunk parsing lives under `streaming/parser`.

## Usage And Cost

The current provider already has a rich usage shape:

- input tokens
- output tokens
- total tokens
- cached input tokens
- cache write tokens
- reasoning tokens
- context window
- context used tokens
- context utilization ratio
- cost breakdown
- cache hit
- provider request id
- raw usage

Keep this richness, but move it under `usage/`.

Usage report:

```rust
struct UsageReport {
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    cached_input_tokens: u64,
    cache_write_tokens: u64,
    reasoning_tokens: u64,
    context_window: Option<u64>,
    context_used_tokens: Option<u64>,
    input_cost: f64,
    output_cost: f64,
    cache_read_cost: f64,
    cache_write_cost: f64,
    reasoning_cost: f64,
    total_cost: f64,
    currency: String,
    provider_request_id: Option<String>,
}
```

`usage/pricing/` owns model pricing tables and pricing source metadata.

`usage/reports/` owns aggregation:

- by call
- by runtime
- by turn
- by session
- by provider
- by model
- by day

## Logging

Call logging remains supported.

`logging/call_log/` owns:

- request metadata
- response metadata
- error metadata
- usage
- duration
- route/provider/model
- call id
- raw payload references

Rules:

- Redact auth and secrets.
- Bound raw payload size.
- Store large raw payloads by reference.
- Keep `LOG_PATH` override compatibility; the default provider log location is
  project-root `log/provider/YYYY-MM-DD`.
- Prefer structured JSON logs.

## Monitoring

### `monitoring/health/`

Provider health summary:

- configured
- authenticated
- ready
- degraded
- failed
- last success
- last error

### `monitoring/rate_limits/`

Rate limit snapshots:

- provider limit id
- window
- remaining
- reset time
- retry-after
- source

### `monitoring/latency/`

Latency metrics:

- request duration
- time to first token
- tokens per second
- retry latency
- queue time if known

### `monitoring/alerts/`

Provider alerts:

- auth expired
- quota low
- repeated timeouts
- provider degraded
- model unavailable

## Control

Control modules expose operational switches.

### `control/lifecycle/`

Provider lifecycle:

- initialize
- reload config
- refresh catalog
- shutdown

### `control/pause_resume/`

Pause or resume provider/model usage.

Use cases:

- temporarily disable a failing provider
- avoid a rate-limited provider
- allow user to force a provider off

### `control/kill_switch/`

Emergency disable by provider, model, or route.

### `control/quotas/`

Local quota and budget controls:

- per-session budget
- per-day budget
- max retries
- max timeout budget
- max context window use warning

## Storage

### `storage/config_store/`

Loads and saves provider config.

Sources:

- environment
- `TURA_ENV_PATH`
- `TURA_PROVIDER_CONFIG` (preferred), `TURALLM_CONFIG` (legacy)
- provider `config/provider_config.json`
- future global Tura config from `tura_path`

### `storage/secret_store/`

Stores auth secrets and tokens.

### `storage/usage_store/`

Stores usage records for replay and aggregation.

### `storage/log_store/`

Stores call logs and raw payload references.

## Runtime Integration

Runtime calls Provider for one model call:

```text
crates/runtime/provider_runtime
  -> provider/routing
  -> provider/auth
  -> provider/request
  -> provider/providers/*
  -> provider/response
  -> provider/usage
  -> provider/logging
```

Runtime owns turn/session behavior. Provider owns provider-call behavior.

**Provider-branch ownership rule (binding):** runtime must contain zero
provider-name `if`/`match` branches, zero per-provider URL or format checks,
and zero `<thought>` / `tool_calls` / `functionCall` / `thoughtSignature`
parsing. Every such concern lives in this crate and is exposed through the
response-normalization API above. Runtime only sees the canonical
Responses-API content shape (`input_image`, `input_audio`, `input_file`,
`tool_calls`) and the `ProviderToolCall` struct.

## Gateway Integration

Gateway may expose provider APIs:

- list providers
- list models
- read auth state
- start OAuth login
- complete OAuth login
- logout/revoke
- read usage reports
- read provider health
- pause/resume provider
- update provider settings

Gateway should not inspect raw provider secrets.

## Tests

Minimum tests when implementation begins:

- config path resolution including `TURA_ENV_PATH` and `TURA_PROVIDER_CONFIG` (preferred), `TURALLM_CONFIG` (legacy)
- route loading from legacy JSON
- API key resolution and masking
- OAuth state transitions
- token refresh success/failure
- provider route fallback policy
- OpenAI-compatible request normalization
- Google request normalization
- Bedrock request normalization
- usage extraction for each provider
- cost calculation by pricing table
- call log redaction
- rate-limit snapshot parsing
- pause/resume provider policy
- tura_path compatibility

## Design Summary

Provider should become a clean provider-control crate:

```text
config + auth + routing + adapters + response + usage + logging + monitoring
```

It should preserve the useful parts of current Tura provider:

- route config
- provider logs
- rich usage/cost shape
- OAuth refresh compatibility
- OpenAI/Google/Bedrock adapters

But it should remove the old concentration of responsibilities from
`tura_llm.rs` and provider-specific mega-files.
