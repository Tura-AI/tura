# GUI Settings

The GUI presents several kinds of settings, but it does not own all of them.
This page maps each control to the place that actually persists it.

## Configuration file locations

| Setting group | Configuration location | Notes |
| --- | --- | --- |
| Workspace runtime settings | `<workspace>/.tura/config.conf` | Written through the gateway `/session/config` endpoint for the selected workspace directory. |
| GUI appearance settings | Gateway `/config` state | Stores theme, corner radius, font, font size, and skill-folder settings in the active gateway config state. |
| Model-tier routes | `TURA_PROVIDER_CONFIG`, or `<runtime-root>/config/provider_config.json`, or `<source-root>/crates/provider/config/provider_config.json` | Written through `/model_config`. The file contains the provider catalog and route table. |
| Provider credentials | `.env` resolved from `TURA_ENV_PATH`, or the runtime project root `.env` | API keys and OAuth tokens are stored as provider environment variables. |
| Custom agents | `<project-root>/agents/src/<agent-id>/agent_config.json` and optional `<project-root>/agents/src/<agent-id>/prompt.md` | Written through `/agent` endpoints. Static/default agents cannot be deleted. |

## Application settings

| Setting | Stored key | Values | Effect |
| --- | --- | --- | --- |
| Language | `language` in `<workspace>/.tura/config.conf` | GUI language options from the app language catalog, including `en` and `zh-CN` | Changes the GUI language immediately and persists the workspace language for later sessions. |

## Appearance settings

| Setting | Stored key | Values | Effect |
| --- | --- | --- | --- |
| Theme color | `theme` in gateway `/config` | `light`, `dark`, `caral`, `uruk`, `liangzhu` | Changes the GUI color theme. |
| Corner radius | `corner_radius` in gateway `/config` | `0px`, `2px`, `8px`, `9.6px` | Changes the radius scale used by GUI surfaces and controls. |
| Main font | `main_font` in gateway `/config` | System, Arial, Noto Sans, humanist, or serif font stack | Sets the primary UI font family. |
| Code font | `code_font` in gateway `/config` | System Mono, Cascadia Code, JetBrains Mono, Fira Code, Consolas | Sets the monospace font family used for code and terminal-like content. |
| Main font size | `main_font_size` in gateway `/config` | Integer size from the GUI size picker | Sets the base UI font size. |
| Code font size | `code_font_size` in gateway `/config` | Integer size from the GUI size picker | Sets the code/monospace font size. |

## Runtime model settings

| Setting | Stored key | Values | Effect |
| --- | --- | --- | --- |
| Selected model | `model`, with derived `active_provider` and `active_model` in `<workspace>/.tura/config.conf` | Any selected `provider/model` pair | Selects the model used for workspace prompt execution. |
| Active agent | `active_agent` in `<workspace>/.tura/config.conf` | Any discovered agent id | Selects the default agent used for workspace prompt execution. |
| Reasoning effort | `model_variant` in `<workspace>/.tura/config.conf` | `low`, `medium`, `high`, `xhigh`, `max` | Sets the model reasoning-effort variant for workspace prompts. `max` is sent only to GPT-5.6 models and maps to `xhigh` for older models. |
| Priority routing | `model_acceleration_enabled` in `<workspace>/.tura/config.conf` | `true`, `false` | Enables priority/accelerated routing where the selected provider supports it. |

## Model tier settings

| Setting | Stored location | Values | Effect |
| --- | --- | --- | --- |
| Thinking tier | Provider config `routes.thinking.providers[0]` | Any listed model option for the `thinking` tier | Sets the first-choice provider/model for the thinking tier. The GUI also updates the workspace `model` selection to the chosen concrete model. |
| Fast tier | Provider config `routes.fast.providers[0]` | Any listed model option for the `fast` tier | Sets the first-choice provider/model for the fast tier. The GUI also updates the workspace `model` selection to the chosen concrete model. |
| Other configured tiers | Provider config `routes.<tier>.providers[0]` | Any listed model option for that tier | If the provider config exposes additional tiers, the GUI shows and edits them the same way. |

## Provider settings

| Setting/action | Stored location | Effect |
| --- | --- | --- |
| Provider domain filter | Not persisted | Filters the provider list by domain, such as `llm`, `media_generation`, `search`, or `other`. |
| Provider search | In-memory GUI state | Filters the provider list by text while the GUI is open. |
| API key/token | `.env` resolved from `TURA_ENV_PATH`, or runtime project root `.env` | Stores the key under the provider method's `token_env`, validates it, and updates provider status. |
| OAuth login | `.env` and provider auth metadata in the provider auth flow | Starts OAuth login and stores resulting credentials when the flow completes. Auto flows poll until authenticated; manual/device flows require a code or provider-specific callback step. |
| Validate provider | No new setting unless a draft key is supplied | Checks stored or drafted credentials and displays a validation receipt. |
| Logout provider | Provider auth storage for that provider | Removes stored provider auth and refreshes the provider status. |

## Agent settings

| Setting | Stored location | Values | Effect |
| --- | --- | --- | --- |
| Agent provider | `<project-root>/agents/src/<agent-id>/agent_config.json`, inside the agent `provider` config | Any provider from the model config options | Sets the provider used by that custom agent. |
| Agent model | `<project-root>/agents/src/<agent-id>/agent_config.json`, inside the agent `provider` config | Any model available for the selected provider | Sets the model override used by that custom agent. If unset, the agent falls back to its default model tier. |
| Agent default model tier | `<project-root>/agents/src/<agent-id>/agent_config.json`, inside the agent `provider` config | `thinking`, `fast` | Selects which model tier the agent uses when it has no concrete model override. |
| Agent reasoning effort | `<project-root>/agents/src/<agent-id>/agent_config.json`, inside the agent `provider` config | `low`, `medium`, `high`, `xhigh`, `max` | Sets the reasoning-effort override for that agent. |
| Agent priority routing | `<project-root>/agents/src/<agent-id>/agent_config.json`, inside the agent `provider` config | `true`, `false` | Enables priority/accelerated routing for that agent where supported. |
| Delete agent | `<project-root>/agents/src/<agent-id>/` | Dynamic non-default agents only | Deletes the custom agent directory. Static agents and `default_config` agents are protected. |

## Personalization settings

| Setting | Stored key | Values | Effect |
| --- | --- | --- | --- |
| Persona | `active_persona` in `<workspace>/.tura/config.conf` | Any persona with media, plus built-in fallbacks `tura`, `wonderful`, `pidan` | Selects the persona used by the workspace and avatar preview. |
| Avatar display | `agent_avatar.display_mode` encoded in `<workspace>/.tura/config.conf` key `agent_avatar` | `hidden`, `static`, `dynamic` | Hides the avatar, shows a static avatar, or enables dynamic/interactive avatar rendering. |
| Avatar pixel size | `agent_avatar.pixel_size` encoded in `<workspace>/.tura/config.conf` key `agent_avatar` | `10` through `30` | Controls avatar pixel-art block size. |
| Avatar threshold | `agent_avatar.threshold` encoded in `<workspace>/.tura/config.conf` key `agent_avatar` | `100` through `200` | Controls the grayscale cutoff used when rendering avatar pixels. |

## Workspace config keys used by GUI

The GUI writes these keys to `<workspace>/.tura/config.conf` when runtime or personalization settings are saved:

| Key | Purpose |
| --- | --- |
| `language` | Workspace language. |
| `model` | Selected model or tier. |
| `active_provider` | Provider part of a concrete model pair. |
| `active_model` | Model part of a concrete model pair. |
| `active_agent` | Selected default agent. |
| `active_persona` | Selected persona. |
| `model_variant` | Reasoning-effort variant. |
| `model_acceleration_enabled` | Priority routing flag. |
| `agent_avatar` | JSON-encoded avatar display, pixel size, threshold, and persona metadata. |
