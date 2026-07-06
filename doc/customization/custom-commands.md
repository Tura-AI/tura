# Custom Commands

Custom commands add new tool handlers, schemas, policies, prompts, router metadata, tests, and agent capability exposure.

## Navigation

- [Documentation index](../SUMMARY.md)
- [Root overview](../../README.md)
- [Commands](../core/commands.md)
- [Tool](../architecture/tool.md)
- [Command Run](../core/command-run.md)
- [Testing](../development/testing.md)

## Mental model

This page belongs to the **Customization** group. Read it as a practical operator guide, not as a marketing page.
The implementation is real code in this repository, and the source references below name functions and file paths without line numbers so the document stays stable across edits.
Tura favors explicit state, narrow tool surfaces, and verification evidence. That is the recurring shape behind this topic.

## Source references

- `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`
- `execute` in `crates/tools/src/command_run/handler.rs`
- `execute_async_value_with_allowed_lock_scope_and_sandbox` in `crates/tools/src/command_run/handler.rs`
- `parse_command_item` in `crates/tools/src/command_run/handler_parse.rs`
- `ToolContext::new_with_lock_scope` in `crates/tools/src/runtime/tool.rs`
- `run_router_command` in `crates/router/src/cli.rs`
- `serve_socket` in `crates/router/src/daemon.rs`
- `serve_stdio` in `crates/router/src/daemon.rs`
- `dispatch_run_agent` in `crates/router/src/runtime_dispatch.rs`
- `CommandRunService::execute` in `crates/router/src/services/command_run.rs`
- `discover_agents` in `agents/src/store.rs`
- `load_agent` in `agents/src/store.rs`
- `save_dynamic_agent` in `agents/src/store.rs`
- `delete_dynamic_agent` in `agents/src/store.rs`
- `default_agent_config` in `agents/src/store.rs`

## Quick examples

### Tool module

```text
crates/tools/src/commands/<name>/
```

### External package

```text
commands/<name>/src for command-owned dependencies.
```

### Router registry

```text
Add router metadata only when discovery or lifecycle management is needed.
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
| Treating custom commands as a loose concept | The wrong layer gets changed and tests become decorative | Use the source references on this page to find the owning code first |
| Relying on prompt wording as proof | The runtime may satisfy text while breaking the product contract | Inspect stored records, command output, API responses, or files |
| Skipping environment details | Local homes, binaries, sockets, or provider config silently differ | Print the active path or config before debugging behavior |
| Mixing UI and backend ownership | The GUI or TUI becomes a second runtime by accident | Keep clients thin and route through gateway/router APIs |

## Practical workflow

1. Start from the user-facing action related to **Custom Commands**.
2. Identify the stable boundary: CLI, gateway endpoint, router service, runtime loop, tool handler, provider call, or session store.
3. Read the source file that owns that boundary before changing anything.
4. Run the smallest command that proves the current behavior.
5. Make the minimal change in the owner, not in a downstream workaround.
6. Verify with the focused test or command that observes the same boundary.
7. Update documentation if the command, setting, route, or behavior changed.

## Detailed guide

### What it owns

Custom Commands has one practical ownership question: which process or module is allowed to decide the behavior. Tura keeps that answer explicit so a UI, script, or prompt does not quietly duplicate backend logic.

- Find the owner before patching adjacent code.
- Use gateway endpoints for client-facing state instead of reading private files.
- Use router services for process ownership and command execution when a runtime worker is involved.
- Use session_log APIs for durable history instead of ad hoc JSON files.

### How it appears to users

Users usually meet Custom Commands through commands, settings, sessions, or visible UI behavior. The documentation should explain that path first, then point to the implementation detail only when it helps the user operate or customize the system.

- Prefer examples that can be pasted into a shell or matched to a UI screen.
- Explain when a result is stored, streamed, printed, or only held in memory.
- Name required environment variables and files close to the example that needs them.
- Avoid implying that live providers or paid services are required for local business tests.

### How it is verified

Verification for Custom Commands should observe the same contract the user depends on. If the user depends on CLI output, check CLI output. If the user depends on a session record, query session_log. If the user depends on command execution, verify command_run results.

- Use focused tests before broad CI when investigating a specific failure.
- Use broad CI after cross-crate or packaging changes.
- Keep live-provider checks separate from deterministic business checks.
- Record the command and result in the final change summary.

### How to extend it safely

Extending Custom Commands should follow the existing repository shape. Add behavior to the owning crate, expose it through the existing gateway/router/runtime/tool boundary, then update clients and docs as thin consumers.

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

### Scenario 1: Custom Commands operator path

When working with custom commands, first inspect the active configuration. The relevant owner for this scenario is `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/runtime/tool.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 2: Custom Commands operator path

When working with custom commands, first run a focused command. The relevant owner for this scenario is `execute` in `crates/tools/src/command_run/handler.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/command_run/handler.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `execute` in `crates/tools/src/command_run/handler.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 3: Custom Commands operator path

When working with custom commands, first query the gateway or session store. The relevant owner for this scenario is `execute_async_value_with_allowed_lock_scope_and_sandbox` in `crates/tools/src/command_run/handler.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/command_run/handler.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `execute_async_value_with_allowed_lock_scope_and_sandbox` in `crates/tools/src/command_run/handler.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 4: Custom Commands operator path

When working with custom commands, first check the owning source file. The relevant owner for this scenario is `parse_command_item` in `crates/tools/src/command_run/handler_parse.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/command_run/handler_parse.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `parse_command_item` in `crates/tools/src/command_run/handler_parse.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 5: Custom Commands operator path

When working with custom commands, first verify the result with a deterministic test. The relevant owner for this scenario is `ToolContext::new_with_lock_scope` in `crates/tools/src/runtime/tool.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/runtime/tool.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `ToolContext::new_with_lock_scope` in `crates/tools/src/runtime/tool.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 6: Custom Commands operator path

When working with custom commands, first update linked documentation when behavior changes. The relevant owner for this scenario is `run_router_command` in `crates/router/src/cli.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/cli.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_router_command` in `crates/router/src/cli.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 7: Custom Commands operator path

When working with custom commands, first inspect the active configuration. The relevant owner for this scenario is `serve_socket` in `crates/router/src/daemon.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/daemon.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `serve_socket` in `crates/router/src/daemon.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 8: Custom Commands operator path

When working with custom commands, first run a focused command. The relevant owner for this scenario is `serve_stdio` in `crates/router/src/daemon.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/daemon.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `serve_stdio` in `crates/router/src/daemon.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 9: Custom Commands operator path

When working with custom commands, first query the gateway or session store. The relevant owner for this scenario is `dispatch_run_agent` in `crates/router/src/runtime_dispatch.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/runtime_dispatch.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `dispatch_run_agent` in `crates/router/src/runtime_dispatch.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 10: Custom Commands operator path

When working with custom commands, first check the owning source file. The relevant owner for this scenario is `CommandRunService::execute` in `crates/router/src/services/command_run.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/services/command_run.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `CommandRunService::execute` in `crates/router/src/services/command_run.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 11: Custom Commands operator path

When working with custom commands, first verify the result with a deterministic test. The relevant owner for this scenario is `discover_agents` in `agents/src/store.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `agents/src/store.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `discover_agents` in `agents/src/store.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 12: Custom Commands operator path

When working with custom commands, first update linked documentation when behavior changes. The relevant owner for this scenario is `load_agent` in `agents/src/store.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `agents/src/store.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `load_agent` in `agents/src/store.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 13: Custom Commands operator path

When working with custom commands, first inspect the active configuration. The relevant owner for this scenario is `save_dynamic_agent` in `agents/src/store.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `agents/src/store.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `save_dynamic_agent` in `agents/src/store.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 14: Custom Commands operator path

When working with custom commands, first run a focused command. The relevant owner for this scenario is `delete_dynamic_agent` in `agents/src/store.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `agents/src/store.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `delete_dynamic_agent` in `agents/src/store.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 15: Custom Commands operator path

When working with custom commands, first query the gateway or session store. The relevant owner for this scenario is `default_agent_config` in `agents/src/store.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `agents/src/store.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `default_agent_config` in `agents/src/store.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 16: Custom Commands operator path

When working with custom commands, first check the owning source file. The relevant owner for this scenario is `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/runtime/tool.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 17: Custom Commands operator path

When working with custom commands, first verify the result with a deterministic test. The relevant owner for this scenario is `execute` in `crates/tools/src/command_run/handler.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/command_run/handler.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `execute` in `crates/tools/src/command_run/handler.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 18: Custom Commands operator path

When working with custom commands, first update linked documentation when behavior changes. The relevant owner for this scenario is `execute_async_value_with_allowed_lock_scope_and_sandbox` in `crates/tools/src/command_run/handler.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for custom commands | Command, UI action, API route, or config field |
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

- [ ] 1. I can explain what Custom Commands owns in one sentence.
- [ ] 2. I know which process or crate owns Custom Commands behavior.
- [ ] 3. I checked the source references listed above before changing behavior.
- [ ] 4. I used a command, test, API route, or stored record as evidence.
- [ ] 5. I did not store secrets, tokens, or provider credentials in docs or examples.
- [ ] 6. I did not make the GUI or TUI duplicate backend runtime behavior.
- [ ] 7. I kept deterministic tests separate from live-provider checks.
- [ ] 8. I updated cross-links when adding or moving a related page.
- [ ] 9. I preserved old Markdown files outside this new doc tree.
- [ ] 10. I can point to the exact function name and source file for the main behavior.
- [ ] 11. Custom Commands documentation still matches the current implementation and examples.
- [ ] 12. Custom Commands documentation still matches the current implementation and examples.
- [ ] 13. Custom Commands documentation still matches the current implementation and examples.
- [ ] 14. Custom Commands documentation still matches the current implementation and examples.
- [ ] 15. Custom Commands documentation still matches the current implementation and examples.
- [ ] 16. Custom Commands documentation still matches the current implementation and examples.
- [ ] 17. Custom Commands documentation still matches the current implementation and examples.
- [ ] 18. Custom Commands documentation still matches the current implementation and examples.
- [ ] 19. Custom Commands documentation still matches the current implementation and examples.
- [ ] 20. Custom Commands documentation still matches the current implementation and examples.
- [ ] 21. Custom Commands documentation still matches the current implementation and examples.
- [ ] 22. Custom Commands documentation still matches the current implementation and examples.
- [ ] 23. Custom Commands documentation still matches the current implementation and examples.
- [ ] 24. Custom Commands documentation still matches the current implementation and examples.
- [ ] 25. Custom Commands documentation still matches the current implementation and examples.
- [ ] 26. Custom Commands documentation still matches the current implementation and examples.
- [ ] 27. Custom Commands documentation still matches the current implementation and examples.
- [ ] 28. Custom Commands documentation still matches the current implementation and examples.
- [ ] 29. Custom Commands documentation still matches the current implementation and examples.
- [ ] 30. Custom Commands documentation still matches the current implementation and examples.

## See also

- [Commands](../core/commands.md)
- [Tool](../architecture/tool.md)
- [Command Run](../core/command-run.md)
- [Testing](../development/testing.md)

## Maintenance notes

- Maintenance note 1: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 2: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 3: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 4: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 5: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 6: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 7: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 8: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 9: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 10: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 11: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 12: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 13: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 14: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 15: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 16: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 17: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 18: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 19: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 20: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 21: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 22: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 23: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 24: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 25: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 26: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 27: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 28: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 29: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 30: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 31: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 32: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 33: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 34: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 35: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 36: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 37: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 38: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 39: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 40: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 41: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 42: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 43: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 44: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
- Maintenance note 45: keep `customization/custom-commands.md` aligned with crates/tools/src/runtime/tool.rs, crates/tools/src/command_run/handler.rs, crates/tools/src/command_run/handler.rs.
