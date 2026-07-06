# Gateway

Gateway is the HTTP/SSE front used by TUI and GUI for health, config, providers, sessions, prompt streaming, files, projects, and product routes.

## Navigation

- [Documentation index](../SUMMARY.md)
- [Root overview](../../README.md)
- [Router](../architecture/router.md)
- [Terminal User Interface](../architecture/terminal-user-interface.md)
- [Graphic User Interface](../architecture/graphic-user-interface.md)
- [Session DB](../architecture/session-db.md)

## Mental model

This page belongs to the **Architecture** group. Read it as a practical operator guide, not as a marketing page.
The implementation is real code in this repository, and the source references below name functions and file paths without line numbers so the document stays stable across edits.
Tura favors explicit state, narrow tool surfaces, and verification evidence. That is the recurring shape behind this topic.

## Source references

- `build_router` in `crates/gateway/src/web/server.rs`
- `run_server` in `crates/gateway/src/web/server.rs`
- `run_server_until_shutdown` in `crates/gateway/src/web/server.rs`
- `local_bind_addr` in `crates/gateway/src/web/server.rs`
- `session_store` in `crates/gateway/src/session/mod.rs`
- `SessionLogStore` in `crates/session_log/src/store.rs`
- `run_socket_service` in `crates/session_log/src/service.rs`
- `SessionLogClient` in `crates/session_log/src/client.rs`
- `UpsertSessionRequest` in `crates/session_log/src/protocol.rs`
- `CommandCheckpoint` in `crates/session_log/src/checkpoint.rs`
- `tura_llm` in `crates/provider/src/tura_llm.rs`
- `load_tura_config` in `crates/provider/src/tura_conf.rs`
- `extract_response_text` in `crates/provider/src/response_extraction.rs`
- `extract_tool_calls` in `crates/provider/src/response_extraction.rs`
- `provider_config.json` in `crates/provider/config/provider_config.json`

## Quick examples

### Health

```text
GET /global/health
```

### Prompt

```text
POST /session/{sessionID}/prompt_async
```

### Events

```text
GET /event
```

## Operational rules

1. Choose the smallest front that exposes the behavior you need.
2. Keep workspace paths explicit when a command depends on a repository.
3. Prefer structured JSON output for scripts, tests, and automation.
4. Treat provider, session, and command records as different evidence channels.
5. Verify behavior at the boundary that owns the contract, not at a convenient proxy.

## Common failure modes

| Failure | Why it hurts | Safer move |
| --- | --- | --- |
| Treating gateway as a loose concept | The wrong layer gets changed and tests become decorative | Use the source references on this page to find the owning code first |
| Relying on prompt wording as proof | The runtime may satisfy text while breaking the product contract | Inspect stored records, command output, API responses, or files |
| Skipping environment details | Local homes, binaries, sockets, or provider config silently differ | Print the active path or config before debugging behavior |
| Mixing UI and backend ownership | The GUI or TUI becomes a second runtime by accident | Keep clients thin and route through gateway/router APIs |

## Practical workflow

1. Start from the user-facing action related to **Gateway**.
2. Identify the stable boundary: CLI, gateway endpoint, router service, runtime loop, tool handler, provider call, or session store.
3. Read the source file that owns that boundary before changing anything.
4. Run the smallest command that proves the current behavior.
5. Make the minimal change in the owner, not in a downstream workaround.
6. Verify with the focused test or command that observes the same boundary.
7. Update documentation if the command, setting, route, or behavior changed.

## Detailed guide

### What it owns

Gateway has one practical ownership question: which process or module is allowed to decide the behavior. Tura keeps that answer explicit so a UI, script, or prompt does not quietly duplicate backend logic.

- Find the owner before patching adjacent code.
- Use gateway endpoints for client-facing state instead of reading private files.
- Use router services for process ownership and command execution when a runtime worker is involved.
- Use session_log APIs for durable history instead of ad hoc JSON files.

### How it appears to users

Users usually meet Gateway through commands, settings, sessions, or visible UI behavior. The documentation should explain that path first, then point to the implementation detail only when it helps the user operate or customize the system.

- Prefer examples that can be pasted into a shell or matched to a UI screen.
- Explain when a result is stored, streamed, printed, or only held in memory.
- Name required environment variables and files close to the example that needs them.
- Avoid implying that live providers or paid services are required for local business tests.

### How it is verified

Verification for Gateway should observe the same contract the user depends on. If the user depends on CLI output, check CLI output. If the user depends on a session record, query session_log. If the user depends on command execution, verify command_run results.

- Use focused tests before broad CI when investigating a specific failure.
- Use broad CI after cross-crate or packaging changes.
- Keep live-provider checks separate from deterministic business checks.
- Record the command and result in the final change summary.

### How to extend it safely

Extending Gateway should follow the existing repository shape. Add behavior to the owning crate, expose it through the existing gateway/router/runtime/tool boundary, then update clients and docs as thin consumers.

- Do not add a second source of truth for settings or session state.
- Do not bypass router ownership for long-running child processes.
- Do not put provider secrets into repository files or public assets.
- Do not use narrow smoke tests as proof of compatibility when the public surface is broader.

### Client boundary

Clients should remain thin. They format state, call gateway endpoints, subscribe to events, and display errors; they do not become alternate runtimes or private session stores.

- Keep gateway API clients typed.
- Use SSE events for live transcript updates.
- Let gateway own provider auth portals and session mutation routes.
- Avoid reading private workspace database files from UI code.

## Reference scenarios

### Scenario 1: Gateway operator path

When working with gateway, first inspect the active configuration. The relevant owner for this scenario is `build_router` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `build_router` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 2: Gateway operator path

When working with gateway, first run a focused command. The relevant owner for this scenario is `run_server` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_server` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 3: Gateway operator path

When working with gateway, first query the gateway or session store. The relevant owner for this scenario is `run_server_until_shutdown` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_server_until_shutdown` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 4: Gateway operator path

When working with gateway, first check the owning source file. The relevant owner for this scenario is `local_bind_addr` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `local_bind_addr` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 5: Gateway operator path

When working with gateway, first verify the result with a deterministic test. The relevant owner for this scenario is `session_store` in `crates/gateway/src/session/mod.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/session/mod.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `session_store` in `crates/gateway/src/session/mod.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 6: Gateway operator path

When working with gateway, first update linked documentation when behavior changes. The relevant owner for this scenario is `SessionLogStore` in `crates/session_log/src/store.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/store.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `SessionLogStore` in `crates/session_log/src/store.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 7: Gateway operator path

When working with gateway, first inspect the active configuration. The relevant owner for this scenario is `run_socket_service` in `crates/session_log/src/service.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/service.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_socket_service` in `crates/session_log/src/service.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 8: Gateway operator path

When working with gateway, first run a focused command. The relevant owner for this scenario is `SessionLogClient` in `crates/session_log/src/client.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/client.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `SessionLogClient` in `crates/session_log/src/client.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 9: Gateway operator path

When working with gateway, first query the gateway or session store. The relevant owner for this scenario is `UpsertSessionRequest` in `crates/session_log/src/protocol.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/protocol.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `UpsertSessionRequest` in `crates/session_log/src/protocol.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 10: Gateway operator path

When working with gateway, first check the owning source file. The relevant owner for this scenario is `CommandCheckpoint` in `crates/session_log/src/checkpoint.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/checkpoint.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `CommandCheckpoint` in `crates/session_log/src/checkpoint.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 11: Gateway operator path

When working with gateway, first verify the result with a deterministic test. The relevant owner for this scenario is `tura_llm` in `crates/provider/src/tura_llm.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/provider/src/tura_llm.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `tura_llm` in `crates/provider/src/tura_llm.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 12: Gateway operator path

When working with gateway, first update linked documentation when behavior changes. The relevant owner for this scenario is `load_tura_config` in `crates/provider/src/tura_conf.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/provider/src/tura_conf.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `load_tura_config` in `crates/provider/src/tura_conf.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 13: Gateway operator path

When working with gateway, first inspect the active configuration. The relevant owner for this scenario is `extract_response_text` in `crates/provider/src/response_extraction.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/provider/src/response_extraction.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `extract_response_text` in `crates/provider/src/response_extraction.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 14: Gateway operator path

When working with gateway, first run a focused command. The relevant owner for this scenario is `extract_tool_calls` in `crates/provider/src/response_extraction.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/provider/src/response_extraction.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `extract_tool_calls` in `crates/provider/src/response_extraction.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 15: Gateway operator path

When working with gateway, first query the gateway or session store. The relevant owner for this scenario is `provider_config.json` in `crates/provider/config/provider_config.json`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/provider/config/provider_config.json` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `provider_config.json` in `crates/provider/config/provider_config.json`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 16: Gateway operator path

When working with gateway, first check the owning source file. The relevant owner for this scenario is `build_router` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `build_router` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 17: Gateway operator path

When working with gateway, first verify the result with a deterministic test. The relevant owner for this scenario is `run_server` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_server` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 18: Gateway operator path

When working with gateway, first update linked documentation when behavior changes. The relevant owner for this scenario is `run_server_until_shutdown` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for gateway | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_server_until_shutdown` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

## Checklist

- [ ] 1. I can explain what Gateway owns in one sentence.
- [ ] 2. I know which process or crate owns Gateway behavior.
- [ ] 3. I checked the source references listed above before changing behavior.
- [ ] 4. I used a command, test, API route, or stored record as evidence.
- [ ] 5. I did not store secrets, tokens, or provider credentials in docs or examples.
- [ ] 6. I did not make the GUI or TUI duplicate backend runtime behavior.
- [ ] 7. I kept deterministic tests separate from live-provider checks.
- [ ] 8. I updated cross-links when adding or moving a related page.
- [ ] 9. I preserved old Markdown files outside this new doc tree.
- [ ] 10. I can point to the exact function name and source file for the main behavior.
- [ ] 11. Gateway documentation still matches the current implementation and examples.
- [ ] 12. Gateway documentation still matches the current implementation and examples.
- [ ] 13. Gateway documentation still matches the current implementation and examples.
- [ ] 14. Gateway documentation still matches the current implementation and examples.
- [ ] 15. Gateway documentation still matches the current implementation and examples.
- [ ] 16. Gateway documentation still matches the current implementation and examples.
- [ ] 17. Gateway documentation still matches the current implementation and examples.
- [ ] 18. Gateway documentation still matches the current implementation and examples.
- [ ] 19. Gateway documentation still matches the current implementation and examples.
- [ ] 20. Gateway documentation still matches the current implementation and examples.
- [ ] 21. Gateway documentation still matches the current implementation and examples.
- [ ] 22. Gateway documentation still matches the current implementation and examples.
- [ ] 23. Gateway documentation still matches the current implementation and examples.
- [ ] 24. Gateway documentation still matches the current implementation and examples.
- [ ] 25. Gateway documentation still matches the current implementation and examples.
- [ ] 26. Gateway documentation still matches the current implementation and examples.
- [ ] 27. Gateway documentation still matches the current implementation and examples.
- [ ] 28. Gateway documentation still matches the current implementation and examples.
- [ ] 29. Gateway documentation still matches the current implementation and examples.
- [ ] 30. Gateway documentation still matches the current implementation and examples.

## See also

- [Router](../architecture/router.md)
- [Terminal User Interface](../architecture/terminal-user-interface.md)
- [Graphic User Interface](../architecture/graphic-user-interface.md)
- [Session DB](../architecture/session-db.md)

## Maintenance notes

- Maintenance note 1: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 2: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 3: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 4: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 5: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 6: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 7: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 8: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 9: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 10: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 11: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 12: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 13: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 14: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 15: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 16: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 17: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 18: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 19: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 20: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 21: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 22: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 23: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 24: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 25: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 26: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 27: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 28: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 29: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 30: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 31: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 32: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 33: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 34: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 35: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 36: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 37: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 38: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 39: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 40: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 41: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 42: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 43: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 44: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
- Maintenance note 45: keep `architecture/gateway.md` aligned with crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs, crates/gateway/src/web/server.rs.
