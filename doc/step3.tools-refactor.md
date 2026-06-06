# Tools Refactor Requirements

Status: proposed  
Scope: step 3 of 3  
Related: [step1.gateway-router-session-db-refactor.md](step1.gateway-router-session-db-refactor.md), [step2.runtime-refactor.md](step2.runtime-refactor.md)

## Goal

Make tools configurable like agents. Core tools remain compiled into runtime/tools and are invoked in-process. Non-core tools move to top-level `commands/` and execute as binaries or external programs. Router owns the tools registry and binary path resolution, but runtime/tools owns the actual external command process launch for command-run execution.

Initial classification:

```text
core = true:
  shell_command
  bash
  apply_patch
  task_status
  compact_context
  planning

core = false:
  read_media
  web_discover
```

`read_media` and `web_discover` move out of `crates/tools/src/commands` into top-level `commands/` and become external command packages.

## Current Code Reality

Current command execution is in-process:

```text
runtime
  -> code_tools::command_run
  -> code_tools::runtime::tool::ToolRouter
  -> crates/tools/src/commands/<command>
```

`ToolRouter` here is not `crates/router`; it is an in-process dispatcher. `shell_command` may spawn a shell, but command handler dispatch does not go through router today.

Tool prompt/schema/policy files currently live under:

```text
crates/tools/src/commands/<command>/
  mod.rs
  prompt.md
  schema.json
  policy.toml
```

The refactor must preserve prompt/schema/policy injection while introducing configurable manifests.

All current commands, including core commands that stay in `crates/tools`, must receive manifests and participate in the registry/config/state-machine model. This requirement applies to existing code, not only to moved external commands.

## Target Execution Model

Core command:

```text
runtime -> tools facade -> in-process core handler
```

External command:

```text
runtime -> tools facade -> router tools registry lookup -> hidden external CLI process
```

Router does not spawn non-core command processes for this phase. Router provides command metadata, enabled state, safe configuration, aliases, and resolved executable paths. Tools owns command behavior, schemas, prompts, policies, CLI protocol, and the runtime-side launch code.

## Tool Manifest

Every command must have a manifest. The manifest should be similar in spirit to agent config.

Suggested file:

```text
command.toml
```

Required fields:

```toml
id = "read_media"
name = "Read Media"
description = "Inspect local media and documents."
core = false
category = "media"
execution = "one_shot"
state_machine = "default_command"
supports_macro_command = true
mutating = false
network = false

[runtime]
binary = "tura-command-read-media"
entry = ""
language = "rust"

[limits]
default_timeout_ms = 60000
max_timeout_ms = 300000

[paths]
prompt = "prompt.md"
schema = "schema.json"
policy = "policy.toml"

[builder_config]
raw = {}
```

`core` is required, but it is not enough by itself. `execution` is also required:

```text
in_process
one_shot
persistent
```

Core commands should normally use `execution = "in_process"`.

External commands may use `one_shot` or `persistent`, but their child processes are launched by runtime/tools after registry lookup. They are not router children.

## Configurable Values

Each command can define configurable values in its manifest. This configuration shape is intentionally more flexible than the fixed command metadata, but UI-renderable config entries are restricted to three types:

```text
enum
string
boolean
```

Numeric settings must be modeled as `enum` values. This keeps gateway and frontend rendering predictable and avoids unconstrained numeric widgets.

```toml
[[configurable]]
key = "pdf_default_pages"
type = "enum"
default = "5"
description = "Default number of PDF pages to inspect."
enum = ["1", "3", "5", "10"]
```

String and boolean examples:

```toml
[[configurable]]
key = "download_directory"
type = "string"
default = ""
description = "Optional workspace-relative download directory."

[[configurable]]
key = "include_metadata"
type = "boolean"
default = "true"
description = "Include compact media metadata in output."
```

Router registry and gateway APIs must expose safe configurable values. The gateway write API must not allow arbitrary modification of security-sensitive fields:

Not user-editable by default:

- `core`
- `binary`
- `execution`
- `mutating`
- `network`
- policy path
- permission policy

User/config editable:

- enabled/disabled
- aliases
- timeout within allowed bounds
- configurable defaults

The manifest may also contain a raw command-owned JSON/TOML config object for builders, but gateway/frontend should only render entries declared in `[[configurable]]` with the allowed types above.

### Config Schema Rules

Configurable entries must follow this normalized schema:

```text
key
label
description
type: enum|string|boolean
default
enum: string list, required only for enum
required: boolean
scope: user|workspace|agent|session
```

Rules:

- `key` is stable and snake_case.
- All values are stored as strings or booleans in persisted config.
- `enum` values are strings even when they represent numbers.
- Numeric free-entry fields are not allowed.
- Nested UI-renderable config is not allowed.
- Command authors may use raw JSON/TOML under `builder_config`, but gateway/frontend must treat it as opaque.
- Gateway must validate config values against registry metadata before saving.
- Runtime/tools must receive merged config as JSON so command authors can evolve internal config without changing frontend rendering.

## Tool State Machine

Tools need a small state machine modeled after agent/session state management. The state machine describes registry/config/runtime availability, not business logic.

Target file:

```text
crates/tools/src/state_machine.rs
```

Router registry should expose persisted state through:

```text
crates/router/src/registry/tools.rs
```

Required states:

```text
Discovered
Configured
Enabled
Disabled
Unavailable
Running
Succeeded
Failed
```

State meanings:

- `Discovered`: manifest found but not merged with user config.
- `Configured`: manifest plus safe configurable defaults are loaded.
- `Enabled`: command may be offered to agents and command_run.
- `Disabled`: command exists but is not offered.
- `Unavailable`: external binary is missing or failed health check.
- `Running`: a runtime-owned external CLI process is currently executing.
- `Succeeded`: last execution completed successfully.
- `Failed`: last execution failed or returned invalid output.

Transitions:

```text
discover -> Discovered
load_config -> Configured
enable -> Enabled
disable -> Disabled
resolve_binary_failed -> Unavailable
execute_started -> Running
execute_succeeded -> Succeeded
execute_failed -> Failed
```

Core tools normally transition only through discovery/config/enabled and per-call running states. External tools also require binary resolution before execution.

State persistence:

- Manifest-derived state is read-only.
- Enabled/disabled and configurable values are user/workspace/session state.
- Last execution state is runtime state and should not overwrite manifest state.
- Router registry exposes all three layers as a merged view.

Suggested modules:

```text
crates/tools/src/state_machine.rs
crates/tools/src/config.rs
crates/router/src/registry/tools/state.rs
crates/router/src/registry/tools/config.rs
```

## Directory Layout

Target:

```text
commands/
  read_media/
    command.toml
    prompt.md
    schema.json
    policy.toml
    Cargo.toml or package metadata
    src/

  web_discover/
    command.toml
    prompt.md
    schema.json
    policy.toml
    Cargo.toml or package metadata
    src/

crates/tools/src/commands/
  shell_command/
    command.toml
    mod.rs
    prompt.md
    schema.json
    policy.toml
  apply_patch/
  task_status/
  compact_context/
  planning/
  bash/
```

Core commands keep their Rust implementation in `crates/tools`. They still get manifests so registry behavior is uniform.

External commands move implementation and heavy dependencies out of `crates/tools`.

Core command directory requirements:

```text
crates/tools/src/commands/<id>/
  command.toml
  mod.rs
  prompt.md
  schema.json
  policy.toml
```

External command directory requirements:

```text
commands/<id>/
  command.toml
  prompt.md
  schema.json
  policy.toml
  src/ or entry file
  package metadata
```

No command implementation file should exceed 1000 lines. Existing command modules must be split when touched:

```text
args.rs
access.rs
runner.rs
output.rs
config.rs
types.rs
```

This rule applies to current `read_media`, `web_discover`, `shell_command`, and future command packages.

## Router Registry

Add tools registry to router:

```text
crates/router/src/registry/tools.rs
```

Responsibilities:

- Discover core manifests under `crates/tools/src/commands`.
- Discover external manifests under top-level `commands`.
- Resolve aliases.
- Resolve binary path.
- Expose enabled commands for agent/runtime use.
- Provide command metadata to gateway.
- Enforce safe configurable write rules.
- Expose tool state and resolved executable metadata.

Router should not implement command business logic and should not spawn non-core command processes for command_run execution. Runtime/tools asks router for registry data, then launches the hidden CLI directly.

Registry module split:

```text
crates/router/src/registry/tools/
  mod.rs
  discover.rs
  manifest.rs
  aliases.rs
  config.rs
  state.rs
  resolve.rs
  api.rs
```

Responsibilities:

- `discover.rs`: scan core and external command locations.
- `manifest.rs`: parse and validate `command.toml`.
- `aliases.rs`: canonical command id and alias resolution.
- `config.rs`: safe configurable read/write and merging.
- `state.rs`: persisted tool state and runtime availability.
- `resolve.rs`: binary path resolution.
- `api.rs`: structs returned to gateway.

No registry file should exceed 1000 lines.

## Gateway Tool APIs

Add gateway APIs for frontend configuration:

```text
GET /tool
GET /tool/{toolID}
PATCH /tool/{toolID}
GET /tool/{toolID}/config
PATCH /tool/{toolID}/config
```

Gateway forwards registry/config requests to router. Gateway does not scan command directories directly.

Gateway must expose only safe registry/config operations. It must not expose raw manifest editing.

The API should return:

```json
{
  "id": "read_media",
  "core": false,
  "category": "media",
  "execution": "one_shot",
  "enabled": true,
  "aliases": ["view_media"],
  "supports_macro_command": true,
  "mutating": false,
  "configurable": []
}
```

Write API rules:

- `PATCH /tool/{toolID}` can enable/disable and edit aliases only when allowed.
- `PATCH /tool/{toolID}/config` can update only declared `[[configurable]]` keys.
- Gateway must reject unknown keys, invalid enum values, and attempts to edit `core`, `execution`, `binary`, `mutating`, `network`, or policy fields.
- Frontend should render only `enum`, `string`, and `boolean`.

## Tools Facade

`crates/tools` should expose a facade that chooses execution path:

```text
core manifest -> in-process handler
external manifest -> registry-resolved CLI launcher
```

Target files:

```text
crates/tools/src/registry/
  mod.rs
  manifest.rs
  core.rs

crates/tools/src/external/
  mod.rs
  client.rs
  launcher.rs
  protocol.rs
  state.rs

crates/tools/src/runtime/tool.rs
  remains facade, not a giant registry
```

Avoid coupling `crates/tools` directly to router internals. Use a protocol/client boundary.

Tools facade split:

```text
crates/tools/src/
  registry/
    mod.rs
    manifest.rs
    core.rs
  external/
    mod.rs
    client.rs
    launcher.rs
    protocol.rs
    state.rs
  runtime/
    tool.rs
    dispatch.rs
```

Rules:

- `runtime/tool.rs` should become facade/trait definitions and should not be a giant static registry.
- `runtime/dispatch.rs` should choose core vs external execution.
- `external/launcher.rs` is the only place that starts external command CLI processes.
- `external/client.rs` asks router registry for metadata and executable paths.
- Core handlers remain in command modules.

## External Command Protocol

External command binaries should support:

```text
health_check
capabilities
access
execute
cancel optional
```

Envelope:

```json
{ "kind": "health_check", "payload": {} }
{ "kind": "capabilities", "payload": {} }
{ "kind": "access", "payload": { "arguments": {}, "session_dir": "" } }
{ "kind": "execute", "payload": { "arguments": {}, "session_dir": "", "call_id": "" } }
```

Responses must be JSON and include:

```json
{ "ok": true, "output": {}, "success": true }
```

Runtime/tools launches external commands after resolving executable metadata from router. All processes must be hidden. On Windows, use process creation flags or equivalent options that do not show a console window.

External command process rules:

- Runtime/tools owns the external CLI child process.
- Router is not the parent of non-core command CLI processes.
- Runtime/tools must terminate the child on command cancellation or runtime cancellation.
- Runtime/tools must capture stdout/stderr.
- CLI output must be parsed as protocol JSON.
- Invalid JSON is a command failure with captured stderr/log tail.

## Read Media Migration

Move from:

```text
crates/tools/src/commands/read_media/
```

To:

```text
commands/read_media/
```

Requirements:

- Preserve `prompt.md`, `schema.json`, `policy.toml`.
- Move heavy dependencies out of `code-tools` if possible.
- Provide `tura-command-read-media` binary or package entry.
- Support `access` preflight for read paths.
- Support `supports_macro_command = true`.

## Web Discover Migration

Move from:

```text
crates/tools/src/commands/web_discover/
```

To:

```text
commands/web_discover/
```

Requirements:

- Preserve prompt/schema/policy.
- Move network and HTML/media dependencies out of `code-tools` if possible.
- Provide `tura-command-web-discover`.
- Support network policy metadata.
- Support `supports_macro_command = true`.

## Core Commands

Core commands stay compiled:

```text
shell_command
bash
apply_patch
task_status
compact_context
planning
```

They need manifests but should remain in-process initially.

Reasons:

- `task_status`, `planning`, and `compact_context` are tightly coupled to runtime/session semantics.
- `apply_patch` is tied to file locks, change tracking, and failure-stop behavior.
- `shell_command` already spawns user commands; externalizing its handler is not first priority.

## Agent Integration

Agents should reference command ids from the router tools registry. Agent command selection should be resolved using:

```text
agent config
router tools registry
command enabled state
mode/session policy
```

Runtime should receive an allowed command set from router/agent resolution rather than hard-coding command availability.

Runtime/tools may query router for updated tool registry metadata at command-run execution time. The query returns metadata and executable paths only; it does not create command child processes.

## Session DB And Checkpoint Interaction

External commands that mutate state must integrate with runtime checkpoint rules from [step2.runtime-refactor.md](step2.runtime-refactor.md):

- Runtime emits `command_started`.
- Command executes through core or external path.
- Runtime emits `command_finished` after output.
- Mutating command checkpoint must be acknowledged before continuing.

Tools do not write session DB directly unless explicitly allowed later. Runtime owns checkpoint emission.

External command result records must include:

```text
tool_id
command_id
core
binary_path
execution
arguments
success
output
stderr_summary
exit_code
started_at
finished_at
```

Runtime converts this into command-level checkpoints described in [step2.runtime-refactor.md](step2.runtime-refactor.md).

## Migration Steps

1. Add `command.toml` manifests to existing core commands.
2. Add router tools registry that can read core manifests.
3. Add gateway tool read APIs.
4. Add gateway tool safe config write APIs.
5. Add tools facade manifest loader.
6. Move `read_media` to `commands/read_media`.
7. Implement external command protocol and runtime-side hidden CLI launcher for `read_media`.
8. Move `web_discover` to `commands/web_discover`.
9. Implement external command protocol and runtime-side hidden CLI launcher for `web_discover`.
10. Add tools state machine and gateway/router state/config APIs.
11. Remove heavy media/web dependencies from `code-tools` when no longer needed.

Each migration step must keep existing command_run behavior working. Do not move `read_media` or `web_discover` until manifest discovery, prompt/schema/policy loading, and runtime external launcher are all in place.

## Tests

Required tests:

- Router loads core command manifests.
- Router loads external command manifests.
- Gateway lists tools through router.
- Unsafe manifest fields cannot be changed through gateway.
- Configurable values support only enum, string, and boolean UI-renderable entries.
- Numeric-like settings are represented as enum values.
- Tool state transitions match discovered/configured/enabled/running/succeeded/failed behavior.
- Core command executes in-process.
- External `read_media` resolves binary through router and is launched hidden by runtime/tools.
- External `web_discover` resolves binary through router and is launched hidden by runtime/tools.
- Agent allowed command list respects registry enabled state.
- Prompt/schema/policy injection works for core and external commands.
- Existing core commands expose manifests and registry metadata.
- Gateway rejects raw manifest edits.
- External CLI invalid JSON is reported as command failure.
- Runtime cancellation terminates a runtime-owned external command process.

## Non-Goals In Step 3

- Moving every command external.
- Externalizing `apply_patch`.
- Externalizing `shell_command`.
- Browser worker implementation, except reserving registry/lifecycle shape.
