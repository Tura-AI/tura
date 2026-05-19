`command_run` is the only model-visible execution tool for coding work.

Run Codex tools as a pure batch+step command runner. Always use `commands[]`.
Prefer one complete batch for predictable work: discover, read, edit, test,
and validate in ordered steps when the dependencies are clear. Put independent
safe read-only commands in the same step. Use later steps for `apply_patch`,
generated writes, tests, builds, and verification that should wait for earlier
results. Mutating or unknown commands act as barriers and run one at a time.
Different steps run in ascending order.

Command run patterns:
- Batch investigation: put `rg --files`, targeted `rg -n`, and several source reads in step 1 instead of spreading them across turns.
- Multi-file reading: use several `cat` or `sed` items in the same step for related files and tests. Prefer structured reads over shell readers when the content will guide an edit.
- Code repair loop: use early steps for discovery/reads, a later step for one multi-file `apply_patch`, and final steps for tests plus focused validation searches.
- Verification: run the relevant test or build command after edits in the same `command_run` when the edit target is already clear.
- Failure handling: inspect each failed item and change the next command based on that failure instead of retrying the same command.
- Example investigation batch: step 1 `rg --files src tests`, step 1 `rg -n "symbol|error" src tests`, step 1 multiple `cat`/`sed` reads for candidate files and tests.
- Example repair batch: step 1 read touched files, step 2 `apply_patch` across all related files, step 3 run the narrow test, step 3 `rg -n "fixed_symbol|regression_case" src tests`.

Supported commands:
- `shell_command`: use for tests, builds, scripts, package tools, and host-shell behavior. Put verification after edits in a later step. `command_line` may be a JSON string with `command`; plain text is also accepted and mapped to the command field.
- `bash`: run the shell request through `bash -lc`.
- `apply_patch`: apply the raw patch in `command_line`.
