# Tura

Tura is an open-source, terminal-native coding agent for long-horizon repository
work: reproduce the issue, inspect the real execution path, patch narrowly,
verify with commands, and keep the evidence attached to the session instead of
ending with a confident shrug. Computers already do enough shrugging.

## Benchmark advantage first

The homepage copy in `i18n.js` makes the claim in measurable terms:
Tura is built to solve problems other coding agents cannot reliably finish. In
150+ long-horizon benchmark tasks, the copy reports Tura saving 75% of tokens and
scoring 89%, 24% higher than Claude Code.

The benchmark is not a simple Q&A token-saving demo. It evaluates agents and
extensions in real development workflows: inspect a production repository, hold
the task objective for many turns, run the right commands, patch the right owner
code, recover from failed validation, and prove the result.

| Benchmark dimension | Concrete shape from `i18n.js` | Why it matters |
| --- | --- | --- |
| Long-horizon task set | 150+ long-horizon benchmark tasks | Measures persistence across multi-step repository work instead of one-shot answers. |
| Diversified debug tasks | Tasks require 20+ turns on average | Forces reproduction, trace inspection, patching, reruns, and follow-up decisions. |
| Community benchmark | Anyone can contribute benchmark tasks and evaluation reports | Keeps the benchmark expandable instead of frozen around vendor-friendly examples. |
| High-resolution challenge tests | Focuses on the top 20% most difficult issues and uses finer scoring | Separates strong agents from agents that look similar on easy tasks. |
| Real repo and refactoring tasks | Reproduction tasks from tens of thousands to millions of lines of code | Exposes context drift, weak file selection, brittle edits, and fake completion. |
| Token discipline | Verified strength is plotted against token use | Rewards agents that preserve context instead of spending it on repeated tool chatter. |

The benchmark metadata in the product copy is deliberately production-scale:

| Metric | Value |
| --- | ---: |
| Sessions | 1,310 |
| Languages | 6 |
| Tasks | 104 |
| Total lines of code | 5,404,042 |
| Total eval harness | 2,392 |
| Metadata fields | 45 |

The benchmark chart copy ranks the compared agents like this:

| Agent | Benchmark score |
| --- | ---: |
| Tura | 95 |
| Claude Code | 71 |
| Codex | 67 |
| Cursor | 63 |
| Chat | 49 |

That is the design target: higher verified task strength with fewer wasted
tokens on hard repository work. The benchmark notes live in
[benchmark/README.md](benchmark/README.md), but the short version is simple:
Tura is optimized for the part of coding-agent work where optimistic chat stops
being useful.

## Built from eval, not claims

Tura is not built around the claim that a model is smart. The model is only one
piece. The failure mode in real coding-agent work is usually the harness around
the model: vague tools, unstable context, no durable task state, weak
reproduction discipline, and completion claims made before the repo is verified.

Tura is benchmark/eval-built:

- no provider-visible tool-calling sprawl; macro commands collapse execution into
  compact ordered batches;
- no context-compaction turns as the main product trick; runtime context is
  managed as session state instead of drifting through summaries;
- no skill plugin pile; task-specific manuals are selected by runtime state and
  injected only when they are needed;
- no completion-by-vibes; tests, builds, screenshots, logs, or explicit blockers
  are part of the answer.

The core loop is:

```text
objective -> inspect stable boundary -> reproduce -> patch -> verify -> audit
```

The implementation details are split across the runtime, tools, session log, and
gateway owner docs:

- [Runtime architecture](crates/runtime/ARCHITECTURE.md)
- [Tool architecture](crates/tools/ARCHITECTURE.md)
- [Session DB architecture](crates/session_log/ARCHITECTURE.md)
- [Gateway architecture](crates/gateway/ARCHITECTURE.md)

## Feature 1: Macro Command

`i18n.js` describes this as "Tool calling replaced by macro commands" and
"Complete in one turn what other agents need three turns to do." In Tura, the
macro surface is [`command_run`](docs/core/command-run.md): a single ordered
batch that can run shell commands, apply patches, update task state, discover
web or media references, inspect media, and validate results.

Instead of the model spending separate turns on "search", then "read", then
"patch", then "test", Tura groups independent work into the same step and puts
dependent work in later steps. That matters in long tasks because every wasted
tool round trip burns context and increases the chance the agent forgets what it
was proving.

### Concrete use case: patch runtime command execution

The homepage command animation gives the exact shape:

```text
Commands

#1 shell_command running
$ rg -n "TODO" crates C:\Users\liuliu\Documents\tura\crates\runtime\src\turn_loop\mod.rs:214

#1 shell_command running
$ rg --files crates/runtime/src C:\Users\liuliu\Documents\tura\crates\runtime\src

#2 apply_patch pending
crates/tools/src/command_run/handler.rs

#3 shell_command running
$ cargo build -p runtime C:\Users\liuliu\Documents\tura

#4 shell_command running
$ cargo test -p runtime --lib C:\Users\liuliu\Documents\tura

#4 shell_command running
$ cargo clippy -p runtime --all-targets C:\Users\liuliu\Documents\tura
```

Expanded as a real Tura workflow:

```text
Goal:
  Fix a runtime command-run issue without touching unrelated code.

Step 1: discover the stable boundary.
  - Search the runtime turn loop for the reported behavior.
  - List runtime source files to find the actual owner module.
  - Read the command-run handler and nearby tests.

Step 2: patch only the owner code.
  - Use apply_patch on crates/tools/src/command_run/handler.rs.
  - Keep the edit narrow enough that a review can explain it in one sentence.

Step 3: verify compile behavior.
  - cargo build -p runtime

Step 4: verify behavior and quality.
  - cargo test -p runtime --lib
  - cargo clippy -p runtime --all-targets

Completion evidence:
  - changed file names;
  - exact commands run;
  - pass/fail result;
  - any remaining blocker if validation cannot run.
```

The point is not just batching. It is execution discipline: independent discovery
runs together, patches happen after the owner module is known, and validation is
not a decorative afterword.

Docs and owner code:

- [Command run](docs/core/command-run.md)
- [Commands](docs/core/commands.md)
- [Tool architecture](crates/tools/ARCHITECTURE.md)

## Feature 2: Backward Thinking

`i18n.js` describes backward reasoning with a deliberately small example:
rock-paper-scissors. To keep the game fair, the agent must reason backward from
the desired property, not forward from whatever token it feels like producing.

```text
To keep rock-paper-scissors fair and challenging,
we need unbiased play.
Each move must have a true one-in-three chance.
A LLM cannot guarantee that from text probabilities alone.
Use a random-number generator script to generate randint(1, 3).
Then map rock, paper, or scissors to the number.
```

That is the small version. The repository version is the same pattern with more
damage available.

### Concrete use case: duplicated stream messages

A weaker agent sees duplicated GUI messages and starts by adding frontend
deduplication. That may hide the symptom while preserving the bug in the session
log. Tura works backward from the verified end state:

```text
Desired end state:
  The user sees each streamed provider message once after live display and replay.

Previous necessary state:
  The session DB stores each provider event id once.

Previous necessary state:
  Runtime preserves provider event identity when forwarding stream chunks.

Previous necessary state:
  Provider integration emits stable chunk identifiers, or the runtime creates a
  stable id before persistence.

Current move:
  Replay provider stream chunks and assert the duplicate at the session_db
  boundary before touching the GUI.

Avoid:
  A frontend-only dedupe patch before the persistence path is proven correct.
```

This is why Tura's completion criteria usually sound strict. The finish line is
not "the visible duplicate disappeared." The finish line is "the stable boundary
that caused the duplicate is reproduced, patched, and verified."

Docs and owner code:

- [Context management](docs/core/context-management.md)
- [Task status](docs/core/task-status.md)
- [Runtime architecture](crates/runtime/ARCHITECTURE.md)
- [Session DB architecture](crates/session_log/ARCHITECTURE.md)

## Feature 3: Runtime Context

`i18n.js` describes runtime context as the replacement for "complex skills
management and task drift after context compression." Tura stores task focus,
manual selection, and compacted state in the runtime/session system instead of
making the model pretend it remembers everything.

The homepage example is a visual/frontend task:

```text
new task: edit webpage button colors
choose manuals: frontend + visual
compact_context(drop stale data-flow)
keep: code patterns + visual prefs
resume from latest user request
verify screenshots: 1440 / 768 / 390
```

Expanded as a real Tura workflow:

```text
Goal:
  Change webpage button colors without breaking layout or interaction behavior.

Runtime state:
  task_status.task_type = ["frontend", "visual"]

Context kept:
  - current user request;
  - relevant code patterns;
  - visual preferences and constraints;
  - files already changed;
  - validation still required.

Context dropped:
  - stale backend data-flow notes;
  - unrelated earlier exploration;
  - old candidate approaches that were not used.

Verification required:
  - inspect the changed component;
  - run the known frontend check;
  - capture or inspect screenshots at 1440, 768, and 390 widths;
  - report the exact command and result.
```

That means a resumed task is not a new agent guessing from a vague summary. It
has a task type, a selected manual set, a compact context checkpoint, and a clear
verification path. Less mystical than "skills", more useful than hoping the
model's attention span survives a 40-turn debug session.

Docs and owner code:

- [Context management](docs/core/context-management.md)
- [Runtime prompt](docs/core/runtime-prompt.md)
- [Task status](docs/core/task-status.md)
- [Runtime architecture](crates/runtime/ARCHITECTURE.md)

## Feature 4: Test Driven Development

`i18n.js` says Tura treats every prompt as a starting point: before making a
patch, it investigates the full execution path and reproduces the issue first.
The benchmark copy also says Tura reduced false completion claims by an average
of 76% compared with a model-native harness.

The product example is explicit:

```text
task: duplicated stream messages
repro: replay provider stream chunks
assert: session_db appends each id once
trace: llm > provider > runtime > session_db
then inspect gateway > gui replay
avoid: frontend dedupe before root cause
```

### Concrete use case: provider streaming duplication

Expanded into the TDD loop:

```text
Reported problem:
  The GUI shows duplicated streamed assistant messages.

Wrong first patch:
  Add a UI dedupe filter and call it done.

Tura test-first path:
  1. Reproduce by replaying provider stream chunks.
  2. Assert session_db appends each event id exactly once.
  3. Trace the event path: llm -> provider -> runtime -> session_db.
  4. Patch the first boundary that creates or stores duplicates.
  5. Inspect gateway replay after persistence is correct.
  6. Inspect GUI replay after gateway behavior is correct.
  7. Report the focused reproduction, changed files, and passing validation.

Completion evidence:
  - the reproduction fails before the patch;
  - the same reproduction passes after the patch;
  - gateway and GUI replay checks do not reintroduce duplicates;
  - no unrelated frontend masking is used as the root fix.
```

This is what benchmark/eval-built means in practice: Tura's behavior is shaped
by the failure cases where agents normally overclaim completion. It is built to
force the missing proof into the workflow.

Docs and owner code:

- [Testing scripts](scripts/ARCHITECTURE.md#xtask-test-collection-scripts)
- [Sessions](docs/start/sessions.md)
- [Command run](docs/core/command-run.md)
- [Gateway architecture](crates/gateway/ARCHITECTURE.md)
- [Graphic user interface architecture](apps/gui/ARCHITECTURE.md)

## Install and run

### NPM release

Mac and Linux:

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

Windows PowerShell:

```powershell
git clone https://github.com/Tura-AI/tura.git
cd tura
.\scripts\install.ps1
.\scripts\build-release.ps1
.\scripts\register-cli.ps1
tura exec "Inspect this workspace"
```

macOS or Linux shell:

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

For OS-specific PATH requirements, executor installation, and how to register the
CLI when the executable is not on PATH, read
[How to start](docs/start/how-to-start.md). For command flags and modes, read
[CLI parameters](docs/start/cli-parameters.md).

## Documentation

The GitBook-style documentation index is [docs/SUMMARY.md](docs/SUMMARY.md). The
full navigation page is [docs/start/navigation.md](docs/start/navigation.md).

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
