# Custom commands

Commands are local execution units routed through `command_run`. They are not
arbitrary shell lines wearing a schema. `shell_command` is a Tura command;
`rg --files` is a CLI command executed inside it. Keeping those layers separate
makes policy and execution much easier to reason about.

Tura has two command families:

| Family | Location | How it runs |
| --- | --- | --- |
| Core/internal commands | `crates/tools/src/commands/<id>` | Compiled into the tools/runtime path. |
| External command packages | `commands/<id>` | Launched as a separate binary through Tura's JSON protocol. |

For customization, prefer external command packages. Changing a core command is
a source-code change and deserves the same care as runtime development. The
shorter path is not always the one through the engine room.

## Command discovery

External command manifests are discovered from the project root in this order:

| Priority | Root |
| --- | --- |
| 1 | `<project-root>/commands/<command_id>/command.toml` |
| 2 | `<project-root>/crates/tools/src/commands/<command_id>/command.toml` |

The first manifest with a given id wins. This lets workspace command packages
override lower-priority registrations.

The project root usually comes from `TURA_PROJECT_ROOT`. Set it explicitly for
release custom commands.

## Release view

Full release builds copy built-in external commands to:

```text
<release-root>/commands/generate_media/
<release-root>/commands/read_media/
<release-root>/commands/web_discover/
```

Add a custom command package here:

```text
<release-root>/commands/my_command/
  command.toml
  prompt.md
  schema.json
  policy.toml
```

Place the command binary in one of the supported binary locations:

```text
<release-root>/my-command-binary(.exe)
<release-root>/bin/my-command-binary(.exe)
<release-root>/target/release/my-command-binary(.exe)
<release-root>/target/debug/my-command-binary(.exe)
```

Or set an explicit binary override. For a manifest binary named
`tura-command-my-tool`, the override variable is:

```powershell
$env:TURA_MY_TOOL_BIN = "D:\\tools\\tura-command-my-tool.exe"
```

Start with:

```powershell
$env:TURA_PROJECT_ROOT = "C:\\path\\to\\tura-release"
tura --agent my-agent "Use my custom command when needed."
```

The command also has to be exposed through an agent capability or an active
runtime prompt manual capability. Registration alone does not make the command
model-visible.

## Source view

From source, create:

```text
commands/my_command/
  Cargo.toml              # if Rust package
  command.toml
  prompt.md
  schema.json
  policy.toml
  src/main.rs             # or another executable implementation
```

If it is a Rust command package, add it to the workspace `Cargo.toml` and build
it:

```sh
cargo build -p my_command
```

For a non-Rust command, provide an executable binary and point `runtime.binary`
at its binary name. The launcher communicates through stdin/stdout using Tura's
external command JSON protocol.

## Minimal external command manifest

`commands/my_echo/command.toml`:

```toml
id = "my_echo"
name = "My Echo"
description = "Echoes a short message through the external command protocol."
core = false
category = "utility"
execution = "one_shot"
state_machine = "default_command"
supports_macro_command = true
mutating = false
network = false

[runtime]
binary = "tura-command-my-echo"
entry = ""
language = "rust"

[limits]
default_timeout_ms = 15000
max_timeout_ms = 60000

[paths]
prompt = "prompt.md"
schema = "schema.json"
policy = "policy.toml"
```

Only these manifest fields are used by the current registry loader:

| Field | Used for |
| --- | --- |
| `id` | Command id inside `command_run`. |
| `core` | Whether command is internal. Custom external commands use `false`. |
| `execution` | `one_shot` is supported for external CLI packages. |
| `supports_macro_command` | Whether macro command dispatch is safe. |
| `mutating` | Whether the command changes local state/files. |
| `runtime.binary` | Binary name to launch. |
| `limits.default_timeout_ms` | Default timeout. |
| `limits.max_timeout_ms` | Maximum timeout. |

Other fields are still useful documentation/UI metadata, but do not assume they
change runtime behavior unless the loader reads them.

## Schema

`schema.json` describes the command's input shape:

```json
{
  "name": "my_echo",
  "description": "Echo a message.",
  "input_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "text": {
        "type": "string",
        "description": "Message to echo."
      }
    },
    "required": ["text"]
  }
}
```

Keep schemas small and strict. A vague schema gives the model room to invent
arguments, then everyone gets to debug vibes. Delightful, no.

## Prompt

`prompt.md` should explain how to use the command from inside `command_run`:

```md
Use `my_echo` only when the user asks to test the custom command. Pass a short
`text` value. Do not use it for normal shell output.
```

## Policy

`policy.toml` documents safety properties:

```toml
read_only = true
network = false
```

Policy files should match actual behavior. If the command writes files, do not
call it read-only. The runtime is allowed to become stricter over time.

## External command protocol

The launcher starts the binary with:

```text
<binary> --protocol
```

It sends JSON on stdin:

```json
{
  "kind": "execute",
  "payload": {
    "arguments": { "text": "hello" },
    "session_dir": "...",
    "call_id": "..."
  }
}
```

The binary must print protocol JSON on stdout:

```json
{
  "ok": true,
  "success": true,
  "exit_code": 0,
  "output": {
    "text": "hello"
  },
  "stderr": ""
}
```

On failure, return `ok: false` or `success: false` and include either
`output.error` or `stderr`.

## Expose the command to an agent

Add the command id to an agent's `agent_capabilities`:

```json
{
  "agent_capabilities": [
    { "capability_name": "shells" },
    { "capability_name": "task_status" },
    { "capability_name": "my_echo" }
  ]
}
```

Or expose it only for a task type by adding it to a runtime prompt manual:

```json
{
  "capabilities": ["my_echo"]
}
```

Use the second option when the command is task-specific. It keeps the base agent
surface smaller.

## Binary resolution

For `runtime.binary = "tura-command-my-echo"`, Tura checks:

1. `TURA_MY_ECHO_BIN`, if set and the path exists;
2. the current executable directory;
3. `<TURA_PROJECT_ROOT>/bin/`;
4. `<TURA_PROJECT_ROOT>/`;
5. `<TURA_PROJECT_ROOT>/target/release/`;
6. `<TURA_PROJECT_ROOT>/target/debug/`;
7. the same `bin`, root, `target/release`, and `target/debug` candidates under
   the discovered repo root.

On Windows, `.exe` is appended automatically for candidate checks.

If no binary is found, source checkouts can fall back to:

```text
cargo run -q -p <command_id> -- --protocol
```

That fallback only works when the command is a Cargo package in the workspace.

## Validation

From source:

```sh
cargo test -q -p tools registry
cargo test -q -p tools external
cargo build -p my_command
```

Manual protocol test:

```sh
printf '{"kind":"execute","payload":{"arguments":{"text":"hello"},"session_dir":".","call_id":"manual"}}' | tura-command-my-echo --protocol
```

Runtime smoke test:

```sh
TURA_PROJECT_ROOT=/path/to/root tura --agent my-agent "Call my_echo with text hello."
```

Check that:

- `command.toml` is under the discovered `commands/<id>` directory;
- `runtime.binary` resolves to an executable or Cargo fallback works;
- the agent/manual exposes the command capability;
- timeout limits are large enough for expected work but bounded;
- mutating/network flags match reality;
- failures return compact useful errors.

## Common failures

| Symptom | Likely cause |
| --- | --- |
| Unsupported external command | Manifest not found, wrong `TURA_PROJECT_ROOT`, `core=true`, or unsupported execution mode. |
| Failed to start command | Binary not found and Cargo fallback is unavailable. |
| Command not visible to model | Not included in `agent_capabilities` or active manual `capabilities`. |
| Invalid protocol JSON | Binary printed logs to stdout or returned the wrong response shape. |
| Command times out | `limits.default_timeout_ms` too low or process waits silently. |
| Release command works only from source | Binary exists only under source `target/debug`; copy it or set `TURA_<NAME>_BIN`. |
