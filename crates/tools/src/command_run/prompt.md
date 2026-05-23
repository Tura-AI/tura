`command_run` is the only model-visible execution tool for coding work.

Run tools as a pure batch+step command runner. Always use `commands[]`.
`command_run` results must be returned through the tool/runtime result channel only.
Do not copy, summarize as JSON, or replay `command_run` results in assistant content.
Use assistant content only for concise reasoning, progress, and conclusions.
Prefer one complete batch for predictable work: discover/read/search/download
the needed context, edit, test, and validate in ordered steps when dependencies are clear.
Use early steps for needed discovery/read/search commands. Use later steps for
`apply_patch`, generated writes, tests, builds, and verification that should wait
for earlier results. Mutating or unknown commands act as barriers and run one at
a time. Different steps run in ascending order.
Each command defaults to a 90 second timeout. If a command reasonably needs more
or less time, set `timeout_ms` on that command item.

Command run patterns:
- Batch investigation: use early commands for the specific discovery, searches, and file reads needed to understand the failure surface.
- Keep related path listing, targeted search, and candidate file reads in the same command_run batch and same read-only step when they are independent.
- Parallel reads: put independent safe read-only commands in the same step when they do not depend on each other.
- Code repair loop: use early steps for needed discovery/reads, a later step for one multi-file `apply_patch`, and final steps in the same `command_run` for tests plus focused validation searches.
- Avoid embedding long generated source code or complex quoting directly in shell/bash command lines; for complex logic, invoke a script/interpreter from shell/bash rather than encoding the logic in shell syntax.
- Verification: run the relevant test or build command after edits in the same `command_run` when the edit target is already clear.
- Failure handling: inspect each failed item and change the next command based on that failure instead of retrying the same command.
- Example discovery batch: step 1 needed `rg --files`, targeted `rg -n`, and doc, sccript, file reads/runs.
- Example repair batch: step 1 `apply_patch` across related files, step 2 write or update a focused test script when needed, step 3 run the narrow test and focused validation searches.
- Example media batch: step 1 delete useless media/doc artifacts, step 2 search/download/generate the needed media, docs, or repo artifacts, step 3 read and verify the resulting media or repo content.

Commands:
