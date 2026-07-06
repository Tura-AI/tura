# Session DB

Session DB is the single SQLite owner per TURA_HOME and the durable store for workspace sessions, records, task state, todos, and queued writes.

## Navigation

- [Documentation index](../SUMMARY.md)
- [Root overview](../../README.md)
- [Sessions](../start/sessions.md)
- [Gateway](../architecture/gateway.md)
- [Router](../architecture/router.md)
- [Context Management](../core/context-management.md)

## Mental model

This page belongs to the **Architecture** group. Read it as a practical operator guide, not as a marketing page.
The implementation is real code in this repository, and the source references below name functions and file paths without line numbers so the document stays stable across edits.
Tura favors explicit state, narrow tool surfaces, and verification evidence. That is the recurring shape behind this topic.

## Source references

- `SessionLogStore` in `crates/session_log/src/store.rs`
- `run_socket_service` in `crates/session_log/src/service.rs`
- `SessionLogClient` in `crates/session_log/src/client.rs`
- `UpsertSessionRequest` in `crates/session_log/src/protocol.rs`
- `CommandCheckpoint` in `crates/session_log/src/checkpoint.rs`
- `run_router_command` in `crates/router/src/cli.rs`
- `serve_socket` in `crates/router/src/daemon.rs`
- `serve_stdio` in `crates/router/src/daemon.rs`
- `dispatch_run_agent` in `crates/router/src/runtime_dispatch.rs`
- `CommandRunService::execute` in `crates/router/src/services/command_run.rs`
- `build_router` in `crates/gateway/src/web/server.rs`
- `run_server` in `crates/gateway/src/web/server.rs`
- `run_server_until_shutdown` in `crates/gateway/src/web/server.rs`
- `local_bind_addr` in `crates/gateway/src/web/server.rs`
- `session_store` in `crates/gateway/src/session/mod.rs`

## Quick examples

### Query workspaces

```text
{"command":"list_workspaces"} | tura_gateway session-log
```

### Query sessions

```text
{"command":"list_sessions","workspace":"C:/repo"} | tura_gateway session-log
```

### HTTP records

```text
GET /session-log/{sessionID}/records?page=0&page_size=100
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
| Treating session db as a loose concept | The wrong layer gets changed and tests become decorative | Use the source references on this page to find the owning code first |
| Relying on prompt wording as proof | The runtime may satisfy text while breaking the product contract | Inspect stored records, command output, API responses, or files |
| Skipping environment details | Local homes, binaries, sockets, or provider config silently differ | Print the active path or config before debugging behavior |
| Mixing UI and backend ownership | The GUI or TUI becomes a second runtime by accident | Keep clients thin and route through gateway/router APIs |

## Practical workflow

1. Start from the user-facing action related to **Session DB**.
2. Identify the stable boundary: CLI, gateway endpoint, router service, runtime loop, tool handler, provider call, or session store.
3. Read the source file that owns that boundary before changing anything.
4. Run the smallest command that proves the current behavior.
5. Make the minimal change in the owner, not in a downstream workaround.
6. Verify with the focused test or command that observes the same boundary.
7. Update documentation if the command, setting, route, or behavior changed.

## Detailed guide

### What it owns

Session DB has one practical ownership question: which process or module is allowed to decide the behavior. Tura keeps that answer explicit so a UI, script, or prompt does not quietly duplicate backend logic.

- Find the owner before patching adjacent code.
- Use gateway endpoints for client-facing state instead of reading private files.
- Use router services for process ownership and command execution when a runtime worker is involved.
- Use session_log APIs for durable history instead of ad hoc JSON files.

### How it appears to users

Users usually meet Session DB through commands, settings, sessions, or visible UI behavior. The documentation should explain that path first, then point to the implementation detail only when it helps the user operate or customize the system.

- Prefer examples that can be pasted into a shell or matched to a UI screen.
- Explain when a result is stored, streamed, printed, or only held in memory.
- Name required environment variables and files close to the example that needs them.
- Avoid implying that live providers or paid services are required for local business tests.

### How it is verified

Verification for Session DB should observe the same contract the user depends on. If the user depends on CLI output, check CLI output. If the user depends on a session record, query session_log. If the user depends on command execution, verify command_run results.

- Use focused tests before broad CI when investigating a specific failure.
- Use broad CI after cross-crate or packaging changes.
- Keep live-provider checks separate from deterministic business checks.
- Record the command and result in the final change summary.

### How to extend it safely

Extending Session DB should follow the existing repository shape. Add behavior to the owning crate, expose it through the existing gateway/router/runtime/tool boundary, then update clients and docs as thin consumers.

- Do not add a second source of truth for settings or session state.
- Do not bypass router ownership for long-running child processes.
- Do not put provider secrets into repository files or public assets.
- Do not use narrow smoke tests as proof of compatibility when the public surface is broader.

## Reference scenarios

### Scenario 1: Session DB operator path

When working with session db, first inspect the active configuration. The relevant owner for this scenario is `SessionLogStore` in `crates/session_log/src/store.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/store.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `SessionLogStore` in `crates/session_log/src/store.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 2: Session DB operator path

When working with session db, first run a focused command. The relevant owner for this scenario is `run_socket_service` in `crates/session_log/src/service.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/service.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_socket_service` in `crates/session_log/src/service.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 3: Session DB operator path

When working with session db, first query the gateway or session store. The relevant owner for this scenario is `SessionLogClient` in `crates/session_log/src/client.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/client.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `SessionLogClient` in `crates/session_log/src/client.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 4: Session DB operator path

When working with session db, first check the owning source file. The relevant owner for this scenario is `UpsertSessionRequest` in `crates/session_log/src/protocol.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/protocol.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `UpsertSessionRequest` in `crates/session_log/src/protocol.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 5: Session DB operator path

When working with session db, first verify the result with a deterministic test. The relevant owner for this scenario is `CommandCheckpoint` in `crates/session_log/src/checkpoint.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/checkpoint.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `CommandCheckpoint` in `crates/session_log/src/checkpoint.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 6: Session DB operator path

When working with session db, first update linked documentation when behavior changes. The relevant owner for this scenario is `run_router_command` in `crates/router/src/cli.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/cli.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_router_command` in `crates/router/src/cli.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 7: Session DB operator path

When working with session db, first inspect the active configuration. The relevant owner for this scenario is `serve_socket` in `crates/router/src/daemon.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/daemon.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `serve_socket` in `crates/router/src/daemon.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 8: Session DB operator path

When working with session db, first run a focused command. The relevant owner for this scenario is `serve_stdio` in `crates/router/src/daemon.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/daemon.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `serve_stdio` in `crates/router/src/daemon.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 9: Session DB operator path

When working with session db, first query the gateway or session store. The relevant owner for this scenario is `dispatch_run_agent` in `crates/router/src/runtime_dispatch.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/runtime_dispatch.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `dispatch_run_agent` in `crates/router/src/runtime_dispatch.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 10: Session DB operator path

When working with session db, first check the owning source file. The relevant owner for this scenario is `CommandRunService::execute` in `crates/router/src/services/command_run.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/services/command_run.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `CommandRunService::execute` in `crates/router/src/services/command_run.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 11: Session DB operator path

When working with session db, first verify the result with a deterministic test. The relevant owner for this scenario is `build_router` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `build_router` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 12: Session DB operator path

When working with session db, first update linked documentation when behavior changes. The relevant owner for this scenario is `run_server` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_server` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 13: Session DB operator path

When working with session db, first inspect the active configuration. The relevant owner for this scenario is `run_server_until_shutdown` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_server_until_shutdown` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 14: Session DB operator path

When working with session db, first run a focused command. The relevant owner for this scenario is `local_bind_addr` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `local_bind_addr` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 15: Session DB operator path

When working with session db, first query the gateway or session store. The relevant owner for this scenario is `session_store` in `crates/gateway/src/session/mod.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/session/mod.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `session_store` in `crates/gateway/src/session/mod.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 16: Session DB operator path

When working with session db, first check the owning source file. The relevant owner for this scenario is `SessionLogStore` in `crates/session_log/src/store.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/store.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `SessionLogStore` in `crates/session_log/src/store.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 17: Session DB operator path

When working with session db, first verify the result with a deterministic test. The relevant owner for this scenario is `run_socket_service` in `crates/session_log/src/service.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/service.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_socket_service` in `crates/session_log/src/service.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 18: Session DB operator path

When working with session db, first update linked documentation when behavior changes. The relevant owner for this scenario is `SessionLogClient` in `crates/session_log/src/client.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for session db | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/client.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `SessionLogClient` in `crates/session_log/src/client.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

## Checklist

- [ ] 1. I can explain what Session DB owns in one sentence.
- [ ] 2. I know which process or crate owns Session DB behavior.
- [ ] 3. I checked the source references listed above before changing behavior.
- [ ] 4. I used a command, test, API route, or stored record as evidence.
- [ ] 5. I did not store secrets, tokens, or provider credentials in docs or examples.
- [ ] 6. I did not make the GUI or TUI duplicate backend runtime behavior.
- [ ] 7. I kept deterministic tests separate from live-provider checks.
- [ ] 8. I updated cross-links when adding or moving a related page.
- [ ] 9. I preserved old Markdown files outside this new doc tree.
- [ ] 10. I can point to the exact function name and source file for the main behavior.
- [ ] 11. Session DB documentation still matches the current implementation and examples.
- [ ] 12. Session DB documentation still matches the current implementation and examples.
- [ ] 13. Session DB documentation still matches the current implementation and examples.
- [ ] 14. Session DB documentation still matches the current implementation and examples.
- [ ] 15. Session DB documentation still matches the current implementation and examples.
- [ ] 16. Session DB documentation still matches the current implementation and examples.
- [ ] 17. Session DB documentation still matches the current implementation and examples.
- [ ] 18. Session DB documentation still matches the current implementation and examples.
- [ ] 19. Session DB documentation still matches the current implementation and examples.
- [ ] 20. Session DB documentation still matches the current implementation and examples.
- [ ] 21. Session DB documentation still matches the current implementation and examples.
- [ ] 22. Session DB documentation still matches the current implementation and examples.
- [ ] 23. Session DB documentation still matches the current implementation and examples.
- [ ] 24. Session DB documentation still matches the current implementation and examples.
- [ ] 25. Session DB documentation still matches the current implementation and examples.
- [ ] 26. Session DB documentation still matches the current implementation and examples.
- [ ] 27. Session DB documentation still matches the current implementation and examples.
- [ ] 28. Session DB documentation still matches the current implementation and examples.
- [ ] 29. Session DB documentation still matches the current implementation and examples.
- [ ] 30. Session DB documentation still matches the current implementation and examples.

## See also

- [Sessions](../start/sessions.md)
- [Gateway](../architecture/gateway.md)
- [Router](../architecture/router.md)
- [Context Management](../core/context-management.md)

## Maintenance notes

- Maintenance note 1: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 2: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 3: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 4: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 5: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 6: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 7: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 8: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 9: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 10: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 11: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 12: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 13: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 14: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 15: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 16: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 17: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 18: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 19: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 20: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 21: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 22: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 23: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 24: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 25: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 26: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 27: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 28: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 29: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 30: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 31: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 32: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 33: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 34: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 35: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 36: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 37: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 38: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 39: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 40: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 41: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 42: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 43: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 44: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
- Maintenance note 45: keep `architecture/session-db.md` aligned with crates/session_log/src/store.rs, crates/session_log/src/service.rs, crates/session_log/src/client.rs.
