# Tura
Tura is a local open-source coding agent built for developers who are tired of useless skills, extensions that claim they can save tokens, and agents that wreck repos without judgment.

Long-horizon task benchmarks are one of the best ways to measure real-world coding-agent performance. Instead of testing agents on isolated prompts or simple Q&A tasks, Tura uses harness-based benchmarks that simulate actual long-horizon development workflows.
> Across 150+ long-horizon benchmark tasks selected from the top 20% most difficult SWE-bench Pro, Deep SWE, and SWE-bench Verified cases, Tura reduced token usage by 75% and achieved an 89% score, outperforming Claude Code by 34%.

You can compare Tura’s real-world performance against today’s most popular coding agents and extensions on the same benchmark tasks.

What makes Tura different is that it is built from eval data, not marketing claims. Tura is powered by a unique harness-driven framework built around four core systems:

## Macro CLI Command Run
Most coding agents still depend on repetitive tool-calling loops: inspect, wait, patch, wait, build, wait, test, wait.

***Tool-calling coding agent:***
```bash
# Turn 1 — inspect environment

rg -n "TODO|command_run|handler" crates/
rg --files crates/runtime/src crates/tools/src
```

```bash
# Turn 2 — apply patch

*** Begin Patch
*** Update File: crates/tools/src/command_run/handler.rs
@@
-    // old command handler logic
+    // patched command handler logic
*** End Patch
```

```bash
# Turn 3 — build

cargo build -p runtime
```

```bash
# Turn 4 — run tests

cargo test -p runtime --lib
```

```bash
# Turn 5 — run lint validation

cargo clippy -p runtime --all-targets
```

Tura takes a different approach. Instead of exposing dozens of small tools to the model, Tura exposes a single macro tool: `command_run`. This lets the agent construct a multi-step execution tree and run related actions in one LLM turn.

In the example below, Tura finishes in one LLM turn what a normal tool-calling agent needs five turns to complete. Both agents run the same commands. The difference is that Tura executes them as one structured macro workflow, while the traditional agent must pay the cost of repeated model round trips.

***Tura macro CLI command:***
```json
{
  "name": "command_run",
  "arguments": {
    "commands": [
      { "step": 1, "command_type": "shell_command", "command_line": "rg -n \"TODO|command_run|handler\" crates/" },
      { "step": 1, "command_type": "shell_command", "command_line": "rg --files crates/runtime/src crates/tools/src" },
      { "step": 2, "command_type": "apply_patch", "command_line": "*** Begin Patch\n*** Update File: crates/tools/src/command_run/handler.rs\n@@\n-    // old command handler logic\n+    // patched command handler logic\n*** End Patch" },
      { "step": 3, "command_type": "shell_command", "command_line": "cargo build -p runtime" },
      { "step": 4, "command_type": "shell_command", "command_line": "cargo test -p runtime --lib" },
      { "step": 4, "command_type": "shell_command", "command_line": "cargo clippy -p runtime --all-targets" }
    ]
  }
}
```
This reduces cached input token usage by up to 5x and improves total completion time by 3.4x in this case.

In heavier workflows involving Playwright browser automation, multi-step internet research, and CI validation, the advantage can grow to 6–9x. You can check the benchmark page for details.


## Backward Reasoning
However impressive LLMs can be, an LLM is still, at its core, a statistical induction model over text-token probabilities.

If you ask an LLM to play rock-paper-scissors with you, depending on the provider, you will probably get something close to 50% rock, 35% paper, and 15% scissors. In other words, if you keep playing paper, you can easily win the game.

This is simply because these words have different occurrence probabilities in English. If you ask the same question in Japanese, the distribution will change.

In coding tasks, this is often fatal.

An agent is more likely to execute and generate code and logic that are statistically more common. But common code and common logic are often mediocre and under-considered.

Tura uses a different strategy.

During reasoning, a common agent reasons from the current state to the prompt goal. In that case, $s_1$ is the current state, and $s_n$ is the goal given by the user prompt.

$$
s_1 \\rightarrow s_2 \\rightarrow s_3 \\rightarrow \\cdots \\rightarrow s_n
$$

Instead, Tura guides the LLM to statistically estimate $s_{n-1}$ first, then reason backward from the state of $s_{n-1}$ to $s_{n-2}$.

In the example below, the LLM can derive the optimal strategy for playing rock-paper-scissors correctly.


```
> To keep rock-paper-scissors fair and challenging,
> We need unbiased play.
> Each move must have a true one-in-three chance.
> An LLM cannot guarantee that from text probabilities alone.
> Use a random-number generator script to generate randint(1, 3)
> Then map rock, paper, or scissors to the number.
```

In real programming tasks, this means that when an agent sees a goal like fixing a frontend bug, it is not pushed to cover the problem with a fallback, a guard clause, or a shallow patch. Instead, it is guided to reason through the full execution path, reconstruct the failure state, and identify the real root cause before writing code. This is one reason Tura can outperform other coding agents by 24% to 65% in benchmarked coding tasks.

## Test-Driven Development
In a very common debugging failure case, the reviewer or the test names a bug and gives an assertion about where the error happens.

In these SWE-bench-style cases, agents are several times more likely than usual to write the wrong patch and hide the problem instead of fixing the real cause.

Tura uses a standard TDD workflow to avoid this failure mode.

First, Tura builds a complete end-to-end test that covers the full lifecycle of the behavior. The goal is not to guess where the bug is, but to reproduce the bug through the real execution path.

Then Tura identifies the earliest invariant, the earliest point where the state becomes wrong, and the real root cause. It uses the failing assertion to reproduce the bug, fixes only the root cause, and reruns the test before considering the task complete.

```
> The user needs to fix a frontend bug where the same message sometimes appears twice.
> The complete message lifecycle goes from the remote LLM provider, through the gateway, into the frontend.
> The agent needs to analyze the earliest invariant state and the earliest point where duplication appears. Tura first reproduces the bug through a Playwright end-to-end test.
> A likely wrong patch would be to hide the issue in the frontend with a fallback or a deduplication guard. That makes the system more fragile and introduces uncertainty into the deduplication behavior.
> The earliest root cause is found in a race condition between the gateway live SSE event and the persisted session DB state.
> The correct fix is to unify the session state machine, patch only that cause, and rerun the failing test before marking the task as complete.
```
In benchmark samples, combining backward reasoning with a TDD workflow significantly reduced false completion.

When the agent claimed that the issue was solved, Tura failed to actually solve the problem only 4% of the time. By comparison, Codex had a 25% false-completion rate, and Claude Code had a 28% false-completion rate.

## Runtime Context and Prompt Manager

Skills are often just weaker prompts loaded into context.

In many agent frameworks, a long-lived session keeps accumulating skill files, tool outputs, and stale task history. When the context becomes too large, the agent enters a separate compaction turn, but that compaction usually preserves only a compressed summary. Important execution details can become vague or lost.

Tura treats context as part of the runtime state machine.

Instead of relying on users to manually reset sessions or letting Markdown skills pile up, Tura uses `task_status`, runtime prompts, and recursive execution manuals to keep the active context scoped to the current task.

| Area | Traditional Skill-Based Agent | Tura |
|---|---|---|
| Session model | Same session keeps running until the user manually starts a new one | Session state can be renamed, refreshed, and managed automatically |
| Skill loading | Loads Markdown skill files into context | Dynamically loads task-specific runtime prompts and execution manuals |
| Prompt strength | Skills behave like weak contextual instructions or tool output | Runtime prompts are tied to the active task state |
| Context pollution | Old skills and irrelevant context remain active until compacted or reset | Irrelevant context can be removed, compacted, or replaced |
| Compaction | Separate long compaction turn | Compaction is handled as a CLI operation |
| Information preserved | Usually compressed summaries only | Code locations, patches, tests, and task status can be preserved |
| Token cost | High because stale context stays active | Lower because context is task-scoped |
| Failure mode | Agent mixes old tasks, vague summaries, and irrelevant skills | Agent keeps execution aligned with the current task |
| Tool/manual loading | Broad skills are loaded even when only part is useful | CLI commands and manuals are loaded through a recursive task tree |

Because compaction is a CLI command, Tura can batch it through `command_run` together with the exact execution state. The next turn does not need to rediscover the task from a vague summary.

In benchmark samples, after a context compaction event, Tura resumed meaningful execution — either rerunning tests or applying a patch — in an average of 1.8 turns.

Codex needed 4.3 turns, while Claude Code needed 7.2 turns.
Across multiple debugging benchmarks, Tura was 54% more likely than Claude Code to locate the real issue in the hardest 20% of benchmark tasks, and 34% more likely to fix the actual bug.


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
