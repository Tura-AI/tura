# Command Run

command_run is the compact macro tool surface for batching shell commands, patches, media/web commands, and task-state updates into ordered steps.

## Navigation

- [Documentation index](../SUMMARY.md)
- [Root overview](../../README.md)
- [Commands](../core/commands.md)
- [Task Status](../core/task-status.md)
- [Tool](../architecture/tool.md)
- [Runtime](../architecture/runtime.md)

## Mental model

This page belongs to the **Core** group. Read it as a practical operator guide, not as a marketing page.
The implementation is real code in this repository, and the source references below name functions and file paths without line numbers so the document stays stable across edits.
Tura favors explicit state, narrow tool surfaces, and verification evidence. That is the recurring shape behind this topic.

## Source references

- `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`
- `execute` in `crates/tools/src/command_run/handler.rs`
- `execute_async_value_with_allowed_lock_scope_and_sandbox` in `crates/tools/src/command_run/handler.rs`
- `parse_command_item` in `crates/tools/src/command_run/handler_parse.rs`
- `ToolContext::new_with_lock_scope` in `crates/tools/src/runtime/tool.rs`
- `process_from_user` in `crates/runtime/src/mano/process.rs`
- `process_from_session` in `crates/runtime/src/manas/process.rs`
- `run_session` in `crates/runtime/src/manas/runtime_turn.rs`
- `extract_compact_context_results` in `crates/runtime/src/turn_loop/tool_step.rs`
- `append_missing_runtime_prompt_manuals` in `crates/runtime/src/prompt_style/runtime_prompt_manual.rs`
- `run_router_command` in `crates/router/src/cli.rs`
- `serve_socket` in `crates/router/src/daemon.rs`
- `serve_stdio` in `crates/router/src/daemon.rs`
- `dispatch_run_agent` in `crates/router/src/runtime_dispatch.rs`
- `CommandRunService::execute` in `crates/router/src/services/command_run.rs`

## Quick examples

### Parallel reads

```text
step 1: rg --files and rg -n searches share the same step.
```

### Patch then test

```text
step 1: apply_patch, step 2: cargo test -p tools command_run.
```

### Router-owned process

```text
CommandRunService::execute runs tool work under the router.
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
| Treating command run as a loose concept | The wrong layer gets changed and tests become decorative | Use the source references on this page to find the owning code first |
| Relying on prompt wording as proof | The runtime may satisfy text while breaking the product contract | Inspect stored records, command output, API responses, or files |
| Skipping environment details | Local homes, binaries, sockets, or provider config silently differ | Print the active path or config before debugging behavior |
| Mixing UI and backend ownership | The GUI or TUI becomes a second runtime by accident | Keep clients thin and route through gateway/router APIs |

## Practical workflow

1. Start from the user-facing action related to **Command Run**.
2. Identify the stable boundary: CLI, gateway endpoint, router service, runtime loop, tool handler, provider call, or session store.
3. Read the source file that owns that boundary before changing anything.
4. Run the smallest command that proves the current behavior.
5. Make the minimal change in the owner, not in a downstream workaround.
6. Verify with the focused test or command that observes the same boundary.
7. Update documentation if the command, setting, route, or behavior changed.

## Detailed guide

### What it owns

Command Run has one practical ownership question: which process or module is allowed to decide the behavior. Tura keeps that answer explicit so a UI, script, or prompt does not quietly duplicate backend logic.

- Find the owner before patching adjacent code.
- Use gateway endpoints for client-facing state instead of reading private files.
- Use router services for process ownership and command execution when a runtime worker is involved.
- Use session_log APIs for durable history instead of ad hoc JSON files.

### How it appears to users

Users usually meet Command Run through commands, settings, sessions, or visible UI behavior. The documentation should explain that path first, then point to the implementation detail only when it helps the user operate or customize the system.

- Prefer examples that can be pasted into a shell or matched to a UI screen.
- Explain when a result is stored, streamed, printed, or only held in memory.
- Name required environment variables and files close to the example that needs them.
- Avoid implying that live providers or paid services are required for local business tests.

### How it is verified

Verification for Command Run should observe the same contract the user depends on. If the user depends on CLI output, check CLI output. If the user depends on a session record, query session_log. If the user depends on command execution, verify command_run results.

- Use focused tests before broad CI when investigating a specific failure.
- Use broad CI after cross-crate or packaging changes.
- Keep live-provider checks separate from deterministic business checks.
- Record the command and result in the final change summary.

### How to extend it safely

Extending Command Run should follow the existing repository shape. Add behavior to the owning crate, expose it through the existing gateway/router/runtime/tool boundary, then update clients and docs as thin consumers.

- Do not add a second source of truth for settings or session state.
- Do not bypass router ownership for long-running child processes.
- Do not put provider secrets into repository files or public assets.
- Do not use narrow smoke tests as proof of compatibility when the public surface is broader.

### Command-run discipline

The command surface is intentionally compact. A batch can contain multiple commands, but the step numbers must represent real dependency order rather than narration order.

- Independent reads share a step.
- Edits happen after investigation.
- Verification happens after edits.
- A failed apply_patch cancels later commands so a broken edit is not hidden by noisy follow-up output.

## Reference scenarios

### Scenario 1: Command Run operator path

When working with command run, first inspect the active configuration. The relevant owner for this scenario is `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/runtime/tool.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 2: Command Run operator path

When working with command run, first run a focused command. The relevant owner for this scenario is `execute` in `crates/tools/src/command_run/handler.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/command_run/handler.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `execute` in `crates/tools/src/command_run/handler.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 3: Command Run operator path

When working with command run, first query the gateway or session store. The relevant owner for this scenario is `execute_async_value_with_allowed_lock_scope_and_sandbox` in `crates/tools/src/command_run/handler.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/command_run/handler.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `execute_async_value_with_allowed_lock_scope_and_sandbox` in `crates/tools/src/command_run/handler.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 4: Command Run operator path

When working with command run, first check the owning source file. The relevant owner for this scenario is `parse_command_item` in `crates/tools/src/command_run/handler_parse.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/command_run/handler_parse.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `parse_command_item` in `crates/tools/src/command_run/handler_parse.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 5: Command Run operator path

When working with command run, first verify the result with a deterministic test. The relevant owner for this scenario is `ToolContext::new_with_lock_scope` in `crates/tools/src/runtime/tool.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/runtime/tool.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `ToolContext::new_with_lock_scope` in `crates/tools/src/runtime/tool.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 6: Command Run operator path

When working with command run, first update linked documentation when behavior changes. The relevant owner for this scenario is `process_from_user` in `crates/runtime/src/mano/process.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/runtime/src/mano/process.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `process_from_user` in `crates/runtime/src/mano/process.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 7: Command Run operator path

When working with command run, first inspect the active configuration. The relevant owner for this scenario is `process_from_session` in `crates/runtime/src/manas/process.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/runtime/src/manas/process.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `process_from_session` in `crates/runtime/src/manas/process.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 8: Command Run operator path

When working with command run, first run a focused command. The relevant owner for this scenario is `run_session` in `crates/runtime/src/manas/runtime_turn.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/runtime/src/manas/runtime_turn.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_session` in `crates/runtime/src/manas/runtime_turn.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 9: Command Run operator path

When working with command run, first query the gateway or session store. The relevant owner for this scenario is `extract_compact_context_results` in `crates/runtime/src/turn_loop/tool_step.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/runtime/src/turn_loop/tool_step.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `extract_compact_context_results` in `crates/runtime/src/turn_loop/tool_step.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 10: Command Run operator path

When working with command run, first check the owning source file. The relevant owner for this scenario is `append_missing_runtime_prompt_manuals` in `crates/runtime/src/prompt_style/runtime_prompt_manual.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/runtime/src/prompt_style/runtime_prompt_manual.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `append_missing_runtime_prompt_manuals` in `crates/runtime/src/prompt_style/runtime_prompt_manual.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 11: Command Run operator path

When working with command run, first verify the result with a deterministic test. The relevant owner for this scenario is `run_router_command` in `crates/router/src/cli.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/cli.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_router_command` in `crates/router/src/cli.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 12: Command Run operator path

When working with command run, first update linked documentation when behavior changes. The relevant owner for this scenario is `serve_socket` in `crates/router/src/daemon.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/daemon.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `serve_socket` in `crates/router/src/daemon.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 13: Command Run operator path

When working with command run, first inspect the active configuration. The relevant owner for this scenario is `serve_stdio` in `crates/router/src/daemon.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/daemon.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `serve_stdio` in `crates/router/src/daemon.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 14: Command Run operator path

When working with command run, first run a focused command. The relevant owner for this scenario is `dispatch_run_agent` in `crates/router/src/runtime_dispatch.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/runtime_dispatch.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `dispatch_run_agent` in `crates/router/src/runtime_dispatch.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 15: Command Run operator path

When working with command run, first query the gateway or session store. The relevant owner for this scenario is `CommandRunService::execute` in `crates/router/src/services/command_run.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/services/command_run.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `CommandRunService::execute` in `crates/router/src/services/command_run.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 16: Command Run operator path

When working with command run, first check the owning source file. The relevant owner for this scenario is `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/runtime/tool.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 17: Command Run operator path

When working with command run, first verify the result with a deterministic test. The relevant owner for this scenario is `execute` in `crates/tools/src/command_run/handler.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/command_run/handler.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `execute` in `crates/tools/src/command_run/handler.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 18: Command Run operator path

When working with command run, first update linked documentation when behavior changes. The relevant owner for this scenario is `execute_async_value_with_allowed_lock_scope_and_sandbox` in `crates/tools/src/command_run/handler.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for command run | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/command_run/handler.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `execute_async_value_with_allowed_lock_scope_and_sandbox` in `crates/tools/src/command_run/handler.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

## Checklist

- [ ] 1. I can explain what Command Run owns in one sentence.
- [ ] 2. I know which process or crate owns Command Run behavior.
- [ ] 3. I checked the source references listed above before changing behavior.
- [ ] 4. I used a command, test, API route, or stored record as evidence.
- [ ] 5. I did not store secrets, tokens, or provider credentials in docs or examples.
- [ ] 6. I did not make the GUI or TUI duplicate backend runtime behavior.
- [ ] 7. I kept deterministic tests separate from live-provider checks.
- [ ] 8. I updated cross-links when adding or moving a related page.
- [ ] 9. I preserved old Markdown files outside this new doc tree.
- [ ] 10. I can point to the exact function name and source file for the main behavior.
- [ ] 11. Command Run documentation still matches the current implementation and examples.
- [ ] 12. Command Run documentation still matches the current implementation and examples.
- [ ] 13. Command Run documentation still matches the current implementation and examples.
- [ ] 14. Command Run documentation still matches the current implementation and examples.
- [ ] 15. Command Run documentation still matches the current implementation and examples.
- [ ] 16. Command Run documentation still matches the current implementation and examples.
- [ ] 17. Command Run documentation still matches the current implementation and examples.
- [ ] 18. Command Run documentation still matches the current implementation and examples.
- [ ] 19. Command Run documentation still matches the current implementation and examples.
- [ ] 20. Command Run documentation still matches the current implementation and examples.
- [ ] 21. Command Run documentation still matches the current implementation and examples.
- [ ] 22. Command Run documentation still matches the current implementation and examples.
- [ ] 23. Command Run documentation still matches the current implementation and examples.
- [ ] 24. Command Run documentation still matches the current implementation and examples.
- [ ] 25. Command Run documentation still matches the current implementation and examples.
- [ ] 26. Command Run documentation still matches the current implementation and examples.
- [ ] 27. Command Run documentation still matches the current implementation and examples.
- [ ] 28. Command Run documentation still matches the current implementation and examples.
- [ ] 29. Command Run documentation still matches the current implementation and examples.
- [ ] 30. Command Run documentation still matches the current implementation and examples.

## See also

- [Commands](../core/commands.md)
- [Task Status](../core/task-status.md)
- [Tool](../architecture/tool.md)
- [Runtime](../architecture/runtime.md)

## Maintenance notes

- Maintenance note 1: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 2: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 3: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 4: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 5: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 6: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 7: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 8: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 9: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 10: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 11: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 12: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 13: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 14: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 15: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 16: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 17: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 18: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 19: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 20: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 21: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 22: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 23: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 24: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 25: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 26: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 27: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 28: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 29: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 30: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 31: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 32: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 33: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 34: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 35: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 36: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 37: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 38: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 39: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 40: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 41: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 42: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 43: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 44: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 45: keep `core/command-run.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
