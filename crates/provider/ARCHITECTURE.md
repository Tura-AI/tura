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
- `tura_llm_config.json`
- `.env` / `TURA_ENV_PATH`
- `TURALLM_CONFIG`
- provider call logs under `log/`
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
  ARCHITECTURE.md

  config/
  log/
  tests/

  src/
    auth/
      oauth/
      api_key/
      token_store/
      login_state/

    config/
      loader/
      settings/
      profiles/
      path_compat/

    models/
      catalog/
      presets/
      capabilities/

    routing/
      routes/
      fallback/
      policy/

    providers/
      openai/
      openai_compatible/
      google/
      bedrock/

    request/
      builder/
      validation/
      normalization/

    response/
      normalization/
      tool_calls/
      errors/

    streaming/
      events/
      parser/
      receiver/

    usage/
      tokens/
      cost/
      pricing/
      reports/

    logging/
      call_log/
      redaction/
      retention/

    monitoring/
      health/
      rate_limits/
      latency/
      alerts/

    control/
      lifecycle/
      pause_resume/
      kill_switch/
      quotas/

    state/
      provider_state/
      auth_state/
      call_state/

    storage/
      config_store/
      secret_store/
      usage_store/
      log_store/

    utils/
```

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

llm/_openai_provider.rs
  -> providers/openai
  -> providers/openai_compatible
  -> streaming/*
  -> response/normalization

llm/_google_provider.rs
  -> providers/google

llm/_bedrock_provider.rs
  -> providers/bedrock

llm/_llm_log.rs
  -> logging/call_log
  -> logging/redaction
  -> logging/retention
```

## Compatibility With `tura_path`

Provider must not invent independent path rules. It should use the path contract
from `tura_path` for project-root-aware paths.

Supported compatibility inputs:

- `TURA_ENV_PATH`
- `TURALLM_CONFIG`
- project config from `tura_path`
- provider `config/tura_llm_config.json`
- provider `log/YYYY-MM-DD/...json`

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

[[provider.routes.tura_coder.providers]]
provider = "openai"
base_url = "https://api.openai.com/v1"
model = "gpt-5.5"
temperature = 0.2
priority = 100

[[provider.routes.tura_coder.providers]]
provider = "google"
base_url = "https://generativelanguage.googleapis.com"
model = "gemini-..."
temperature = 0.2
priority = 50
```

Existing JSON route config remains supported:

```text
config/tura_llm_config.json
```

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

Resolves a named route such as `tura_coder` to ordered provider candidates.

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

- `providers/openai/`
- `providers/openai_compatible/`
- `providers/google/`
- `providers/bedrock/`

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
- Keep legacy `log/YYYY-MM-DD` compatibility.
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
- `TURALLM_CONFIG`
- provider `config/tura_llm_config.json`
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

- config path resolution including `TURA_ENV_PATH` and `TURALLM_CONFIG`
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
