# Native MCP integration

## Problem

Tura has MCP-oriented architecture but the current installed release does not provide a complete user-facing MCP client path. A local command bridge can prove a server works, but hardcoded executable and repository paths are not portable or maintainable.

## Work

- Add native MCP server configuration, lifecycle management, capability discovery, and tool invocation.
- Support stdio servers first, with explicit environment and working-directory configuration.
- Surface connection and tool errors in the UI.
- Add per-server and per-tool permission controls, including read-only defaults where possible.
- Use code-review-graph as an integration fixture without coupling Tura to that server.

## Acceptance

- A user can configure code-review-graph and invoke it from a desktop session.
- Configuration contains no machine-specific paths and survives app restarts.
- Unavailable servers fail visibly without breaking ordinary chat.
- Tests cover handshake, discovery, invocation, timeout, cancellation, and denied permissions.

