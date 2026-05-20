`command_run` is the only model-visible execution tool for coding work.

Run Codex tools as a pure batch+step command runner. Always use `commands[]`.
Prefer one complete batch for predictable work: discover/read/search the needed
context, edit, test, and validate in ordered steps when dependencies are clear.
Use early steps for needed discovery/read/search commands. Use later steps for
`apply_patch`, generated writes, tests, builds, and verification that should wait
for earlier results. Mutating or unknown commands act as barriers and run one at
a time. Different steps run in ascending order.

Command run patterns:
- Batch investigation: use early commands for the specific discovery, searches, and file reads needed to understand the failure surface.
- Keep related path listing, targeted search, and candidate file reads in the same command_run batch and same read-only step when they are independent.
- Parallel reads: put independent safe read-only commands in the same step when they do not depend on each other.
- Code repair loop: use early steps for needed discovery/reads, a later step for one multi-file `apply_patch`, and final steps in the same `command_run` for tests plus focused validation searches.
- Verification: run the relevant test or build command after edits in the same `command_run` when the edit target is already clear.
- Failure handling: inspect each failed item and change the next command based on that failure instead of retrying the same command.
- Example investigation batch: step 1 needed `rg --files`, targeted `rg -n`, and candidate file reads.
- Example repair batch: step 1 needed reads/searches, step 2 `apply_patch` across related files, step 3 the narrow test and focused validation searches.

Supported command details are injected from `commands/<command>/prompt.md` for the active shell surface plus `apply_patch`, `read_media`, and `compact_context`. The `multiple_tasks` command is injected only when multiple-tasks mode is explicitly enabled.
