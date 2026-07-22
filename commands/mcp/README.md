# Native MCP command

Tura reads stdio MCP servers from `$TURA_HOME/mcp.json`. Without an explicit
home it reads `<workspace>/.tura/mcp.json`.

```json
{
  "servers": {
    "code-review-graph": {
      "command": "uvx",
      "args": ["--from", "code-review-graph", "code-review-graph"],
      "cwd": ".",
      "env": {},
      "enabled": true,
      "allowed_tools": ["*"],
      "timeout_ms": 30000
    }
  }
}
```

The `mcp` macro command supports `list_tools` and `call_tool`. A tool call is
rejected unless its name, or `*`, appears in `allowed_tools`. Server processes
are scoped to one macro-command invocation and are terminated afterward.

