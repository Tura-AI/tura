# How to start

Use the smallest front end that fits the job. The detailed reference is
[docs/start/how-to-start.md](../../docs/start/how-to-start.md).

| Start method | Best for | Command |
| --- | --- | --- |
| TUI | Interactive terminal work | `tura` |
| CLI one-shot | Direct prompt from a shell or script | `tura exec "..."` |
| CLI via gateway | Scriptable prompt with gateway streaming/history | `tura run "..."` |
| GUI desktop | Visual workspace and session management | `tura_gui` |
| Web GUI/gateway | Browser GUI and HTTP/SSE API | `tura_gateway` |
| Source shortcut | Start from the checkout | `scripts/start.*` |

## Common starts

```sh
tura
tura "Inspect this repository"
tura exec "Find the riskiest area in this workspace"
tura run "Summarize the current session"
```

For flags and low-level binaries, continue to [CLI parameters](cli-parameters.md).
