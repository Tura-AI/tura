# CLI And TUI

Tura exposes two terminal entrypoints.

The Rust CLI is the direct local execution path. It runs the runtime in-process
through the `gateway` crate binary:

```bash
cargo run -p gateway --bin tura -- exec "Inspect the workspace"
```

The TypeScript CLI/TUI is the gateway client path. It communicates with
`crates/gateway` over HTTP/SSE and is the preferred surface for terminal UX,
session management, provider status, permissions, and future interactive TUI
work:

```bash
node apps/tui/dist/index.js --help
```

## Rust CLI

Usage:

```text
tura exec [OPTIONS] [PROMPT...]
tura [OPTIONS] [PROMPT...]
```

Important options:

```text
-C, --cwd PATH                  workspace directory for the session
-m, --model MODEL               model override; bare names become openai/MODEL
    --agent NAME                agent override
    --session-id ID             reuse a deterministic session id
    --json                      emit JSONL events
    --output-last-message PATH  write the final assistant message to PATH
    --multiple-tasks-mode       enable the multiple_tasks command surface
-c, --config KEY=VALUE          runtime override
```

If no prompt is passed, the Rust CLI reads stdin.

Examples:

```bash
cargo run -p gateway --bin tura -- exec -C . -m openai/gpt-5 "Fix the failing test"
echo "Summarize the architecture" | cargo run -p gateway --bin tura -- exec --json
```

## TypeScript CLI/TUI

Usage:

```text
tura [OPTIONS]                         open the interactive TUI
tura [OPTIONS] run [PROMPT...]         run a non-interactive prompt
tura [OPTIONS] resume SESSION_ID       show or continue a session
tura [OPTIONS] <command> --help        show command-specific help
```

Root options:

```text
--gateway-url URL   gateway base URL
--cwd PATH          workspace directory sent to gateway
--json              JSON output where supported
--color MODE        auto, always, or never
--verbose           print gateway requests to stderr
```

Commands:

```text
run           send a prompt through the gateway and stream the answer
resume        show an existing session or append a follow-up prompt
session       list, show, or delete sessions
config        read or update workspace session config
provider      list providers and inspect auth state
permission    list and answer pending permission requests
command       list or execute gateway slash commands
status        print gateway, workspace, provider, and service status
completion    generate shell completion
```

### `run`

```text
tura run [PROMPT...] [RUN_OPTIONS]
```

Options:

```text
--session ID                  append the prompt to an existing session
--model PROVIDER/MODEL        request-scoped model override
--agent NAME                  request-scoped agent override
--session-type TYPE           session type passed to gateway
--model-variant LEVEL         reasoning/model variant override
--reasoning-effort LEVEL      alias for --model-variant
--model-acceleration          enable priority/accelerated model routing
--no-model-acceleration       disable priority/accelerated routing
--output text|json|ndjson     output format
--json                        alias for --output json
--stream, --no-stream         stream gateway events or poll for completion
--timeout SEC                 timeout before aborting the turn
--last-message-file PATH      write the final assistant message to PATH
-c, --config KEY=VALUE        runtime override
```

Examples:

```bash
node apps/tui/dist/index.js run "Inspect the workspace"
node apps/tui/dist/index.js run --output ndjson "Fix the failing test"
node apps/tui/dist/index.js run --model openai/gpt-5 --agent coding_agent "Plan the refactor"
```

### Session And Resume

```bash
node apps/tui/dist/index.js session list
node apps/tui/dist/index.js session show SESSION_ID
node apps/tui/dist/index.js session delete SESSION_ID
node apps/tui/dist/index.js resume --last "Continue from the previous result"
```

### Config

```bash
node apps/tui/dist/index.js config get
node apps/tui/dist/index.js config get model
node apps/tui/dist/index.js config set model=openai/gpt-5 agent=coding_agent
```

Config is read and written through the gateway session-config API for the
selected workspace.

### Providers And Permissions

```bash
node apps/tui/dist/index.js provider list
node apps/tui/dist/index.js provider status openai
node apps/tui/dist/index.js provider logout openai
node apps/tui/dist/index.js permission list
node apps/tui/dist/index.js permission reply REQUEST_ID --approve
node apps/tui/dist/index.js permission reply REQUEST_ID --deny
```

### Gateway Commands And Status

```bash
node apps/tui/dist/index.js command list
node apps/tui/dist/index.js command run NAME ARGS...
node apps/tui/dist/index.js status
node apps/tui/dist/index.js completion bash
```

## Gateway URL Resolution

The TypeScript client resolves the gateway URL in this order:

1. `--gateway-url`.
2. `TURA_GATEWAY_URL`.
3. `http://127.0.0.1:4096`.

Workspace-scoped requests send the selected directory through both the query
string and `x-opencode-directory` header where the gateway route expects it.

## Exit Codes

The TypeScript CLI uses deterministic exit codes:

```text
0 completed
1 gateway/runtime/provider error
2 invalid CLI usage
3 permission denied or cancelled
4 timeout
5 gateway unavailable
```

## Shell Completion

```bash
node apps/tui/dist/index.js completion bash
node apps/tui/dist/index.js completion zsh
node apps/tui/dist/index.js completion fish
```

Install the generated completion with the standard mechanism for your shell.
