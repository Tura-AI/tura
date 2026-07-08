# CLI Bug-Fix Binary Matrix

This debug benchmark is the bug-fix counterpart to the eza-style binary oracle
source-port benchmark.

Each task starts agents from a known buggy source version of a widely used CLI
tool. The prompt describes the bug and constraints, but not hidden verifier
answers. The harness validates the repair by running the agent-built CLI on the
same reproducer inputs and comparing the observable result with the next fixed
release binary. Repository test suites are not the oracle.

The task metadata records:

- buggy and fixed release versions
- exact buggy and fixed tag commits
- upstream issue number plus dynamically extracted issue text used in the prompt
- upstream issue plus fixing PR or commit when available
- binary acquisition method
- reproducer files and command invocations
- expected buggy-vs-fixed behavior
- approximate repository source line scale used during selection

Agent prompts include the buggy version/ref/commit and issue report text
rendered from task metadata. They do not include the issue URL, issue number,
fixed source, fixed binary paths, fixed output, or fixing commit history. The
agent-visible `task.json` follows the same rule: it carries only extracted issue
text, not tracker identifiers. The prompt explicitly forbids web search,
browsing GitHub, fetching remote git history, or inspecting any commit, tag,
branch, release archive, or source package newer than the buggy commit.

The runner also writes `harness-metadata.json` under the benchmark run root. That
file is harness-only oracle metadata: it contains buggy and fixed versions,
refs, commits, issue numbers/URLs, binary rules, build commands, and reproducer
cases for preflight and candidate-vs-fixed scoring.

`oracle-matrix.json` is the binary verifier matrix. Every task has:

- `interfaceAudit`: smoke commands or protocols that are run against both real
  release binaries before scoring, so the harness proves it inspected the actual
  CLI surface.
- `failToPass`: hidden oracle cases where the buggy binary and fixed binary must
  differ. A candidate repair passes only if its observable CLI behavior matches
  the fixed binary.
- `passToPass`: hidden preservation cases where buggy and fixed binaries already
  agree. A candidate repair must keep those behaviors intact.

The binary audit mode downloads or installs the configured buggy and fixed
release executables into a suite-level cache, runs interface audit entrypoints,
then runs the f2p/p2p preflight. It writes per-task reports plus a suite report
at `binary-audit/binary-audit.json`. That report records the binary command,
buggy/fixed version, ref, commit, command group, compare policy, status, stdout
head, stderr head, side-effect snapshots, and any failed preflight case.

Run the real binary oracle preflight:

```bash
COMMAND_RUN_AGENT_BINARY_AUDIT=1 \
COMMAND_RUN_AGENT_ORACLE_PREFLIGHT=1 \
COMMAND_RUN_AGENT_TASKS=all \
node benchmark/tasks/debug/cli-bugfix-binary-matrix-10/runner.mjs
```

Run a metadata self-test:

```bash
COMMAND_RUN_AGENT_SELF_TEST=1 node benchmark/tasks/debug/cli-bugfix-binary-matrix-10/runner.mjs
```

Select a subset:

```bash
COMMAND_RUN_AGENT_TASKS=eza-grid-non-tty,ripgrep-empty-vfile-invert node benchmark/tasks/debug/cli-bugfix-binary-matrix-10/runner.mjs
```
