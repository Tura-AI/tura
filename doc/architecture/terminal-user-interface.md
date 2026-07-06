# Terminal User Interface

The TypeScript TUI is a thin terminal client that talks to gateway APIs for sessions, prompts, providers, agents, personas, config, and streaming events.

## Navigation

- [Documentation index](../SUMMARY.md)
- [Root overview](../../README.md)
- [CLI Parameters](../start/cli-parameters.md)
- [Gateway](../architecture/gateway.md)
- [Graphic User Interface](../architecture/graphic-user-interface.md)
- [Settings](../start/settings.md)

## Mental model

This page belongs to the **Architecture** group. Read it as a practical operator guide, not as a marketing page.
The implementation is real code in this repository, and the source references below name functions and file paths without line numbers so the document stays stable across edits.
Tura favors explicit state, narrow tool surfaces, and verification evidence. That is the recurring shape behind this topic.

## Source references

- `runCli` in `apps/tui/src/cli.ts`
- `runPrompt` in `apps/tui/src/commands/run.ts`
- `sessionCommand` in `apps/tui/src/commands/session.ts`
- `providerCommand` in `apps/tui/src/commands/provider.ts`
- `gatewayCommand` in `apps/tui/src/commands/gateway.ts`
- `build_router` in `crates/gateway/src/web/server.rs`
- `run_server` in `crates/gateway/src/web/server.rs`
- `run_server_until_shutdown` in `crates/gateway/src/web/server.rs`
- `local_bind_addr` in `crates/gateway/src/web/server.rs`
- `session_store` in `crates/gateway/src/session/mod.rs`
- `runRustCliExec` in `apps/tui/src/cli.ts`
- `parseRun` in `apps/tui/src/cli.ts`
- `runPrompt` in `apps/tui/src/commands/run.ts`
- `promptPayload` in `apps/tui/src/commands/run.ts`
- `run_router_command` in `crates/router/src/cli.rs`
- `main` in `npm/tura.mjs`

## Quick examples

### Interactive

```text
tura
```

### Run prompt

```text
tura run "Explain this failing test"
```

### Provider command

```text
tura provider list --json
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
| Treating terminal user interface as a loose concept | The wrong layer gets changed and tests become decorative | Use the source references on this page to find the owning code first |
| Relying on prompt wording as proof | The runtime may satisfy text while breaking the product contract | Inspect stored records, command output, API responses, or files |
| Skipping environment details | Local homes, binaries, sockets, or provider config silently differ | Print the active path or config before debugging behavior |
| Mixing UI and backend ownership | The GUI or TUI becomes a second runtime by accident | Keep clients thin and route through gateway/router APIs |

## Practical workflow

1. Start from the user-facing action related to **Terminal User Interface**.
2. Identify the stable boundary: CLI, gateway endpoint, router service, runtime loop, tool handler, provider call, or session store.
3. Read the source file that owns that boundary before changing anything.
4. Run the smallest command that proves the current behavior.
5. Make the minimal change in the owner, not in a downstream workaround.
6. Verify with the focused test or command that observes the same boundary.
7. Update documentation if the command, setting, route, or behavior changed.

## Detailed guide

### What it owns

Terminal User Interface has one practical ownership question: which process or module is allowed to decide the behavior. Tura keeps that answer explicit so a UI, script, or prompt does not quietly duplicate backend logic.

- Find the owner before patching adjacent code.
- Use gateway endpoints for client-facing state instead of reading private files.
- Use router services for process ownership and command execution when a runtime worker is involved.
- Use session_log APIs for durable history instead of ad hoc JSON files.

### How it appears to users

Users usually meet Terminal User Interface through commands, settings, sessions, or visible UI behavior. The documentation should explain that path first, then point to the implementation detail only when it helps the user operate or customize the system.

- Prefer examples that can be pasted into a shell or matched to a UI screen.
- Explain when a result is stored, streamed, printed, or only held in memory.
- Name required environment variables and files close to the example that needs them.
- Avoid implying that live providers or paid services are required for local business tests.

### How it is verified

Verification for Terminal User Interface should observe the same contract the user depends on. If the user depends on CLI output, check CLI output. If the user depends on a session record, query session_log. If the user depends on command execution, verify command_run results.

- Use focused tests before broad CI when investigating a specific failure.
- Use broad CI after cross-crate or packaging changes.
- Keep live-provider checks separate from deterministic business checks.
- Record the command and result in the final change summary.

### How to extend it safely

Extending Terminal User Interface should follow the existing repository shape. Add behavior to the owning crate, expose it through the existing gateway/router/runtime/tool boundary, then update clients and docs as thin consumers.

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

### Scenario 1: Terminal User Interface operator path

When working with terminal user interface, first inspect the active configuration. The relevant owner for this scenario is `runCli` in `apps/tui/src/cli.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/cli.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `runCli` in `apps/tui/src/cli.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 2: Terminal User Interface operator path

When working with terminal user interface, first run a focused command. The relevant owner for this scenario is `runPrompt` in `apps/tui/src/commands/run.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/commands/run.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `runPrompt` in `apps/tui/src/commands/run.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 3: Terminal User Interface operator path

When working with terminal user interface, first query the gateway or session store. The relevant owner for this scenario is `sessionCommand` in `apps/tui/src/commands/session.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/commands/session.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `sessionCommand` in `apps/tui/src/commands/session.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 4: Terminal User Interface operator path

When working with terminal user interface, first check the owning source file. The relevant owner for this scenario is `providerCommand` in `apps/tui/src/commands/provider.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/commands/provider.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `providerCommand` in `apps/tui/src/commands/provider.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 5: Terminal User Interface operator path

When working with terminal user interface, first verify the result with a deterministic test. The relevant owner for this scenario is `gatewayCommand` in `apps/tui/src/commands/gateway.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/commands/gateway.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `gatewayCommand` in `apps/tui/src/commands/gateway.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 6: Terminal User Interface operator path

When working with terminal user interface, first update linked documentation when behavior changes. The relevant owner for this scenario is `build_router` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `build_router` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 7: Terminal User Interface operator path

When working with terminal user interface, first inspect the active configuration. The relevant owner for this scenario is `run_server` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_server` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 8: Terminal User Interface operator path

When working with terminal user interface, first run a focused command. The relevant owner for this scenario is `run_server_until_shutdown` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_server_until_shutdown` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 9: Terminal User Interface operator path

When working with terminal user interface, first query the gateway or session store. The relevant owner for this scenario is `local_bind_addr` in `crates/gateway/src/web/server.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/web/server.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `local_bind_addr` in `crates/gateway/src/web/server.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 10: Terminal User Interface operator path

When working with terminal user interface, first check the owning source file. The relevant owner for this scenario is `session_store` in `crates/gateway/src/session/mod.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `crates/gateway/src/session/mod.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `session_store` in `crates/gateway/src/session/mod.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 11: Terminal User Interface operator path

When working with terminal user interface, first verify the result with a deterministic test. The relevant owner for this scenario is `runRustCliExec` in `apps/tui/src/cli.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/cli.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `runRustCliExec` in `apps/tui/src/cli.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 12: Terminal User Interface operator path

When working with terminal user interface, first update linked documentation when behavior changes. The relevant owner for this scenario is `parseRun` in `apps/tui/src/cli.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/cli.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `parseRun` in `apps/tui/src/cli.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 13: Terminal User Interface operator path

When working with terminal user interface, first inspect the active configuration. The relevant owner for this scenario is `runPrompt` in `apps/tui/src/commands/run.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/commands/run.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `runPrompt` in `apps/tui/src/commands/run.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 14: Terminal User Interface operator path

When working with terminal user interface, first run a focused command. The relevant owner for this scenario is `promptPayload` in `apps/tui/src/commands/run.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/commands/run.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `promptPayload` in `apps/tui/src/commands/run.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 15: Terminal User Interface operator path

When working with terminal user interface, first query the gateway or session store. The relevant owner for this scenario is `run_router_command` in `crates/router/src/cli.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/cli.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_router_command` in `crates/router/src/cli.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 16: Terminal User Interface operator path

When working with terminal user interface, first check the owning source file. The relevant owner for this scenario is `main` in `npm/tura.mjs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `npm/tura.mjs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `main` in `npm/tura.mjs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 17: Terminal User Interface operator path

When working with terminal user interface, first verify the result with a deterministic test. The relevant owner for this scenario is `runCli` in `apps/tui/src/cli.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/cli.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `runCli` in `apps/tui/src/cli.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 18: Terminal User Interface operator path

When working with terminal user interface, first update linked documentation when behavior changes. The relevant owner for this scenario is `runPrompt` in `apps/tui/src/commands/run.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for terminal user interface | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/commands/run.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `runPrompt` in `apps/tui/src/commands/run.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

## Checklist

- [ ] 1. I can explain what Terminal User Interface owns in one sentence.
- [ ] 2. I know which process or crate owns Terminal User Interface behavior.
- [ ] 3. I checked the source references listed above before changing behavior.
- [ ] 4. I used a command, test, API route, or stored record as evidence.
- [ ] 5. I did not store secrets, tokens, or provider credentials in docs or examples.
- [ ] 6. I did not make the GUI or TUI duplicate backend runtime behavior.
- [ ] 7. I kept deterministic tests separate from live-provider checks.
- [ ] 8. I updated cross-links when adding or moving a related page.
- [ ] 9. I preserved old Markdown files outside this new doc tree.
- [ ] 10. I can point to the exact function name and source file for the main behavior.
- [ ] 11. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 12. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 13. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 14. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 15. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 16. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 17. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 18. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 19. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 20. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 21. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 22. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 23. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 24. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 25. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 26. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 27. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 28. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 29. Terminal User Interface documentation still matches the current implementation and examples.
- [ ] 30. Terminal User Interface documentation still matches the current implementation and examples.

## See also

- [CLI Parameters](../start/cli-parameters.md)
- [Gateway](../architecture/gateway.md)
- [Graphic User Interface](../architecture/graphic-user-interface.md)
- [Settings](../start/settings.md)

## Maintenance notes

- Maintenance note 1: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 2: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 3: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 4: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 5: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 6: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 7: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 8: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 9: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 10: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 11: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 12: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 13: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 14: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 15: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 16: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 17: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 18: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 19: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 20: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 21: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 22: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 23: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 24: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 25: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 26: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 27: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 28: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 29: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 30: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 31: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 32: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 33: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 34: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 35: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 36: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 37: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 38: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 39: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 40: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 41: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 42: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 43: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 44: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
- Maintenance note 45: keep `architecture/terminal-user-interface.md` aligned with apps/tui/src/cli.ts, apps/tui/src/commands/run.ts, apps/tui/src/commands/session.ts.
