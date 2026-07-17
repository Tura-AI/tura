# Tura Documentation

Source: https://turaai.net/docs#benchmark-current-test-set-record

Title: Tura Documentation

URL Source: https://turaai.net/docs

Markdown Content:
CI sync: 17 Jul 2026, 05:52:18 UTC[https://github.com/Tura-AI/tura/blob/main/README.md](https://github.com/Tura-AI/tura/blob/main/README.md)

Tura is a local, open-source coding agent for developers who are tired of vague skill claims, token-saving extensions with no evidence, and agents without judgment wreck their repos.

Across 20 DeepSWE v1.1 tasks, tested 60 sessions with GPT-5.6 SOL at High reasoning effort, Tura creates a substantial token-budget advantage by reducing repeated context and model round trips. You can spend that advantage in two ways. Direct turns most of it into lower cost: 83.5% fewer aggregate tokens than the official Codex CLI High configuration, with a verifier success rate of 65.0% versus 60.0%. Balanced puts more of the saved budget back into reasoning, investigation, and verification. It reached an 80.0% success rate—20 percentage points higher than Codex CLI High—while still using 49.6% fewer tokens.[[1]](https://turaai.net/docs#fn1)[[2]](https://turaai.net/docs#fn2)

#### Benchmark

Long-horizon task [benchmarks](https://turaai.net/benchmark) are one way to look past a polished isolated prompt and see how an agent handles real work. The published comparison uses harness-based development tasks with archived prompts, per-round tool calls, token usage, patches, and verifier results.

> The primary comparison below holds the model and reasoning label fixed: Tura Balanced High, Tura Direct High, and the official Codex CLI High configuration on 20 DeepSWE tasks and 5 rewrite tasks. The evidence record also retains Codex CLI Medium as a separate secondary configuration; the benchmark methodology keeps 2 separately reviewed design tasks outside the harness-scored population.[[1:1]](https://turaai.net/docs#fn1)

[Full report on GitHub](https://turaai.net/docs#benchmark-current-test-set-record)

The published results do not establish equivalent quality or performance for every configured provider. Broader Anthropic/Claude, Google/Gemini, OpenAI-compatible, local-provider, UI-latency, runtime/session parsing, and cross-OS measurements remain part of the documented [roadmap](https://turaai.net/docs#roadmap) and [known evidence gaps](https://turaai.net/docs#known-issues).

#### Screenshots

![Image 1: Tura GUI](https://raw.githubusercontent.com/Tura-AI/tura/main/assets/screenshot/gui-ci-quality-demo.svg)

_GUI page with multi-session concurrent work and HTML rich text support._

![Image 2: Tura TUI](https://raw.githubusercontent.com/Tura-AI/tura/main/assets/screenshot/tui-ci-quality-demo.svg)

_TUI page with multi-session concurrent work and HTML rich text support._

The results below come from published benchmark artifacts, not an uncited aggregate. Three systems do most of the work:

### Macro CLI Command Run

Most coding agents still depend on repetitive tool-calling loops: inspect, wait, patch, wait, build, wait, test, wait.

_**Tool-calling coding agent:**_

```
# Turn 1 — inspect environment

rg -n "TODO|command_run|handler" crates/
rg --files crates/runtime/src crates/tools/src
```

```
# Turn 2 — apply patch

*** Begin Patch
*** Update File: crates/tools/src/command_run/handler.rs
@@
-    // old command handler logic
+    // patched command handler logic
*** End Patch
```

```
# Turn 3 — build

cargo build -p runtime
```

```
# Turn 4 — run tests

cargo test -p runtime --lib
```

```
# Turn 5 — run lint validation

cargo clippy -p runtime --all-targets
```

Tura takes a different route. Instead of exposing dozens of small tools to the model, it exposes one macro tool: `command_run`. The agent can then build a multi-step execution tree and run related actions in one LLM turn.

In the example below, both agents run the same commands. A normal tool-calling agent needs five LLM turns; Tura handles the sequence as one structured macro workflow. The saved work is conversational overhead, not engineering discipline.

_**Tura macro CLI command:**_

```
{
  "name": "command_run",
  "arguments": {
    "commands": [
      {
        "step": 1,
        "command_type": "shell_command",
        "command_line": "rg -n \"TODO|command_run|handler\" crates/"
      },
      {
        "step": 1,
        "command_type": "shell_command",
        "command_line": "rg --files crates/runtime/src crates/tools/src"
      },
      {
        "step": 2,
        "command_type": "apply_patch",
        "command_line": "*** Begin Patch\n*** Update File: crates/tools/src/command_run/handler.rs\n@@\n-    // old command handler logic\n+    // patched command handler logic\n*** End Patch"
      },
      {
        "step": 3,
        "command_type": "shell_command",
        "command_line": "cargo build -p runtime"
      },
      {
        "step": 4,
        "command_type": "shell_command",
        "command_line": "cargo test -p runtime --lib"
      },
      {
        "step": 4,
        "command_type": "shell_command",
        "command_line": "cargo clippy -p runtime --all-targets"
      }
    ]
  }
}
```

There is no ablation test proving that `command_run` alone causes Tura’s lower turn and token usage. In the matched-High DeepSWE comparison, however, Balanced used 66.8% fewer model rounds and 49.6% fewer tokens than Codex CLI High, while Direct used 84.0% fewer rounds and 83.5% fewer tokens.[[1:2]](https://turaai.net/docs#fn1)[[2:1]](https://turaai.net/docs#fn2)

### Backward Reasoning

However impressive LLMs can be, an LLM is still, at its core, a statistical induction model over text-token probabilities.

For example, asking an LLM to choose among rock, paper, and scissors does not guarantee a uniform random result. If a true one-in-three distribution matters, the choice needs an external random-number source rather than an uncited assumption about model output probabilities.

In coding tasks, this is often fatal.

An agent is more likely to execute and generate code and logic that are statistically more common. But common code and common logic are often mediocre and under-considered.

Tura uses a different strategy.

During reasoning, a common agent reasons from the current state to the prompt goal. In that case,  is the current state, and  is the goal given by the user prompt.

Instead, Tura guides the LLM to statistically estimate  first, then reason backward from the state of  to .

In the example below, the LLM can derive the optimal strategy for playing rock-paper-scissors correctly.

```
> To keep rock-paper-scissors fair and challenging,
> We need unbiased play.
> Each move must have a true one-in-three chance.
> An LLM cannot guarantee that from text probabilities alone.
> Use a random-number generator script to generate randint(1, 3)
> Then map rock, paper, or scissors to the number.
```

In programming tasks, this means that when an agent sees a goal like fixing a frontend bug, it is guided to reason through the full execution path, reconstruct the failure state, and identify the root cause before writing code. In the matched-High DeepSWE comparison, Tura Balanced passed 12 more of 60 binary task verifiers than Codex CLI High.

Both configurations in that contrast use GPT-5.6 SOL with the High reasoning label, so a High-versus-Medium effort mismatch does not explain the 20-point pass-rate difference. The result is still a system-level association, not a causal estimate for backward reasoning or any other individual feature.[[1:3]](https://turaai.net/docs#fn1)[[2:2]](https://turaai.net/docs#fn2)

### Runtime Context and Prompt Manager

Skills are often just weaker prompts loaded into context.

In many agent frameworks, a long-lived session keeps accumulating skill files, tool outputs, and stale task history. When the context becomes too large, the agent enters a separate compaction turn, but that compaction usually preserves only a compressed summary. Important execution details can become vague or lost.

Tura treats context as part of the runtime state machine.

Instead of relying on users to manually reset sessions or letting Markdown skills pile up, Tura uses `task_status`, runtime prompts, and recursive execution manuals to keep the active context scoped to the current task.

Traditional skill-based agents usually keep one session running until the user starts another, load broad Markdown skills into that session, and leave them active until a reset or compaction. Tura instead ties runtime prompts to explicit task state: sessions can be renamed, refreshed, and managed automatically; task-specific manuals and CLI commands are loaded through a recursive task tree; and irrelevant context can be removed, replaced, or compacted from the CLI. The checkpoint can retain code locations, patches, tests, and task status rather than only a loose summary. In practice, that means less stale context, lower task-scoped token cost, and fewer chances for an old skill or vague summary to steer the current job.

Because compaction is a CLI operation, Tura can preserve exact execution state in `task_status.compact_context`. In the published benchmark sessions, Tura moved beyond read-only inspection and resumed execution an average of 2.6 rounds after compaction, compared with an estimated 5.4 rounds for Codex.[[3]](https://turaai.net/docs#fn3)[[4]](https://turaai.net/docs#fn4)[[5]](https://turaai.net/docs#fn5)[[6]](https://turaai.net/docs#fn6)[[7]](https://turaai.net/docs#fn7)

Tura’s 2.6-round result is calculated from explicit `compact_context` events in its archived round contracts. Codex does not expose equivalent compaction events, so its 5.4-round result is estimated from points where input-token usage drops sharply, excluding identifiable media-reading boundaries.

### Install and run

#### NPM release

Mac and Linux:

```
npm install tura-ai
tura
```

Windows:

```
npm install -g tura-ai
tura
```

The same main package is also published to GitHub Packages as `@tura-ai/tura`. Configure the `@tura-ai` scope for `https://npm.pkg.github.com`, authenticate with a token that has `read:packages`, then install `@tura-ai/tura`. The unscoped `tura-ai` package on npm remains the simplest public installation.

Tura does not bundle provider credentials. On first launch, configure an LLM provider and select one of its models before sending a prompt. See [Provider setup](https://turaai.net/docs#start-providers) for the CLI, TUI, and GUI flows.

#### Source checkout

Windows PowerShell:

```
git clone https://github.com/Tura-AI/tura.git
cd tura
.\scripts\install.ps1
tura
```

macOS or Linux shell:

```
git clone https://github.com/Tura-AI/tura.git
cd tura
./scripts/install.sh
tura
```

The source installer performs the complete environment setup, release build, and user PATH registration flow. Pass `-EnvironmentOnly` on PowerShell or `--environment-only` on macOS/Linux only when you intentionally want dependency setup without building or registering Tura.

#### Common entrypoints

| Entry | Use it for |
| --- | --- |
| `tura` | Interactive terminal UI. |
| `tura "prompt"` | Open the TUI with an initial prompt. |
| `tura exec "prompt"` | Direct Rust CLI prompt runner. |
| `tura run "prompt"` | Gateway-backed prompt with streaming/history. |
| `tura bash`, `tura zsh`, `tura shel` | Prompt with a selected command-run shell surface. |
| `tura_gateway` | Local HTTP/SSE gateway and optional web GUI serving. |
| `tura_gui` | Desktop GUI workspace client. |

For OS-specific PATH requirements, executor installation, and how to register the CLI when the executable is not on PATH, read [How to start](https://turaai.net/docs#start-how-to-start). For command flags and modes, read [CLI parameters](https://turaai.net/docs#start-cli-parameters).

### Documentation

The GitBook-style documentation index is [docs/SUMMARY.md](https://turaai.net/docs#summary). The full navigation page is [docs/start/navigation.md](https://turaai.net/docs#start-navigation).

#### Start

*   [Overview](https://turaai.net/docs#start-overview)
*   [Install](https://turaai.net/docs#start-install)
*   [How to start](https://turaai.net/docs#start-how-to-start)
*   [CLI parameters](https://turaai.net/docs#start-cli-parameters)
*   [Settings](https://turaai.net/docs#start-settings)
*   [Providers](https://turaai.net/docs#start-providers)
*   [Sessions](https://turaai.net/docs#start-sessions)
*   [Navigation](https://turaai.net/docs#start-navigation)

#### Core

*   [Task status](https://turaai.net/docs#core-task-status)
*   [Context management](https://turaai.net/docs#core-context-management)
*   [Runtime prompt](https://turaai.net/docs#core-runtime-prompt)
*   [Command run](https://turaai.net/docs#core-command-run)
*   [Commands](https://turaai.net/docs#core-commands)
*   [Agents](https://turaai.net/docs#core-agents)
*   [Personas](https://turaai.net/docs#core-personas)
*   [Rich text](https://turaai.net/docs#core-html-rich-text)
*   [Dynamic prompt injection](https://turaai.net/docs#core-prompt-style)

#### Architecture

*   [Session DB](https://turaai.net/docs#architecture-session-db)
*   [Gateway](https://turaai.net/docs#architecture-gateway)
*   [Router](https://turaai.net/docs#architecture-router)
*   [Runtime](https://turaai.net/docs#architecture-runtime)
*   [Tool](https://turaai.net/docs#architecture-tool)
*   [Terminal user interface](https://turaai.net/docs#architecture-terminal-user-interface)
*   [Graphic user interface](https://turaai.net/docs#architecture-graphic-user-interface)

#### Customization

*   [Custom providers](https://turaai.net/docs#customization-custom-providers)
*   [Custom personas](https://turaai.net/docs#customization-custom-personas)
*   [Custom agents](https://turaai.net/docs#customization-custom-agents)
*   [Custom runtime prompt](https://turaai.net/docs#customization-custom-runtime-prompt)
*   [Custom commands](https://turaai.net/docs#customization-custom-commands)

#### Development

*   [Scripts](https://turaai.net/docs#development-scripts)
*   [Testing](https://turaai.net/docs#development-testing)
*   [Environment](https://turaai.net/docs#start-settings)
*   [Architecture](https://turaai.net/docs#development-architecture)
*   [Benchmark methodology](https://turaai.net/docs#benchmark-benchmark-methodology)
*   [Current test-set evidence record](https://turaai.net/docs#benchmark-current-test-set-record)
*   [Benchmark artifacts](https://github.com/Tura-AI/benchmark/tree/main/results)

### Contributing and project governance

Contributions should be small, reviewable, and supported by evidence at the test layer that owns the affected behavior. Choose the matching issue and pull request type rather than applying one checklist to every change.

*   [Contributing](https://turaai.net/docs#contributing) - start here for contribution types, development setup, test selection, and pull-request steps.
*   [Contribution guide](https://turaai.net/docs#contributing-guide) - test ownership, affected matrices, performance evidence, and artifact-sanitization rules.
*   [Roadmap](https://turaai.net/docs#roadmap) - current 0.1.x stabilization priorities and the planned 0.2 task-planning workspace.
*   [Known issues and evidence gaps](https://turaai.net/docs#known-issues) - open architecture, provider, benchmark, performance, and cross-OS work.
*   [Code of Conduct](https://turaai.net/docs#code-of-conduct) - community standards and the open agent-harness principle.
*   [Security policy](https://turaai.net/docs#security) - supported versions and private vulnerability reporting.
*   [Support](https://turaai.net/docs#support) - where to report bugs, request features, or ask setup and usage questions.

### License

Tura is licensed under AGPL-3.0-or-later. See [LICENSE](https://turaai.net/docs#license).

### Benchmark notes and sources

*   [Benchmark methodology](https://turaai.net/docs#benchmark-benchmark-methodology)
*   [Current test-set evidence record](https://turaai.net/docs#benchmark-current-test-set-record)
*   [Benchmark artifacts](https://github.com/Tura-AI/benchmark/tree/main/results)

* * *

1.   [`tura-benchmark` current test-set evidence record](https://turaai.net/docs#benchmark-current-test-set-record), which defines the 280-run published population, the 278-run relationship-analysis population, configuration provenance, aggregation formulas, exclusions, and identification limits. The README’s primary High-to-High tables select the 210 Tura Balanced High, Tura Direct High, and Codex CLI High sessions from that published population. [↩︎](https://turaai.net/docs#fnref1)[↩︎](https://turaai.net/docs#fnref1:1)[↩︎](https://turaai.net/docs#fnref1:2)[↩︎](https://turaai.net/docs#fnref1:3)

2.   Tura’s DeepSWE observations are in [`tura-benchmark` replicate 1](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/manifest.json), [replicate 2](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/manifest.json), and [replicate 3](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r03/manifest.json). The matched Codex CLI High observations are in [High replicate 1](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-codex-cli-high-r01/manifest.json), [High replicate 2](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-codex-cli-high-r02/manifest.json), and [High replicate 3](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-codex-cli-high-r03/manifest.json). Each configuration contributes 60 sessions on the same 20 task IDs. [↩︎](https://turaai.net/docs#fnref2)[↩︎](https://turaai.net/docs#fnref2:1)[↩︎](https://turaai.net/docs#fnref2:2)

3.   [`tura-benchmark` DynamoDB round 107 compact](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/dynamodb-toolbox-conditional-attribute-requirements/tura-balanced/dynamodb-toolbox-conditional-attribute-requirements-tura-balanced-run-01/metadata/contracts/rounds/round-0107.json) and [round 114 first later patch](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/dynamodb-toolbox-conditional-attribute-requirements/tura-balanced/dynamodb-toolbox-conditional-attribute-requirements-tura-balanced-run-01/metadata/contracts/rounds/round-0114.json). [↩︎](https://turaai.net/docs#fnref3)

4.   [`tura-benchmark` Wasmi replicate 1 round 43 compact](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-01/metadata/contracts/rounds/round-0043.json) and [round 44 first non-read action](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-01/metadata/contracts/rounds/round-0044.json). The run ends at round 46 without a later patch or test action. [↩︎](https://turaai.net/docs#fnref4)

5.   [`tura-benchmark` Wasmi replicate 2 round 26 compact](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-02/metadata/contracts/rounds/round-0026.json), [round 28 first non-read action](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-02/metadata/contracts/rounds/round-0028.json), and [round 39 first later patch/test](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-02/metadata/contracts/rounds/round-0039.json). [↩︎](https://turaai.net/docs#fnref5)

6.   [`tura-benchmark` Wasmi replicate 3 round 39 compact](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r03/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-03/metadata/contracts/rounds/round-0039.json) and [round 41 first later patch/test](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r03/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-03/metadata/contracts/rounds/round-0041.json). [↩︎](https://turaai.net/docs#fnref6)

7.   [`tura-benchmark` eza round 23 compact](https://github.com/Tura-AI/benchmark/blob/main/results/rewrite/report-20260710-gpt56-sol/eza/tura-balanced/eza-tura-balanced-gpt56-sol-run-02/metadata/contracts/rounds/round-0023.json) and [round 24 first later test](https://github.com/Tura-AI/benchmark/blob/main/results/rewrite/report-20260710-gpt56-sol/eza/tura-balanced/eza-tura-balanced-gpt56-sol-run-02/metadata/contracts/rounds/round-0024.json). [↩︎](https://turaai.net/docs#fnref7)
