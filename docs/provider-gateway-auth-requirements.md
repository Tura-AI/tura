# Provider And Gateway Auth Integration Requirements

This document is a handoff specification for completing Tura provider auth,
subscription login, token persistence, provider runtime robustness, and gateway
auth CLI/API integration.

The work must follow the existing repository architecture. Do not collapse
provider runtime behavior into gateway, and do not move UI/API login behavior
into provider.

## Architecture Boundary Rules

The following boundaries are mandatory.

### Provider Crate Owns

All model-provider runtime behavior belongs in `crates/provider`.

Provider-owned work includes:

- Provider config loading from `crates/provider/config/tura_llm_config.json`,
  `TURALLM_CONFIG`, `.env`, and `TURA_ENV_PATH`.
- Provider auth config schema and auth state vocabulary.
- API key resolution.
- OAuth token resolution and refresh.
- Local subscription token discovery, such as Codex or Claude Code local auth.
- Provider route lookup and fallback policy.
- Provider request building and provider-specific adapters.
- Streaming and non-streaming HTTP calls.
- Provider timeout policy and retry/backoff behavior.
- Provider response normalization.
- Tool-call normalization.
- Cache, token, reasoning, cost, and usage accounting.
- Provider logs and secret redaction.
- Provider health, status, rate-limit, and degradation state.

Gateway may call provider-owned functions, but gateway must not duplicate
provider runtime rules.

### Gateway Crate Owns

All frontend-facing and CLI-facing login/control behavior belongs in
`crates/gateway`.

Gateway-owned work includes:

- HTTP API routes for provider list, auth methods, auth status, login start,
  callback, token submission, validate, refresh, and logout.
- Browser opening and OAuth callback handling.
- User-facing CLI auth commands when those commands are implemented in the
  gateway binary or gateway API layer.
- Translating UI/CLI auth payloads into provider auth config writes.
- Persisting UI-facing provider status through gateway stores.
- Returning structured API responses for auth success/failure.
- Never exposing raw secrets in HTTP responses, logs, or UI replay state.

Gateway must not build provider call payloads, parse model streams, calculate
provider usage, or decide provider runtime refresh rules.

### Router Owns

Router owns startup/lifecycle of managed local services and CLI forwarding
metadata. If gateway needs to be started by router, fix router/gateway binary
lifecycle there. Do not implement provider auth in router.

### Runtime Owns

Runtime owns agent turns and calls provider for one model request. Runtime must
not own OAuth/login behavior.

## Current Implementation Summary

Read these files before coding:

- `ARCHITECTURE.md`
- `crates/provider/ARCHITECTURE.md`
- `crates/gateway/ARCHITECTURE.md`
- `crates/provider/src/tura_llm.rs`
- `crates/provider/src/tura_llm_conf.rs`
- `crates/provider/src/tura_conf.rs`
- `crates/provider/src/llm/_openai_provider.rs`
- `crates/provider/src/llm/_google_provider.rs`
- `crates/provider/src/llm/_bedrock_provider.rs`
- `crates/provider/src/llm/_llm_log.rs`
- `crates/provider/config/tura_llm_config.json`
- `crates/gateway/src/api/provider.rs`
- `crates/gateway/src/web/server.rs`
- `crates/router/src/main.rs`

Current important behavior:

- OpenAI/Codex subscription auth is the only near-complete route.
- `crates/provider/src/tura_llm.rs` contains OpenAI OAuth refresh and Codex
  local auth discovery through `~/.codex/auth.json`.
- `crates/provider/src/llm/_openai_provider.rs` contains Codex subscription
  `/backend-api/codex/responses` support, streaming parsing, command-run
  streaming events, cache usage extraction, and provider timeout behavior.
- Google has a native non-streaming `generateContent` implementation in
  `_google_provider.rs`.
- Bedrock has a small adapter.
- Other providers mostly use the OpenAI-compatible `/chat/completions` path.
- `crates/gateway/src/api/provider.rs` hard-codes provider list, auth methods,
  OAuth authorize/callback behavior, provider env key names, and provider auth
  config writes.
- Anthropic currently exposes `Claude Browser Token` through gateway, but this
  is a manual browser-token flow, not a full Claude Code or subscription OAuth
  integration.
- Gemini/Google subscription OAuth and Antigravity auth are not implemented.
- Gateway package currently does not expose a normal `gateway` binary even
  though router expects `target/debug/gateway.exe`.

## Goal

Implement a complete provider auth and provider runtime integration surface for:

- Codex / OpenAI subscription OAuth.
- OpenAI API key.
- Claude Code local subscription login.
- Claude browser/subscription token login.
- Anthropic API key.
- Gemini / Google subscription OAuth when available.
- Gemini / Google API key.
- Antigravity provider auth if a supported provider endpoint/token source exists.
- Antigravity browser/subscription token.
- Antigravity API key.
- Generic OpenAI-compatible API-key providers.
- Bedrock credentials.
- Other configured providers in `tura_llm_config.json`.

The completed system should let gateway and CLI list, login, validate, refresh,
logout, and inspect status for every provider auth method, while provider crate
handles the actual runtime auth resolution, refresh, request, stream, timeout,
usage, and logging behavior.

## Non-Goals

Do not rewrite the whole provider crate into the future architecture in one
large refactor.

Do not move gateway API handlers into provider.

Do not move provider streaming or request-building logic into gateway.

Do not add unrelated agent prompt changes.

Do not store secrets in `tura_llm_config.json`; config should store env key
names, login method, endpoint metadata, account id, status, and timestamps, not
raw tokens.

Do not print tokens in logs, test snapshots, HTTP responses, or CLI output.

## Required Provider Registry

Create one shared provider-auth registry source of truth, preferably in
`crates/provider`, that gateway can project into UI/CLI API responses.

The registry should describe each provider id, runtime id, display name,
supported auth methods, env keys, config keys, default base URL, endpoint type,
and capability flags.

Suggested provider ids:

- `openai`
- `openai-api`
- `anthropic`
- `anthropic-api`
- `claude-code`
- `google`
- `google-api`
- `gemini`
- `gemini-api`
- `Antigravity`
- `antigravity`
- `antigravity-api`
- `openrouter`
- `deepseek`
- `minimax`
- `moonshotai`
- `qwen`
- `xai`
- `opencode`
- `bedrock`

If a current route uses a provider id already present in
`tura_llm_config.json`, preserve compatibility. Do not rename existing ids
without a compatibility alias.

Each registry entry must include:

- `provider_id`
- `runtime_provider_id`
- `display_name`
- `base_url_config_key`
- `default_base_url`
- `supported_models` or catalog hook
- `auth_methods`
- `token_env`
- `login_env`
- optional `refresh_env`
- optional `expires_env`
- optional `account_env`
- optional `endpoint_env`
- optional `local_auth_discovery`
- optional `oauth_authorize_kind`
- optional `oauth_callback_kind`
- capability flags:
  - `supports_streaming`
  - `supports_tool_call_streaming`
  - `supports_cache_usage`
  - `supports_reasoning_usage`
  - `supports_subscription`
  - `supports_api_key`
  - `supports_oauth_refresh`
  - `supports_model_validation`

Gateway should use this registry for:

- provider display names
- provider auth method list
- provider env/config projection
- provider id to runtime id mapping
- browser login URLs
- token field names
- logout cleanup fields

Provider should use this registry for:

- auth resolution
- request adapter selection
- refresh support detection
- capability-aware streaming/cache/usage behavior

## Auth Method Model

Define a typed auth method model. Avoid stringly typed logic spread across
gateway and provider.

Required auth method kinds:

- `api_key`
- `oauth_pkce`
- `browser_token`
- `local_cli_token`
- `device_code`
- `aws_credentials`
- `none`

Required login values persisted in env/config:

- `api`
- `oauth`
- `browser`
- `local`
- `device`
- `aws`

Required auth states:

- `unknown`
- `not_configured`
- `api_key_configured`
- `oauth_starting`
- `oauth_waiting_for_browser`
- `oauth_waiting_for_callback`
- `browser_token_required`
- `local_token_discovered`
- `authenticated`
- `refreshing`
- `expired`
- `revoking`
- `revoked`
- `failed`

Required provider runtime states:

- `unknown`
- `disabled`
- `configured`
- `missing_auth`
- `ready`
- `degraded`
- `rate_limited`
- `paused`
- `failed`

Do not conflate auth state and runtime health. A provider can be authenticated
but degraded or rate-limited.

## Env And Config Requirements

Provider config remains in:

- `crates/provider/config/tura_llm_config.json`
- optional override `TURALLM_CONFIG`

Secrets are stored in:

- `.env` resolved through `TURA_ENV_PATH` or provider config path rules
- future secret store/keyring when implemented
- local provider auth files only when read-only discovery is supported

Config entries must not contain raw access tokens or API keys.

### Required Common Config Entry Shape

Every provider auth config entry should be able to use:

```json
{
  "type": "oauth",
  "login": "oauth",
  "status": "connected",
  "provider": "openai",
  "auth_url": "https://...",
  "endpoint": "https://...",
  "token_env": "OPENAI_API_KEY",
  "login_env": "OPENAI_LOGIN",
  "refresh_env": "OPENAI_REFRESH_TOKEN",
  "expires_env": "OPENAI_TOKEN_EXPIRES",
  "account_env": "OPENAI_ACCOUNT_ID",
  "account_id": "optional-account-id",
  "updated_at": "2026-05-23T00:00:00Z"
}
```

For API-key auth:

```json
{
  "type": "api_key",
  "login": "api",
  "status": "connected",
  "provider": "anthropic",
  "token_env": "ANTHROPIC_API_KEY",
  "login_env": "ANTHROPIC_LOGIN",
  "updated_at": "2026-05-23T00:00:00Z"
}
```

For browser-token auth:

```json
{
  "type": "browser_token",
  "login": "browser",
  "status": "connected",
  "provider": "anthropic",
  "auth_url": "https://claude.ai/login",
  "token_env": "ANTHROPIC_API_KEY",
  "login_env": "ANTHROPIC_LOGIN",
  "updated_at": "2026-05-23T00:00:00Z"
}
```

### Required Env Keys

Preserve existing env compatibility:

- `OPENAI_API_KEY`
- `OPENAI_LOGIN`
- `OPENAI_REFRESH_TOKEN`
- `OPENAI_TOKEN_EXPIRES`
- `OPENAI_ACCOUNT_ID`
- `OPENAI_CODEX_ENDPOINT`
- `ANTHROPIC_API_KEY`
- `ANTHROPIC_LOGIN`
- `GOOGLE_API_KEY`
- `GOOGLE_LOGIN`
- `GEMINI_API_KEY`
- `GEMINI_LOGIN`
- `ANTIGRAVITY_API_KEY`
- `ANTIGRAVITY_LOGIN`
- `OPENROUTER_API_KEY`
- `DEEPSEEK_API_KEY`
- `MINIMAX_API_KEY`
- `MOONSHOTAI_API_KEY`
- `QWEN_API_KEY`
- `XAI_API_KEY`
- `OPENCODE_API_KEY`
- Bedrock/AWS credential variables already used by the Bedrock adapter or AWS
  SDK path.

Add refresh/expires/account keys only for providers that can actually refresh:

- `GOOGLE_ACCESS_TOKEN`
- `GOOGLE_REFRESH_TOKEN`
- `GOOGLE_TOKEN_EXPIRES`
- `GOOGLE_ACCOUNT_ID`
- `CLAUDE_CODE_ACCESS_TOKEN` if local discovery requires a separate env bridge
- `CLAUDE_CODE_REFRESH_TOKEN` only if supported by the discovered source
- `CLAUDE_CODE_TOKEN_EXPIRES` only if known

Do not invent fake refresh tokens for browser-token providers.

## Provider Runtime Requirements

Implement or preserve these in `crates/provider`.

### Auth Resolution

Provider runtime must resolve credentials through a provider-owned resolver:

```text
ProviderConfig
  -> registry entry
  -> auth config entry
  -> env/secret/local discovery
  -> optional refresh
  -> AuthCredential
```

Required credential fields:

- `provider_id`
- `runtime_provider_id`
- `login`
- `token`
- optional `refresh_token`
- optional `expires_at_ms`
- optional `account_id`
- optional `endpoint`
- `source`: `env`, `config`, `local_cli`, `secret_store`, `oauth_callback`

Rules:

- Existing `OPENAI_LOGIN=oauth` behavior must keep working.
- Existing `~/.codex/auth.json` discovery must keep working.
- API-key providers must keep using `{PROVIDER}_API_KEY`.
- `provider_auth` config should influence env key selection and login method.
- 401 should trigger one provider-owned refresh/reload attempt only when the
  auth method supports refresh.
- Failed refresh must return a typed provider error.

### OpenAI And Codex

Keep current behavior:

- PKCE OAuth.
- Codex local auth discovery from `~/.codex/auth.json`.
- Codex subscription endpoint.
- `ChatGPT-Account-Id` header when account id exists.
- `originator` and user-agent headers.
- Refresh through OpenAI token endpoint.
- Cache/usage fallback estimation when stream ends without usage.
- Streaming command-run early events.

Move scattered OpenAI auth logic toward provider auth modules over time, but do
not break the current route while refactoring.

### Anthropic / Claude

Required provider-owned work:

- Add a native Anthropic adapter or verify and harden the current compatible
  path against Anthropic real API behavior.
- Native Anthropic request headers must be correct if native adapter is used:
  `x-api-key`, `anthropic-version`, and appropriate content type.
- Support API-key login with `ANTHROPIC_API_KEY` and `ANTHROPIC_LOGIN=api`.
- Support browser-token login with `ANTHROPIC_API_KEY` and
  `ANTHROPIC_LOGIN=browser` only if the runtime endpoint accepts that token.
- Support Claude Code local auth discovery if a local auth file or CLI token
  source is available.
- Model separate `anthropic-api`, `anthropic`, and `claude-code` auth profiles
  instead of treating every Anthropic path as the same token.
- Implement or explicitly mark unsupported:
  - streaming
  - tool-call streaming
  - cache read/write usage extraction
  - reasoning token extraction
  - refresh
  - account id extraction

Do not claim Claude is Codex-equivalent until streaming, cache usage, timeout,
and token lifecycle are all verified by tests.

### Google / Gemini

Required provider-owned work:

- Preserve API-key `generateContent` and embedding behavior.
- Add explicit auth resolution for Google/Gemini API key.
- Add OAuth credential support if a subscription/OAuth route is available:
  access token, refresh token, expires, account id.
- Add refresh for Google OAuth credentials.
- Add native streaming through `streamGenerateContent` or equivalent.
- Add function-call streaming normalization.
- Normalize Google usage fields into `UsageDetails`.
- Preserve `cachedContentTokenCount` mapping.
- Add support for OAuth bearer auth versus API-key query param depending on
  login method.

### Antigravity

No complete Antigravity provider route is currently visible in this repository.
Before implementing runtime behavior, document and verify:

- provider endpoint
- auth method
- token source
- model list
- streaming protocol
- usage response shape
- tool-call support

If no provider API exists, implement only gateway/config scaffolding behind a
disabled provider registry entry and return clear unsupported status.

### Generic OpenAI-Compatible Providers

Providers currently configured or expected:

- `openrouter`
- `deepseek`
- `minimax`
- `moonshotai`
- `qwen`
- `xai`
- `opencode`

Required provider-owned work:

- API-key auth through registry.
- Provider-specific base URL from config.
- Provider-specific headers if required.
- Preserve OpenAI-compatible `/chat/completions` behavior.
- Streaming parser compatibility tests for each provider class where possible.
- Tool-call parsing tests for OpenAI-compatible JSON tool calls.
- MiniMax XML streaming tool-call behavior must keep working.
- Cache usage extraction must preserve current broad field support:
  - `prompt_tokens_details.cached_tokens`
  - `input_tokens_details.cached_tokens`
  - `cache_read_input_tokens`
  - `cache_creation_input_tokens`
  - `cache_read_tokens`
  - `cache_write_tokens`
- Do not send OpenAI-only options to providers that reject them unless already
  guarded.

### Bedrock

Required provider-owned work:

- Treat Bedrock as non-API-key or AWS-credential auth, not generic bearer token.
- Add gateway-visible auth status projection without exposing AWS secrets.
- Add timeout/error/usage normalization tests.

### Streaming

Provider crate must own streaming event parsing.

Required shared event semantics:

- provider output started
- text delta
- reasoning delta when supported
- tool call argument delta
- complete tool call ready
- usage update
- completed
- failed

The existing `ProviderStreamEvent::ProviderOutputStarted` and
`ProviderStreamEvent::CommandRunCommandReady` must keep working.

Required robustness:

- Do not emit incomplete JSON tool calls.
- Emit each complete `command_run.commands[]` object as soon as it becomes
  safely parseable.
- Drain final usage after tool-call arguments complete.
- Tool-call stream deltas must not pollute final output text.
- Stream parser must tolerate CRLF/LF SSE line endings.
- Stream parser must handle split JSON chunks.
- Stream timeout must distinguish first output from idle output.

### Timeout And Retry

Provider crate owns timeout policy.

Current config:

- `provider_latency.active`
- `provider_latency.levels.low`
- `provider_latency.levels.medium`
- `provider_latency.levels.high`
- `provider_latency.levels.highest`
- `TURA_PROVIDER_LATENCY_LEVEL`
- `TURA_SESSION_REASONING_EFFORT` mapping to latency level

Required behavior:

- All provider adapters must use provider latency settings.
- Streaming calls must use first-output and idle-output timeouts.
- Non-streaming calls must use first-response timeout and total timeout.
- Add total request timeout coverage, not just first response timeout.
- Retry policy must be typed and bounded.
- 401 refresh retry can happen only for refresh-capable auth.
- 429 retry/backoff should preserve retry-after if present.
- Provider logs must include timeout phase without logging secrets.

### Usage, Cache, Cost, And Metrics

Provider crate owns usage normalization.

Every provider response should attempt to fill:

- `input_tokens`
- `output_tokens`
- `total_tokens`
- `cached_input_tokens`
- `cache_write_tokens`
- `reasoning_tokens`
- `audio_input_tokens`
- `audio_output_tokens`
- `context_window`
- `context_used_tokens`
- `context_utilization_ratio`
- `cache_hit`
- `cache_triggered_at_input_tokens`
- `finish_reason`
- `provider_request_id`
- `raw_usage`

If a provider does not supply usage, return `None` fields or an explicit
estimated raw-usage marker only where estimation is intentionally supported.

Gateway must only display/project usage; it must not calculate provider usage.

### Logging And Redaction

Provider crate owns provider call logs.

Required:

- Redact authorization headers.
- Redact API keys.
- Redact access tokens.
- Redact refresh tokens.
- Redact browser tokens.
- Redact local auth file token values.
- Keep auth mode, provider id, model id, route id, latency, status, and error
  category.
- Bound raw request/response payload size.
- Preserve existing `crates/provider/log/YYYY-MM-DD/*.json` compatibility.

## Gateway API Requirements

Implement or update these in `crates/gateway`.

### Provider Listing

Gateway should project provider registry and provider config into the UI shape.

Required endpoints:

- `GET /provider`
- `GET /provider/auth`
- `POST /provider/{providerID}/validate`

Provider list must include:

- provider id
- display name
- configured models
- default model
- connected/authenticated state
- env key names only, not values
- auth methods
- capability flags where useful for UI

Do not hard-code model catalogs in gateway if provider crate exposes them.
Gateway can keep temporary compatibility maps only while provider registry is
not fully exposed.

### Auth Methods

`GET /provider/auth` must return every supported provider auth method.

Example shape:

```json
{
  "openai": [
    {"type": "oauth_pkce", "label": "ChatGPT/Codex subscription"},
    {"type": "api_key", "label": "OpenAI API key"}
  ],
  "anthropic": [
    {"type": "local_cli_token", "label": "Claude Code local login"},
    {"type": "browser_token", "label": "Claude browser token"},
    {"type": "api_key", "label": "Anthropic API key"}
  ]
}
```

Gateway labels are user-facing. Provider registry owns the canonical method id.

### OAuth Authorize

Required endpoint:

- `POST /provider/{providerID}/oauth/authorize`

Request:

```json
{
  "method": 0,
  "inputs": {}
}
```

Response:

```json
{
  "url": "https://...",
  "method": "auto",
  "instructions": "Complete authorization in your browser.",
  "state": "opaque-state-id",
  "expires_at": 1770000000000
}
```

Supported response methods:

- `auto`
- `code`
- `device`
- `token`
- `unsupported`

Gateway owns browser open behavior. Provider owns the provider-specific OAuth
metadata and token exchange helper.

### OAuth Callback

Required endpoint:

- `POST /provider/{providerID}/oauth/callback`
- `GET /auth/callback`

Callback behavior:

- OpenAI PKCE callback must keep working.
- Provider-specific HTML should not always say OpenAI.
- Expired state must return structured failure.
- Manual token callback must persist token through the shared auth persistence
  path.
- Callback should return a structured result, not only `true` or `false`, for
  new API surfaces.

Suggested structured response:

```json
{
  "ok": true,
  "provider_id": "openai",
  "login": "oauth",
  "status": "connected",
  "account_id": "optional",
  "message": "Connected"
}
```

### Token And API-Key Login

Required endpoint:

- `PUT /auth/{providerID}`

Required auth payload fields:

- `type`
- `key` or `access`
- optional `refresh`
- optional `expires`
- optional `account_id`
- optional `metadata.login`
- optional `metadata.url`

Gateway must:

- validate payload shape
- reject empty tokens
- persist through provider-compatible config/env path
- return structured errors
- never echo token values

### Status, Refresh, Validate, Logout

Add endpoints:

- `GET /provider/{providerID}/auth/status`
- `POST /provider/{providerID}/auth/refresh`
- `POST /provider/{providerID}/auth/validate`
- `POST /provider/{providerID}/auth/logout`

Status must include:

- provider id
- display name
- login method
- configured/not configured
- authenticated/not authenticated
- expired/valid when known
- account id when safe
- env key names
- last updated time
- last error category

Logout must:

- remove provider auth from gateway store
- remove or blank relevant env values according to chosen env file policy
- update `provider_auth` config status
- avoid deleting unrelated provider credentials

### CLI Requirements

Auth CLI should be implemented through gateway or a gateway-backed command
surface, not provider runtime.

Required commands:

- `tura auth list`
- `tura auth methods`
- `tura auth status`
- `tura auth status <provider>`
- `tura auth login <provider>`
- `tura auth login <provider> --method oauth`
- `tura auth login <provider> --method api`
- `tura auth login <provider> --method browser`
- `tura auth token <provider>`
- `tura auth validate <provider>`
- `tura auth refresh <provider>`
- `tura auth logout <provider>`

Required flags:

- `--json`
- `--env-path <path>`
- `--config <path>`
- `--no-browser`
- `--print-url`
- `--token <value>` for non-interactive tests only
- `--token-stdin`
- `--method <method>`

Rules:

- Interactive token input should hide secret text where possible.
- `--json` output must be machine-parseable.
- `--print-url --no-browser` must support remote/headless login.
- CLI should start or connect to gateway as needed.
- CLI should not require users to discover `target/debug/deps/gateway.exe`.

## Gateway Binary And Router Lifecycle Requirements

Current issue: router expects a `gateway` executable path, but the gateway
package currently exposes bin `tura`, and a usable gateway executable may only
exist under `target/debug/deps/gateway.exe`.

Required:

- Add a normal `gateway` binary target or update router lifecycle to use the
  real gateway server binary.
- Preserve the existing CLI `tura` binary if it is intentionally separate.
- Router `ensure_gateway` must start gateway reliably.
- Gateway health check must be `GET /global/health`.
- Gateway root `/` may remain 404 unless UI needs it, but auth CLI should not
  instruct users to browse `/`.
- OAuth callback server port 1455 must be checked and reported.
- Startup failures must return actionable errors.

## Provider-Specific Work Matrix

### OpenAI / Codex

Provider crate:

- Keep OpenAI API-key route.
- Keep OpenAI OAuth refresh.
- Keep Codex auth discovery from `~/.codex/auth.json`.
- Keep Codex subscription endpoint.
- Keep Codex streaming parser.
- Keep command-run early streaming events.
- Keep cache usage extraction.
- Add structured auth resolver around existing behavior.
- Add status/validate hooks callable from gateway.

Gateway:

- Keep PKCE authorize/callback.
- Keep browser callback on port 1455.
- Add structured status/refresh/logout.
- Add CLI login/status/logout.
- Stop hard-coding OpenAI-only callback copy in shared callback HTML.

Tests:

- PKCE authorize URL includes state/challenge.
- Callback exchanges code and writes env/config.
- Refresh happens when expired.
- Codex auth.json discovery works.
- Streaming tool call emits only complete command objects.
- Cache usage is preserved.

### OpenAI API Key

Provider crate:

- Resolve `OPENAI_API_KEY`.
- Set `OPENAI_LOGIN=api` when configured through gateway.
- Ensure OAuth refresh is not attempted when login is `api`.

Gateway:

- Expose API-key method separately from subscription OAuth.
- Validate key presence without echoing it.

Tests:

- API-key login writes env/config.
- API-key status says configured.
- Logout removes or disables API-key auth without touching OAuth refresh fields
  unless requested.

### Claude Code

Provider crate:

- Add a local Claude Code auth discovery module if a stable local token source
  exists.
- Detect Windows and Unix locations separately.
- Return a typed unsupported status if no known local auth source exists.
- Do not parse or log raw local auth token values.
- Add an auth profile distinct from Anthropic API key.

Gateway:

- Expose `Claude Code local login` auth method.
- Login should either detect existing local auth or instruct the user how to run
  Claude Code login.
- Status should show local token discovered/missing.

Tests:

- Mock local auth file discovery.
- Missing local auth returns action-required status.
- Discovered token is redacted in logs/status.

### Claude Browser / Subscription Token

Provider crate:

- Resolve `ANTHROPIC_API_KEY` with `ANTHROPIC_LOGIN=browser`.
- Verify which endpoint accepts this token before marking provider ready.
- If browser token is not usable by Anthropic API, keep it as a separate
  disabled/unsupported runtime auth mode.

Gateway:

- Keep manual browser-token paste flow only as `browser_token`, not `oauth_pkce`.
- Instructions must say token paste is required.
- Callback should return structured result.

Tests:

- Authorize returns Claude login URL and token-paste method.
- Callback writes `ANTHROPIC_API_KEY` and `ANTHROPIC_LOGIN=browser`.
- Status differentiates browser token from API key.

### Anthropic API Key

Provider crate:

- Add native Anthropic adapter or harden the compatible adapter.
- Resolve `ANTHROPIC_API_KEY` with `ANTHROPIC_LOGIN=api`.
- Use correct Anthropic headers if native.
- Normalize usage/cache/tool calls.

Gateway:

- Expose `Anthropic API key`.
- Write config entry with `type=api_key`, `login=api`.

Tests:

- API-key login writes env/config.
- Request headers are correct.
- Usage fields normalize.
- 401 does not fake-refresh unless a refresh method exists.

### Gemini / Google API Key

Provider crate:

- Keep `GOOGLE_API_KEY` support.
- Optionally support `GEMINI_API_KEY` as alias if desired.
- Preserve generateContent and embedding behavior.
- Add streaming support.
- Add usage/cache normalization tests.

Gateway:

- Expose Google/Gemini API-key auth method.
- Write env/config consistently.

Tests:

- API-key login writes env/config.
- Non-streaming generateContent request shape.
- Streaming request shape after implemented.
- `cachedContentTokenCount` maps to cached input tokens.

### Gemini / Google OAuth

Provider crate:

- Add OAuth credential support only if the target Google/Gemini endpoint
  supports bearer OAuth for the desired subscription/API path.
- Implement refresh using Google token endpoint.
- Resolve access token versus API key based on login method.

Gateway:

- Add OAuth authorize/callback or device-code flow.
- Store access/refresh/expires/account env metadata.

Tests:

- OAuth callback writes token env names and config.
- Expired token refreshes.
- API-key mode remains unaffected.

### Antigravity

Provider crate:

- First document actual endpoint and auth source.
- If supported, add adapter and auth resolver.
- If not supported, registry entry should report unsupported.

Gateway:

- Expose only verified auth methods.
- Do not create fake successful login for unknown provider behavior.

Tests:

- Unsupported status is clear and structured.
- If implemented, auth and smoke validation are covered.

### Antigravity

Provider crate:

- Keep separate `antigravity` and `antigravity-api` auth profiles.
- Verify browser token runtime endpoint.
- API-key path should not use browser login.

Gateway:

- Expose browser-token and API-key methods separately.
- Persist `ANTIGRAVITY_LOGIN=browser` or `api`.

Tests:

- Browser-token callback writes correct login.
- API-key login writes correct login.
- Runtime picks correct token mode.

### Generic OpenAI-Compatible Providers

Provider crate:

- Keep current OpenAI-compatible chat/completions behavior.
- Add registry-driven auth env keys.
- Add provider-specific header hooks.
- Add provider-specific option filtering.
- Add streaming parser tests by response class.

Gateway:

- Expose API-key methods for every configured provider.
- Do not hard-code only openai/anthropic/antigravity.

Tests:

- Each configured provider can be listed.
- API-key login writes provider-specific env/config.
- Runtime can resolve the key by provider id.

### Bedrock

Provider crate:

- Model AWS credential resolution as auth.
- Normalize errors and usage.

Gateway:

- Show Bedrock auth status without raw AWS secrets.

Tests:

- Missing AWS credentials reports missing auth.
- Present mocked credentials reports configured.

## Implementation Plan

### Phase 1: Registry And Types

Files likely touched:

- `crates/provider/src/tura_llm.rs`
- new `crates/provider/src/auth/...`
- new `crates/provider/src/registry/...`
- `crates/provider/src/lib.rs`
- `crates/gateway/src/api/provider.rs`

Tasks:

1. Add provider auth registry types in provider crate.
2. Add auth method enum and login enum.
3. Add provider capability flags.
4. Add compatibility mappings for existing provider ids.
5. Expose read-only registry projection for gateway.
6. Keep existing config load behavior working.

Validation:

- `cargo test -p tura-llm-rust registry`
- `cargo check -p tura-llm-rust`
- `cargo check -p gateway`

### Phase 2: Provider Auth Resolver

Files likely touched:

- `crates/provider/src/tura_conf.rs`
- `crates/provider/src/tura_llm.rs`
- new `crates/provider/src/auth/api_key`
- new `crates/provider/src/auth/oauth`
- new `crates/provider/src/auth/token_store`
- new `crates/provider/src/auth/login_state`

Tasks:

1. Extract `get_api_key` into provider-owned auth resolver.
2. Wrap existing OpenAI refresh logic in auth resolver.
3. Keep `~/.codex/auth.json` discovery.
4. Add typed auth credential result.
5. Add typed missing/expired/refresh-failed errors.
6. Add config/env key resolution from registry.

Validation:

- OpenAI OAuth refresh tests still pass.
- API-key providers still resolve keys.
- Missing key errors are provider-specific.

### Phase 3: Gateway Auth API Projection

Files likely touched:

- `crates/gateway/src/api/provider.rs`
- `crates/gateway/src/web/server.rs`
- `crates/gateway/src/api/types.rs`

Tasks:

1. Replace gateway hard-coded auth method list with provider registry
   projection.
2. Keep compatibility for current response shapes.
3. Add structured auth status response.
4. Add structured callback response for new APIs.
5. Add validate/refresh/logout endpoints.
6. Move provider env-key mapping to registry projection instead of local
   hard-coded gateway helpers where possible.

Validation:

- Existing UI provider list still works.
- Existing OpenAI OAuth still works.
- Anthropic browser-token authorize still works.

### Phase 4: Auth Persistence

Files likely touched:

- `crates/gateway/src/api/provider.rs`
- provider config store module if introduced
- `crates/provider/config/tura_llm_config.json`

Tasks:

1. Centralize env upsert behavior.
2. Avoid duplicate or blank env entries.
3. Persist config metadata only, never raw token values.
4. Support API-key, OAuth, browser-token, local-token, and AWS statuses.
5. Add logout cleanup.

Validation:

- `.env` upsert tests.
- `tura_llm_config.json` update tests.
- Secret redaction tests.

### Phase 5: CLI Auth

Files likely touched:

- `crates/gateway/src/bin/tura.rs`
- `crates/gateway/Cargo.toml`
- possibly router forwarding metadata

Tasks:

1. Add `tura auth ...` command parsing.
2. Start/connect to gateway as needed.
3. Support browser and no-browser flows.
4. Support token stdin.
5. Support JSON output.
6. Run optional smoke validation after successful login.

Validation:

- `tura auth methods --json`
- `tura auth status --json`
- `tura auth login openai --print-url --no-browser`
- `tura auth token anthropic --token-stdin --json`

### Phase 6: Gateway Binary Lifecycle

Files likely touched:

- `crates/gateway/Cargo.toml`
- `crates/gateway/src/bin/...`
- `crates/router/src/main.rs`

Tasks:

1. Expose a real gateway server binary or update router path.
2. Ensure router can start gateway.
3. Ensure gateway health is reachable.
4. Ensure OAuth callback server starts or reports conflict.

Validation:

- `cargo build -p gateway`
- Router `ensure_gateway` test.
- Manual `GET /global/health`.

### Phase 7: Non-OpenAI Runtime Robustness

Files likely touched:

- `crates/provider/src/llm/_google_provider.rs`
- `crates/provider/src/llm/_openai_provider.rs`
- new provider adapters as needed
- provider streaming modules if introduced

Tasks:

1. Add Google/Gemini streaming.
2. Add Anthropic native adapter or compatibility hardening.
3. Add provider-specific option filtering.
4. Add provider-specific header hooks.
5. Add total timeout coverage.
6. Add 401 refresh retry for refresh-capable providers.
7. Add 429/backoff classification.
8. Add usage/cache normalization tests.

Validation:

- Provider unit tests for streaming parsers.
- Provider unit tests for usage extraction.
- Mock HTTP tests for timeout/retry behavior.

## Acceptance Criteria

The work is done only when all of the following are true:

1. Gateway can list every configured provider and its auth methods.
2. Gateway can start login/token flows for Codex/OpenAI, Claude/Anthropic,
   Gemini/Google, Antigravity, generic OpenAI-compatible providers, and Bedrock
   where applicable.
3. Gateway auth APIs return structured errors, not only booleans.
4. CLI can perform auth status/login/token/validate/refresh/logout.
5. Provider crate resolves auth by provider id and login method.
6. OpenAI/Codex current behavior is not regressed.
7. Non-OpenAI providers do not falsely claim Codex-equivalent streaming/cache
   support unless tests prove it.
8. Provider runtime owns streaming, timeout, retry, cache, usage, and logging.
9. Gateway owns browser/login/API/CLI surfaces only.
10. No raw secrets appear in provider logs, gateway logs, CLI output, HTTP
    responses, or test snapshots.
11. Router can start gateway without relying on `target/debug/deps/gateway.exe`.
12. Focused tests pass for affected crates.

## Required Test Matrix

### Provider Crate Tests

Add tests under `crates/provider/tests` or provider module unit tests.

Required:

- registry lists all configured providers.
- env/config path resolution respects `TURA_ENV_PATH`.
- `TURALLM_CONFIG` override works.
- API key resolver handles existing provider ids.
- OpenAI OAuth refresh success.
- OpenAI OAuth refresh failure.
- Codex auth.json discovery.
- Anthropic API key resolver.
- Anthropic browser-token resolver.
- Claude Code local auth discovery, mocked.
- Google API key resolver.
- Google OAuth refresh, mocked if implemented.
- Generic OpenAI-compatible API key resolver.
- Bedrock missing/present credential status.
- OpenAI-compatible streaming parses usage.
- Codex streaming emits command-run only for complete JSON.
- Google streaming parser, when implemented.
- Anthropic streaming parser, when implemented.
- usage/cache extraction for OpenAI, Codex, Anthropic, Google, and generic
  OpenAI-compatible shapes.
- total timeout fires.
- first output timeout fires.
- idle output timeout fires.
- 401 refresh retry happens exactly once for refresh-capable auth.
- 401 does not refresh for API-key-only providers.
- 429 returns typed rate-limit error.
- logs redact tokens.

### Gateway Crate Tests

Add tests under `crates/gateway` unit tests or handler tests.

Required:

- `/provider/auth` includes all registry auth methods.
- provider list includes registry providers and configured route providers.
- OpenAI authorize creates PKCE state.
- OpenAI callback writes env/config metadata.
- Anthropic browser-token authorize returns token-paste method.
- Anthropic callback writes env/config metadata.
- API-key `PUT /auth/{provider}` writes env/config metadata.
- status endpoint masks secrets.
- refresh endpoint delegates provider refresh.
- logout endpoint updates store/config and does not remove unrelated secrets.
- callback HTML provider name is not hard-coded to OpenAI.
- failed persistence returns structured error.

### CLI Tests

Required:

- `tura auth methods --json`
- `tura auth status --json`
- `tura auth status openai --json`
- `tura auth login openai --no-browser --print-url --json`
- `tura auth token anthropic --token-stdin --json`
- `tura auth validate anthropic --json`
- `tura auth logout anthropic --json`
- invalid provider returns non-zero with structured error.
- token is not echoed.

### Router/Gateway Lifecycle Tests

Required:

- `cargo build -p gateway` produces the gateway server binary expected by
  router, or router uses the correct binary.
- router starts gateway and `GET /global/health` succeeds.
- gateway start failure reports path and cause.
- OAuth callback port conflict is reported.

## Build And Verification Commands

Use package names from `ARCHITECTURE.md`.

Provider:

```powershell
cargo fmt -p tura-llm-rust
cargo check -p tura-llm-rust
cargo test -p tura-llm-rust
```

Gateway:

```powershell
cargo fmt -p gateway
cargo check -p gateway
cargo test -p gateway
```

Router lifecycle if touched:

```powershell
cargo fmt -p tura_router
cargo check -p tura_router
cargo test -p tura_router
```

Runtime smoke if provider stream events are touched:

```powershell
cargo check -p code-tools-suite
cargo test -p code-tools-suite coding_agent_live_test
```

Full focused build:

```powershell
cargo build -p gateway
cargo build -p tura_router
cargo build -p tura-llm-rust
```

## Implementation Warnings

- Do not keep adding provider-specific hard-coded branches in gateway when the
  provider registry can express the same data.
- Do not claim browser-token login is OAuth PKCE.
- Do not fake refresh support.
- Do not store raw tokens in `tura_llm_config.json`.
- Do not break existing `OPENAI_LOGIN=oauth`.
- Do not break existing route names such as `tura_coder`.
- Do not break existing provider ids in `tura_llm_config.json`.
- Do not remove provider logs; improve redaction instead.
- Do not let gateway calculate usage.
- Do not let provider open browsers.
- Do not let runtime own login.
- Do not make Antigravity appear connected unless an actual provider endpoint and
  token path are verified.

## Suggested First Pull Request Scope

Start with a narrow PR:

1. Add provider auth registry in `crates/provider`.
2. Project registry into gateway `/provider/auth`.
3. Preserve current OpenAI and Anthropic behavior.
4. Add structured auth status endpoint.
5. Add tests proving no behavior regression.

This creates the foundation without touching streaming adapters yet.

Suggested second PR:

1. Extract provider auth resolver from `ProviderConfig::get_api_key`.
2. Move OpenAI refresh and Codex auth discovery behind the resolver.
3. Add API-key resolver tests for all configured providers.

Suggested third PR:

1. Add CLI auth commands.
2. Fix gateway binary/router lifecycle.

Suggested fourth PR:

1. Add Claude Code, Gemini OAuth, and non-OpenAI streaming/cache robustness one
   provider at a time.

