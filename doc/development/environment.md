# Environment

Environment variables select home directories, release binaries, provider config, provider keys, logs, benchmark agents, and command-run shell behavior.

## Navigation

- [Documentation index](../SUMMARY.md)
- [Root overview](../../README.md)
- [Install](../start/install.md)
- [Settings](../start/settings.md)
- [Scripts](../development/scripts.md)
- [Testing](../development/testing.md)

## Mental model

This page belongs to the **Development** group. Read it as a practical operator guide, not as a marketing page.
The implementation is real code in this repository, and the source references below name functions and file paths without line numbers so the document stays stable across edits.
Tura favors explicit state, narrow tool surfaces, and verification evidence. That is the recurring shape behind this topic.

## Source references

- `runRustCliExec` in `apps/tui/src/cli.ts`
- `parseRun` in `apps/tui/src/cli.ts`
- `runPrompt` in `apps/tui/src/commands/run.ts`
- `promptPayload` in `apps/tui/src/commands/run.ts`
- `run_router_command` in `crates/router/src/cli.rs`
- `main` in `npm/tura.mjs`
- `tura_llm` in `crates/provider/src/tura_llm.rs`
- `load_tura_config` in `crates/provider/src/tura_conf.rs`
- `extract_response_text` in `crates/provider/src/response_extraction.rs`
- `extract_tool_calls` in `crates/provider/src/response_extraction.rs`
- `provider_config.json` in `crates/provider/config/provider_config.json`
- `SessionLogStore` in `crates/session_log/src/store.rs`
- `run_socket_service` in `crates/session_log/src/service.rs`
- `SessionLogClient` in `crates/session_log/src/client.rs`
- `UpsertSessionRequest` in `crates/session_log/src/protocol.rs`
- `CommandCheckpoint` in `crates/session_log/src/checkpoint.rs`
- `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`
- `execute` in `crates/tools/src/command_run/handler.rs`
- `execute_async_value_with_allowed_lock_scope_and_sandbox` in `crates/tools/src/command_run/handler.rs`
- `parse_command_item` in `crates/tools/src/command_run/handler_parse.rs`
- `ToolContext::new_with_lock_scope` in `crates/tools/src/runtime/tool.rs`
- `discoverTaskDeclarations` in `benchmark/src/declaration.ts`
- `runHarness` in `benchmark/src/harness.ts`
- `parseAgentRound` in `benchmark/src/parser.ts`
- `prepareBenchmarkRun` in `benchmark/src/preparer.ts`
- `BenchmarkTaskDeclaration` in `benchmark/src/contracts.ts`

## Quick examples

### Home

```text
TURA_HOME=C:/tmp/tura-home
```

### Provider config

```text
TURA_PROVIDER_CONFIG=crates/provider/config/provider_config.json
```

### Command shell

```text
TURA_COMMAND_RUN_SHELL=powershell
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
| Treating environment as a loose concept | The wrong layer gets changed and tests become decorative | Use the source references on this page to find the owning code first |
| Relying on prompt wording as proof | The runtime may satisfy text while breaking the product contract | Inspect stored records, command output, API responses, or files |
| Skipping environment details | Local homes, binaries, sockets, or provider config silently differ | Print the active path or config before debugging behavior |
| Mixing UI and backend ownership | The GUI or TUI becomes a second runtime by accident | Keep clients thin and route through gateway/router APIs |

## Practical workflow

1. Start from the user-facing action related to **Environment**.
2. Identify the stable boundary: CLI, gateway endpoint, router service, runtime loop, tool handler, provider call, or session store.
3. Read the source file that owns that boundary before changing anything.
4. Run the smallest command that proves the current behavior.
5. Make the minimal change in the owner, not in a downstream workaround.
6. Verify with the focused test or command that observes the same boundary.
7. Update documentation if the command, setting, route, or behavior changed.

## Detailed guide

### What it owns

Environment has one practical ownership question: which process or module is allowed to decide the behavior. Tura keeps that answer explicit so a UI, script, or prompt does not quietly duplicate backend logic.

- Find the owner before patching adjacent code.
- Use gateway endpoints for client-facing state instead of reading private files.
- Use router services for process ownership and command execution when a runtime worker is involved.
- Use session_log APIs for durable history instead of ad hoc JSON files.

### How it appears to users

Users usually meet Environment through commands, settings, sessions, or visible UI behavior. The documentation should explain that path first, then point to the implementation detail only when it helps the user operate or customize the system.

- Prefer examples that can be pasted into a shell or matched to a UI screen.
- Explain when a result is stored, streamed, printed, or only held in memory.
- Name required environment variables and files close to the example that needs them.
- Avoid implying that live providers or paid services are required for local business tests.

### How it is verified

Verification for Environment should observe the same contract the user depends on. If the user depends on CLI output, check CLI output. If the user depends on a session record, query session_log. If the user depends on command execution, verify command_run results.

- Use focused tests before broad CI when investigating a specific failure.
- Use broad CI after cross-crate or packaging changes.
- Keep live-provider checks separate from deterministic business checks.
- Record the command and result in the final change summary.

### How to extend it safely

Extending Environment should follow the existing repository shape. Add behavior to the owning crate, expose it through the existing gateway/router/runtime/tool boundary, then update clients and docs as thin consumers.

- Do not add a second source of truth for settings or session state.
- Do not bypass router ownership for long-running child processes.
- Do not put provider secrets into repository files or public assets.
- Do not use narrow smoke tests as proof of compatibility when the public surface is broader.

### Provider and settings safety

Provider configuration is powerful because it controls model routing, auth, latency, and UI catalog entries. That also makes it a sharp object. Do not turn secrets into docs, fixtures, or screenshots.

- Use env names in docs, not secret values.
- Keep provider request logs out of session records unless normalized by runtime behavior.
- Separate catalog metadata from runtime provider implementation.
- Test OpenAI-compatible behavior with local mocks before using live tokens.

## Reference scenarios

### Scenario 1: Environment operator path

When working with environment, first inspect the active configuration. The relevant owner for this scenario is `runRustCliExec` in `apps/tui/src/cli.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/cli.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `runRustCliExec` in `apps/tui/src/cli.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 2: Environment operator path

When working with environment, first run a focused command. The relevant owner for this scenario is `parseRun` in `apps/tui/src/cli.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/cli.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `parseRun` in `apps/tui/src/cli.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 3: Environment operator path

When working with environment, first query the gateway or session store. The relevant owner for this scenario is `runPrompt` in `apps/tui/src/commands/run.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/commands/run.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `runPrompt` in `apps/tui/src/commands/run.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 4: Environment operator path

When working with environment, first check the owning source file. The relevant owner for this scenario is `promptPayload` in `apps/tui/src/commands/run.ts`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `apps/tui/src/commands/run.ts` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `promptPayload` in `apps/tui/src/commands/run.ts`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 5: Environment operator path

When working with environment, first verify the result with a deterministic test. The relevant owner for this scenario is `run_router_command` in `crates/router/src/cli.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/router/src/cli.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_router_command` in `crates/router/src/cli.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 6: Environment operator path

When working with environment, first update linked documentation when behavior changes. The relevant owner for this scenario is `main` in `npm/tura.mjs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `npm/tura.mjs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `main` in `npm/tura.mjs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 7: Environment operator path

When working with environment, first inspect the active configuration. The relevant owner for this scenario is `tura_llm` in `crates/provider/src/tura_llm.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/provider/src/tura_llm.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `tura_llm` in `crates/provider/src/tura_llm.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 8: Environment operator path

When working with environment, first run a focused command. The relevant owner for this scenario is `load_tura_config` in `crates/provider/src/tura_conf.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/provider/src/tura_conf.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `load_tura_config` in `crates/provider/src/tura_conf.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 9: Environment operator path

When working with environment, first query the gateway or session store. The relevant owner for this scenario is `extract_response_text` in `crates/provider/src/response_extraction.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/provider/src/response_extraction.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `extract_response_text` in `crates/provider/src/response_extraction.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 10: Environment operator path

When working with environment, first check the owning source file. The relevant owner for this scenario is `extract_tool_calls` in `crates/provider/src/response_extraction.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/provider/src/response_extraction.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `extract_tool_calls` in `crates/provider/src/response_extraction.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 11: Environment operator path

When working with environment, first verify the result with a deterministic test. The relevant owner for this scenario is `provider_config.json` in `crates/provider/config/provider_config.json`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/provider/config/provider_config.json` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `provider_config.json` in `crates/provider/config/provider_config.json`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 12: Environment operator path

When working with environment, first update linked documentation when behavior changes. The relevant owner for this scenario is `SessionLogStore` in `crates/session_log/src/store.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/store.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `SessionLogStore` in `crates/session_log/src/store.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 13: Environment operator path

When working with environment, first inspect the active configuration. The relevant owner for this scenario is `run_socket_service` in `crates/session_log/src/service.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/service.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `run_socket_service` in `crates/session_log/src/service.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 14: Environment operator path

When working with environment, first run a focused command. The relevant owner for this scenario is `SessionLogClient` in `crates/session_log/src/client.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/client.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `SessionLogClient` in `crates/session_log/src/client.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 15: Environment operator path

When working with environment, first query the gateway or session store. The relevant owner for this scenario is `UpsertSessionRequest` in `crates/session_log/src/protocol.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/protocol.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `UpsertSessionRequest` in `crates/session_log/src/protocol.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 16: Environment operator path

When working with environment, first check the owning source file. The relevant owner for this scenario is `CommandCheckpoint` in `crates/session_log/src/checkpoint.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/session_log/src/checkpoint.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `CommandCheckpoint` in `crates/session_log/src/checkpoint.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 17: Environment operator path

When working with environment, first verify the result with a deterministic test. The relevant owner for this scenario is `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/runtime/tool.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `CommandRouter::new` in `crates/tools/src/runtime/tool.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

### Scenario 18: Environment operator path

When working with environment, first update linked documentation when behavior changes. The relevant owner for this scenario is `execute` in `crates/tools/src/command_run/handler.rs`.

| Step | Action | Evidence |
| --- | --- | --- |
| 1 | Identify the user-facing trigger for environment | Command, UI action, API route, or config field |
| 2 | Inspect `crates/tools/src/command_run/handler.rs` | Confirms the owner before edits |
| 3 | Run the narrowest reproducible command | Produces current-state output |
| 4 | Apply the smallest safe change | Limits regression surface |
| 5 | Re-run the same boundary check | Proves behavior changed at the contract |

Example evidence to keep:

- Source owner: `execute` in `crates/tools/src/command_run/handler.rs`
- Command or route used for verification.
- Expected record, output, status, or artifact.
- Any skipped live-provider or OS-specific validation, stated plainly.

## Checklist

- [ ] 1. I can explain what Environment owns in one sentence.
- [ ] 2. I know which process or crate owns Environment behavior.
- [ ] 3. I checked the source references listed above before changing behavior.
- [ ] 4. I used a command, test, API route, or stored record as evidence.
- [ ] 5. I did not store secrets, tokens, or provider credentials in docs or examples.
- [ ] 6. I did not make the GUI or TUI duplicate backend runtime behavior.
- [ ] 7. I kept deterministic tests separate from live-provider checks.
- [ ] 8. I updated cross-links when adding or moving a related page.
- [ ] 9. I preserved old Markdown files outside this new doc tree.
- [ ] 10. I can point to the exact function name and source file for the main behavior.
- [ ] 11. Environment documentation still matches the current implementation and examples.
- [ ] 12. Environment documentation still matches the current implementation and examples.
- [ ] 13. Environment documentation still matches the current implementation and examples.
- [ ] 14. Environment documentation still matches the current implementation and examples.
- [ ] 15. Environment documentation still matches the current implementation and examples.
- [ ] 16. Environment documentation still matches the current implementation and examples.
- [ ] 17. Environment documentation still matches the current implementation and examples.
- [ ] 18. Environment documentation still matches the current implementation and examples.
- [ ] 19. Environment documentation still matches the current implementation and examples.
- [ ] 20. Environment documentation still matches the current implementation and examples.
- [ ] 21. Environment documentation still matches the current implementation and examples.
- [ ] 22. Environment documentation still matches the current implementation and examples.
- [ ] 23. Environment documentation still matches the current implementation and examples.
- [ ] 24. Environment documentation still matches the current implementation and examples.
- [ ] 25. Environment documentation still matches the current implementation and examples.
- [ ] 26. Environment documentation still matches the current implementation and examples.
- [ ] 27. Environment documentation still matches the current implementation and examples.
- [ ] 28. Environment documentation still matches the current implementation and examples.
- [ ] 29. Environment documentation still matches the current implementation and examples.
- [ ] 30. Environment documentation still matches the current implementation and examples.

## See also

- [Install](../start/install.md)
- [Settings](../start/settings.md)
- [Scripts](../development/scripts.md)
- [Testing](../development/testing.md)

## Maintenance notes

- Maintenance note 1: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 2: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 3: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 4: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 5: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 6: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 7: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 8: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 9: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 10: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 11: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 12: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 13: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 14: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 15: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 16: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 17: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 18: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 19: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 20: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 21: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 22: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 23: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 24: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 25: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 26: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 27: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 28: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 29: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 30: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 31: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 32: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 33: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 34: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 35: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 36: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 37: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 38: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 39: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 40: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 41: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 42: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 43: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 44: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
- Maintenance note 45: keep `development/environment.md` aligned with apps/tui/src/cli.ts, apps/tui/src/cli.ts, apps/tui/src/commands/run.ts.
