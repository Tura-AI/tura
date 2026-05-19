# command_run current-flow robustness matrix

This file is the fixed non-provider coverage map for the current-compatible
`command_run` path. Every row points to a concrete passing local test. Long E2E
runs call `scripts/test-command-run-robustness.ps1 -NoBuild` as a preflight
whenever a Tura agent is requested.

## Run

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-command-run-robustness.ps1
```

## Tool Architecture And Routing

| Current behavior | Tura coverage |
| --- | --- |
| Only `command_run` is exposed to model turns | `default_non_final_turn_keeps_only_command_run`, `planning_mode_still_keeps_only_command_run`, `planning_mode_still_exposes_only_command_run` |
| Agent configuration is capability-driven, not ad-hoc visible tools | `command_run_prompt_loading_excludes_capability_prompts_from_model_context`, `default_non_final_turn_keeps_only_command_run` |
| `command_run` internal commands are rebuilt as normal tool calls | `pass_internal_command_rebuilds_tool_call_and_dispatches_router_handler` |
| Nested commands route through `ToolRouter -> ToolHandler` | `pass_internal_command_rebuilds_tool_call_and_dispatches_router_handler` |
| Unsupported internal commands are model-visible failures | `fail_unsupported_internal_command_returns_model_visible_result` |
| Empty command batches are model-visible failures | `fail_empty_command_run_returns_current_style_failure_result` |
| Tool outputs use current-style `results` without Tura-specific top-level noise | `pass_current_style_command_run_output_shape`, `tool_output_success_follows_current_style_command_run_results` |
| Tool success/error extraction reads current-style result records | `tool_output_success_follows_current_style_command_run_results` |
| Provider schema removes invalid recursive `additionalProperties` | `provider_schema_strips_additional_properties_recursively` |

## Prompt And Surface Isolation

| Current behavior | Tura coverage |
| --- | --- |
| Shell surface exposes exactly one shell command name | `command_run_provider_description_exposes_only_shell_command_surface`, `command_run_provider_description_exposes_only_bash_surface`, `pass_shell_surface_isolation_canonicalizes_to_one_active_shell` |
| Bash surface does not expose `shell_command` wording | `command_run_provider_description_exposes_only_bash_surface` |
| Shell surface does not expose `bash` wording | `command_run_provider_description_exposes_only_shell_command_surface` |
| Internal aliases canonicalize to the active surface only | `pass_shell_surface_isolation_canonicalizes_to_one_active_shell` |
| Capability prompt files for internal commands are not appended to model context | `command_run_prompt_loading_excludes_capability_prompts_from_model_context` |
| Legacy/runtime reporting fields are not leaked into non-command tools | `tool_argument_normalization_removes_runtime_reporting_fields` |
| `command_run` keeps runtime reporting fields needed for current-compatible batching | `command_run_tool_keeps_runtime_reporting_fields` |
| Legacy step payloads normalize into `commands` | `command_run_legacy_steps_are_normalized_to_commands` |

## Step Scheduling, Locks, And Parallelism

| Current behavior | Tura coverage |
| --- | --- |
| Missing step values default to original 1-based order | `pass_missing_steps_default_to_original_order` |
| Same-step read commands may run without write barriers | `pass_mutating_commands_are_barriers_between_read_batches` |
| Mutating commands become barriers inside a step | `pass_mutating_commands_are_barriers_between_read_batches` |
| Handler mutability gates execution through the router | `pass_internal_command_rebuilds_tool_call_and_dispatches_router_handler`, `pass_mutating_commands_are_barriers_between_read_batches` |
| Pre-tool hooks can block execution before runtime starts | `fail_pre_tool_hook_blocks_tool_before_runtime` |
| Post-tool hooks can replace model-visible response | `pass_post_tool_hook_can_replace_model_visible_response` |
| Tool lifecycle emits started/finished records | `pass_post_tool_hook_can_replace_model_visible_response`, `pass_shell_runtime_records_stdout_stderr_delta_events` |

## Shell Runtime, Cancellation, Timeout, Drain

| Current behavior | Tura coverage |
| --- | --- |
| Shell execution is async and does not create a nested Tokio runtime | `pass_async_command_run_entry_does_not_start_nested_runtime` |
| Timeout produces a quick model-visible failure | `pass_timeout_returns_quick_failure` |
| Timeout kills or detaches from long-running process work instead of waiting full duration | `fail_timeout_kills_descendant_process_tree_quickly` |
| Timeout aborts reader drain when descendants hold stdout/stderr pipes | `fail_timeout_aborts_reader_drain_for_pipe_holding_descendants` |
| Turn cancellation propagates into a running shell command | `fail_turn_cancellation_aborts_running_shell_command` |
| Running shell stdout is consumed continuously as delta events | `pass_shell_runtime_records_stdout_stderr_delta_events` |
| Running shell stderr is consumed continuously as delta events | `pass_shell_runtime_records_stdout_stderr_delta_events` |
| PowerShell output is UTF-8-prefixed before execution | `pass_shell_runtime_records_stdout_stderr_delta_events`, `parses_json_shell_request_with_escaped_quotes` |
| POSIX scripts sent through shell surface can be routed to bash on Windows | `detects_posix_shell_scripts_sent_to_shell_command`, `pass_bash_surface_runs_posix_script_without_exposing_shell_command` |
| Windows Git Bash paths are normalized from `/mnt/<drive>` form | `windows_bash_command_normalizes_wsl_mount_paths` |
| Shell request JSON accepts current-style `command`, `cmd`, `workdir`, timeout fields | `parses_json_shell_request_with_escaped_quotes`, `accepts_codex_command_run_cmd_alias`, `parses_json_shell_request_wrapped_as_json_string` |
| Loose/escaped shell JSON from provider streaming is recovered | `parses_escaped_json_shell_request_with_inner_command_quotes`, `parses_loose_json_request_with_raw_multiline_command`, `parses_loose_json_request_with_regex_backslashes` |
| Current-style `command:`/`cmd:`/`shell:`/`bash:` prefixes are stripped | `strips_current_style_shell_text_prefixes`, `strips_current_style_shell_text_prefixes_inside_multiline_scripts` |

## Apply Patch Runtime And Diff Tracking

| Current behavior | Tura coverage |
| --- | --- |
| `apply_patch` succeeds when context matches | `pass_apply_patch_success_and_fail_context_mismatch` |
| `apply_patch` fails when context mismatches | `pass_apply_patch_success_and_fail_context_mismatch` |
| Add file operations are applied and reported | `pass_apply_patch_add_delete_and_move_are_tracked_in_output` |
| Delete file operations are applied and reported | `pass_apply_patch_add_delete_and_move_are_tracked_in_output` |
| Move/rename operations are applied and reported | `pass_apply_patch_add_delete_and_move_are_tracked_in_output` |
| Patch paths outside workspace are rejected | `fail_apply_patch_rejects_path_outside_workspace` |
| Git Bash absolute paths inside the session directory are accepted | `add_file_accepts_git_bash_absolute_path_inside_session_dir` |
| Shell-embedded apply_patch is intercepted before shell execution | `pass_shell_embedded_apply_patch_is_intercepted_before_shell_execution`, `extracts_apply_patch_embedded_in_shell_wrapper` |
| Read-only text output containing patch markers is not intercepted as a patch | `does_not_extract_patch_from_read_only_text_output` |

## Provider Streaming And Context History

| Current behavior | Tura coverage |
| --- | --- |
| Codex OAuth waits for complete `command_run` JSON before emitting tool call | `command_run_streaming_waits_for_complete_json_arguments` |
| Codex OAuth emits complete `command_run` JSON as object arguments | `command_run_streaming_emits_complete_json_arguments` |
| Codex event argument deltas accumulate before tool-call emit | `codex_event_tool_calls_accumulates_argument_deltas_before_emit` |
| Incomplete Codex event deltas are not emitted | `codex_event_tool_calls_does_not_emit_incomplete_command_run_arguments` |
| Function call/output pairs are preserved in command_run style | `runtime_context_messages_preserve_codex_current_context_without_extra_tail`, `build_context_flattens_nested_command_run_batch_results` |
| Previous command evaluation targets the last command_run command list | `previous_command_evaluation_targets_lists_last_command_run_commands` |
| User-visible runtime text hides raw tool arguments | `user_visible_runtime_text_hides_raw_tool_argument_payload` |
| User-visible runtime text extracts assistant reply text from tool payloads | `user_visible_runtime_text_extracts_reply_message_from_tool_payload` |
| Non-final turns keep automatic tool choice instead of forcing a final answer | `non_final_turn_leaves_tool_choice_auto` |

## E2E Preflight And Long-Run Detection

| Current behavior | Tura coverage |
| --- | --- |
| Long E2E refuses to start Tura agents when command_run robustness preflight fails | `command_run_codex_two_way_e2e.mjs` phase `run tura command_run robustness preflight` |
| Four-link E2E records per-agent shell surface, prompt, tool logs, tokens, and verification | `command_run_codex_two_way_e2e.mjs` summary fields `task_prompts`, `model_config.shell_surfaces`, `runs[].tool_analysis`, `runs[].llm`, `runs[].verify` |
| E2E can run current bash/current shll/tura bash/tura shll in one session | `COMMAND_RUN_AGENT_AGENTS=current-bash,current-shll,tura-bash,tura-shll` |
