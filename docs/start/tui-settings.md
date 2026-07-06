# TUI Settings

## Configuration file locations

| Setting group | Configuration location | Notes |
| --- | --- | --- |
| Workspace runtime settings | `<workspace>/.tura/config.conf` | Written through the gateway `/session/config` endpoint for the active workspace directory. |
| Provider credentials | `.env` resolved from `TURA_ENV_PATH`, or the runtime project root `.env` | API keys and OAuth access tokens are written as provider environment variables. The exact variable name comes from the provider auth method, for example `OPENAI_API_KEY` or another provider-specific `token_env`. |
| Provider model-tier catalog | `TURA_PROVIDER_CONFIG`, or `<runtime-root>/config/provider_config.json`, or `<source-root>/crates/provider/config/provider_config.json` | The TUI reads this catalog to resolve tier names and provider/model options. The interactive TUI settings page does not edit tier routes directly. |

## Visible TUI settings

| Setting | Stored key | Values | Effect |
| --- | --- | --- | --- |
| Model | `model`, with derived `active_provider` and `active_model` when the value is `provider/model` | Any listed provider/model pair, or a model tier name when configured elsewhere | Selects the model used for new prompt execution in the workspace. A concrete `provider/model` also updates the active provider and active model fields. |
| Provider | `active_provider` | Any listed LLM provider | Opens that provider's auth/settings detail in the TUI. The selected provider is used with `active_model` when a concrete active model pair is configured. |
| Agent | `active_agent` | Any discovered agent id | Selects the default agent behavior for new prompt execution in the workspace. |
| Persona | `active_persona` | Any discovered persona id | Selects the assistant persona used for prompts and UI presentation defaults. Defaults to `tura`. |
| Language | `language` | `en`, `zh-CN` | Changes the TUI language and persists the workspace language preference. |
| Reasoning | `model_variant` | `low`, `medium`, `high`, `xhigh` | Sets the reasoning-effort variant sent with model execution. Defaults to `high`. |
| Priority | `model_acceleration_enabled` | `true`, `false` | Enables or disables priority/accelerated model routing where the selected provider supports it. Defaults to `false`. |

## Provider auth detail

| Action | Stored location | Effect |
| --- | --- | --- |
| API key | `.env` resolved from `TURA_ENV_PATH`, or runtime project root `.env` | Validates the entered key, stores it under the provider auth variable, and marks the provider configured. |
| OAuth login | `.env` plus provider auth metadata in the provider config flow | Starts the provider OAuth flow. Auto-callback flows poll until authentication is complete; manual flows wait for a callback code. |
| Logout | Provider auth storage for the selected provider | Removes stored provider auth for that provider and refreshes the displayed auth status. |

## Command-only TUI settings

These settings are supported by the TUI settings logic and config command path, but are not part of the main visible settings list.

| Setting | Stored key | Values | Effect |
| --- | --- | --- | --- |
| Session type | `session_type` | `coding`, `business`, `research`, `planning` | Sets the workspace session mode used by runtime prompts. Defaults to `coding`. |
| Validator | `validator_enabled` | `true`, `false` | Enables or disables validator behavior when the runtime uses validation. |
| Command stall guard | `command_run_stall_guard_profile` | `balanced_20s`, `fast_10s`, `patient_30s`, `long_io_60s`, `off` | Controls how aggressively `command_run` detects stalled output. Profile values map to check interval and identical-output checks. |
| Context message limit | `context_message_limit` | Positive number | Limits how many prior context messages are used by the workspace runtime. |
| Kill processes on start | `kill_processes_on_start` | `true`, `false` | Requests cleanup of existing runtime processes when a session starts. |
| Stall guard check interval | `command_run_stall_guard_check_secs` | Positive number | Overrides the stall guard polling interval in seconds. |
| Stall guard identical checks | `command_run_stall_guard_identical_checks` | Positive number | Overrides how many identical checks are required before a command is treated as stalled. |

## Workspace config file keys

`<workspace>/.tura/config.conf` is a simple `key=value` file. The TUI runtime config parser reads and writes these keys:

| Key | Purpose |
| --- | --- |
| `language` | Workspace UI/runtime language. |
| `model` | Selected model or model tier. |
| `active_provider` | Provider part of the active concrete model pair. |
| `active_model` | Model part of the active concrete model pair. |
| `active_agent` | Default agent id. |
| `active_persona` | Default persona id. |
| `session_type` | Runtime session mode. |
| `model_variant` | Reasoning-effort variant. |
| `model_acceleration_enabled` | Priority routing flag. |
| `context_message_limit` | Optional context message cap. |
| `kill_processes_on_start` | Optional process cleanup flag. |
| `validator_enabled` | Optional validator flag. |
| `force_planning` | Optional planning override used by runtime flows. |
| `show_react_kaomoji` | Optional reaction-display flag. Defaults to `true`. |
| `command_run_stall_guard_profile` | Stall guard profile name. |
| `command_run_stall_guard_check_secs` | Stall guard polling interval override. |
| `command_run_stall_guard_identical_checks` | Stall guard identical-output threshold override. |
| `agent_avatar` | JSON-encoded avatar display settings, shared with GUI personalization. |
