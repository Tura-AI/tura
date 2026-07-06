# Tura

Tura is a terminal-native coding agent built for long-horizon repository work:
inspect the repo, reason backward from the target outcome, make narrow changes,
verify them, and leave an audit trail instead of a pile of hopeful chat.

## Benchmark-first agent work

Tura is designed around the kind of tasks where coding agents usually start to
look expensive: multi-turn debugging, real repository refactors, broad context,
tool-heavy investigation, and verification that cannot be guessed from one file.

The benchmark story in [`i18n.js`](i18n.js) is blunt: Tura targets long-horizon
tasks, token discipline, and verified outcomes. The benchmark set emphasizes:

| Benchmark focus | Why it matters |
| --- | --- |
| Long-horizon repository tasks | Agents have to keep intent, files, commands, and evidence aligned for many turns. |
| Diversified debug tasks | Real fixes require reproduction, trace inspection, root-cause analysis, and validation. |
| High-resolution challenge tests | Hard tasks separate agents better than easy prompt demos with narrow score bands. |
| Real repo and refactoring tasks | Production-scale codebases expose context drift, brittle edits, and weak verification. |
| Token discipline | Strong agents should preserve context and avoid spending tokens on redundant tool chatter. |

In the benchmark copy, Tura is positioned against other agents and extensions on
performance under pressure: higher verified capability, fewer wasted tokens, and
better persistence on broad tasks. Treat the exact numbers as benchmark output,
not decoration. The important point is the design target: Tura is optimized for
measured long-horizon work, not for looking clever in a short prompt clip.

Read the benchmark system notes in [Benchmark](benchmark/README.md).

## Built from eval, not claims

Tura's core claim is not "the model is smart." Models are already smart. The
failure mode is the harness around them: vague prompts, scattered tools, weak
state, loose completion criteria, and no durable proof that the change actually
worked.

Tura is built as an evaluation-driven harness:

- `command_run` keeps tool use compact and auditable;
- runtime prompt manuals load only for the active task type;
- session DB keeps durable context, task state, todos, and command evidence;
- providers and models are routed through explicit config instead of ad hoc env
  guesses;
- verification is part of the completion path, not a nice-to-have epilogue.

The result is a local engineering runtime for agent work. Less magic. Fewer
fireworks. Better chance your repo survives contact with ambition.

## 1. Macro Command

Tura's macro command surface is `command_run`: one compact tool interface that
can batch reads, searches, patches, task-state updates, media/web commands, and
validation into ordered steps. Instead of paying for a large list of provider
visible tools every turn, Tura exposes a smaller execution surface and records
what happened.

Example: a repo inspection task can batch independent reads first, then run the
dependent check only after the relevant files are known.

```text
Goal: inspect a workspace before editing.

step 1:
  rg --files
  rg -n "TODO|FIXME|panic|unwrap" crates apps docs
  Get-Content docs/SUMMARY.md

step 2:
  run the focused test or build command discovered from the files

step 3:
  update task_status with the active work area and task type
```

That is not just convenience. It reduces repeated tool schemas, keeps output
grouped by intent, and makes command history easier to audit when a task goes
long.

Docs:

- [Command run](docs/core/command-run.md)
- [Commands](docs/core/commands.md)
- [Tool architecture](crates/tools/ARCHITECTURE.md)

## 2. Backward Thinking

Tura pushes the agent to reason from the required verified end state backward to
the current move. For debugging, that means the finish line is not "change a
file"; it is "the smallest reproduction fails before the patch and passes after
the patch."

Example: a duplicated stream-message bug should not be fixed first in the GUI.
The safer path is backward:

```text
Desired end state:
  session replay shows each provider stream chunk once.

Previous necessary state:
  session_db appends each event id once.

Previous necessary state:
  runtime and gateway preserve provider event identity.

Current move:
  replay provider stream chunks and assert the duplicate in a focused test.

Wrong direction to avoid:
  add frontend dedupe before proving the root cause.
```

That pattern is why Tura treats issue text, user suggestions, and symptoms as
clues rather than proof. It is slower than guessing for about five minutes, then
faster for the rest of the task. Annoying how often that wins.

Docs:

- [Task status](docs/core/task-status.md)
- [Context management](docs/core/context-management.md)
- [Testing](scripts/ARCHITECTURE.md#xtask-test-collection-scripts)

## 3. Runtime Context

Tura does not paste every possible instruction into every request. Runtime
prompt manuals are selected by `task_status.task_type`, persisted as session
records, and reinserted after compaction. That keeps the task mode active
without dragging every manual through every turn.

Example: a visual frontend task can activate the frontend and visual manuals; a
release failure can activate devops; a document rewrite can activate editorial.
The session records preserve the active mode when context is compacted.

```text
User asks:
  "Fix the GUI transcript rendering and verify it visually."

Runtime state:
  task_group = "GUI transcript"
  task_type = ["visual", "frontend", "debug"]

Effect:
  visual verification rules apply;
  frontend behavior rules apply;
  debug reproduction rules apply;
  command_run gains any capabilities required by the active manuals;
  compaction re-adds the active manuals instead of forgetting the task mode.
```

This is the practical difference between a runtime and a motivational prompt.
The model gets the instructions that match the job, at the time they matter.

Docs:

- [Runtime prompt](docs/core/runtime-prompt.md)
- [Dynamic prompt injection](docs/core/prompt-style.md)
- [Runtime architecture](crates/runtime/ARCHITECTURE.md)

## 4. Test Driven Development

Tura treats a prompt as the start of an investigation, not permission to patch
blindly. Debug and repair tasks should reproduce the problem first, identify the
smallest safe edit, then run the check that proves the outcome.

Example: a failing provider startup path should become a focused test before it
becomes a broad fallback.

```text
Task:
  provider config starts empty and runtime crashes.

Tura flow:
  1. find the provider config load path;
  2. create or run the smallest test that starts with an empty provider set;
  3. observe the failure at the stable boundary;
  4. patch the config handling without hiding unrelated errors;
  5. rerun the focused test;
  6. run the broader relevant suite only after the focused test passes.

Completion evidence:
  exact command, passing result, changed files, and any remaining test gap.
```

That completion evidence matters. "Looks fixed" is not a release strategy; it is
a genre of future incident report.

Docs:

- [Testing](scripts/ARCHITECTURE.md#xtask-test-collection-scripts)
- [Sessions](docs/start/sessions.md)
- [Command run](docs/core/command-run.md)

## Install and run

### NPM release

```bash
npm install tura-ai
tura
tura exec "Inspect this workspace and summarize the risky parts"
```

Windows:

```powershell
npm install -g tura-ai
tura
tura exec "Inspect this workspace and summarize the risky parts"
```

### Source checkout

```powershell
git clone https://github.com/Tura-AI/tura.git
cd tura
.\scripts\install.ps1
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
tura exec "Inspect this workspace"
```

```bash
git clone https://github.com/Tura-AI/tura.git
cd tura
./scripts/install.sh
./scripts/build-release.sh
./scripts/register-cli.sh
tura exec "Inspect this workspace"
```

### Common entrypoints

| Entry | Use it for |
| --- | --- |
| `tura` | Interactive terminal UI. |
| `tura "prompt"` | Open the TUI with an initial prompt. |
| `tura exec "prompt"` | Direct Rust CLI prompt runner. |
| `tura run "prompt"` | Gateway-backed prompt with streaming/history. |
| `tura bash`, `tura zsh`, `tura shel` | Prompt with a selected command-run shell surface. |
| `tura_gateway` | Local HTTP/SSE gateway and optional web GUI serving. |
| `tura_gui` | Desktop GUI workspace client. |

For OS-specific PATH and executor requirements, see
[How to start](docs/start/how-to-start.md). For command options, see
[CLI parameters](docs/start/cli-parameters.md).

## Documentation

The organized documentation lives in [docs/SUMMARY.md](docs/SUMMARY.md).

### Start

- [Overview](docs/start/overview.md)
- [Install](docs/start/install.md)
- [How to start](docs/start/how-to-start.md)
- [CLI parameters](docs/start/cli-parameters.md)
- [Settings](docs/start/settings.md)
- [Providers](docs/start/providers.md)
- [Sessions](docs/start/sessions.md)
- [Navigation](docs/start/navigation.md)

### Core

- [Task status](docs/core/task-status.md)
- [Context management](docs/core/context-management.md)
- [Runtime prompt](docs/core/runtime-prompt.md)
- [Command run](docs/core/command-run.md)
- [Commands](docs/core/commands.md)
- [Agents](docs/core/agents.md)
- [Personas](docs/core/personas.md)
- [Rich text](docs/core/html-rich-text.md)
- [Dynamic prompt injection](docs/core/prompt-style.md)

### Architecture

- [Session DB](crates/session_log/ARCHITECTURE.md)
- [Gateway](crates/gateway/ARCHITECTURE.md)
- [Router](crates/router/ARCHITECTURE.md)
- [Runtime](crates/runtime/ARCHITECTURE.md)
- [Tool](crates/tools/ARCHITECTURE.md)
- [Terminal user interface](apps/tui/ARCHITECTURE.md)
- [Graphic user interface](apps/gui/ARCHITECTURE.md)

### Customization

- [Custom providers](docs/customization/custom-providers.md)
- [Custom personas](docs/customization/custom-personas.md)
- [Custom agents](docs/customization/custom-agents.md)
- [Custom runtime prompt](docs/customization/custom-runtime-prompt.md)
- [Custom commands](docs/customization/custom-commands.md)

### Development

- [Scripts](scripts/ARCHITECTURE.md)
- [Testing](scripts/ARCHITECTURE.md#xtask-test-collection-scripts)
- [Environment](docs/start/settings.md)
- [Architecture](ARCHITECTURE.md)
- [Benchmark](benchmark/README.md)

## License

Tura is licensed under AGPL-3.0-or-later. See [LICENSE](LICENSE).
