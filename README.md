<p align="center">
  <a href="https://turaai.net/">
    <img src="assets/tura/icon.svg" alt="Tura icon" width="96">
  </a>
</p>

<p align="center">
  <a href="https://turaai.net/"><img alt="Website" title="Tura official website" src="https://img.shields.io/badge/Website-turaai.net-40e0d0?style=flat-square&amp;labelColor=555555"></a>
  <a href="https://turaai.net/benchmark"><img alt="Benchmark: 8,243 turns" title="Tura benchmark: 8,243 agent turns" src="https://img.shields.io/badge/Benchmark-8%2C243_turns-9b59b6?style=flat-square&amp;labelColor=555555"></a>
  <a href="https://www.npmjs.com/package/tura-ai"><img alt="npm package" title="Tura npm package" src="https://img.shields.io/npm/v/tura-ai?style=flat-square&amp;logo=npm&amp;label=npm&amp;labelColor=555555&amp;color=cb3837"></a>
</p>

<h1 align="center">Tura:77.5% fewer tokens; 16.7% better performance.</h1>

Tura is a local open-source coding agent built for developers who are tired of useless skills, extensions that claim they can save tokens, and agents that wreck repos without judgment.

Tura reduces model round trips and repeated context through its runtime and macro-command architecture. In the DeepSWE comparison, Balanced used 35.8% fewer turns and 31.1% fewer tokens than Codex CLI, while Direct used 69.1% fewer turns and 77.5% fewer tokens. Both Tura configurations ran GPT-5.6 SOL at High reasoning while still using fewer aggregate tokens than Codex CLI at Medium reasoning. Balanced prioritizes thorough investigation, implementation, and verification for higher task success; Direct follows a shorter execution path to minimize turn and token cost.[^debug-figure][^debug-manifests]

### Benchmark

Long-horizon task [benchmarks](https://turaai.net/benchmark) are one way to measure coding-agent performance beyond isolated prompts. The published comparison uses harness-based development tasks with archived prompts, per-round tool calls, token usage, patches, and verifier results.

> Across 20 DeepSWE v1.1 tasks run three times per agent, Tura first creates a substantial token-budget advantage by reducing repeated context and model round trips. Users can then choose how to spend that advantage: Direct converts most of it into lower cost, using 77.5% fewer aggregate tokens than Codex CLI while achieving a comparable verifier success rate of 65.0% versus 63.3%; Balanced reinvests part of the saved budget into deeper reasoning, investigation, and verification, reaching an 80.0% success rate—16.7 percentage points higher than Codex CLI—while still using 31.1% fewer tokens.[^debug-figure][^debug-manifests]

The published artifacts compare the named Tura Balanced, Tura Direct, and Codex CLI configurations on the same benchmark tasks.[^debug-figure]

The public [current test-set record](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md) gives the full evidence ledger: acquisition and storage, cohort alignment, retained Tura timeouts and severe long tails, the High-versus-Medium rationale, prompt-generation drift, compact-context and other missing ablations, and the next controlled experiments. It also audits eight same-model, same-High-effort design runs. Across those runs, Tura Direct used 43.6% fewer tokens and 24.1% fewer turns while recording 264 tool actions versus Codex CLI's 43. In the squid decks, all 20 Tura video links are resolvable, specific YouTube pages and 18/20 have exact-dish titles; all 20 Codex links are search-result pages, and 60% of Codex recipe citations are searches or broad indexes. In the Paris task, the public HTML artifacts expose the reported Codex angle/layer problems, while Tura's contracts record real-browser WebGL checks and inspected captures at 1440, 768, and 390 pixels.[^test-set-record]

<details>
<summary><strong>FULL BENCHMARK REPORT</strong></summary>

<p align="center">
  <img src="assets/data/benchmark-agent-comparison.svg" alt="DeepSWE Debug and Rewrite Repo benchmark comparison" width="800">
</p>

<p align="center"><em>Harness success and aggregate token usage across 25 high-difficulty tasks, 6 agent-and-model configurations, and 270 sessions. Source and calculation notes are linked below.</em></p>
</details>

### Screenshots

<p align="center">
  <img src="assets/screenshot/gui-ci-quality-demo.svg" alt="Tura GUI" width="800">
</p>

<p align="center"><em>GUI page with multi-session concurrent work and HTML rich text support.</em></p>

<p align="center">
  <img src="assets/screenshot/tui-ci-quality-demo.svg" alt="Tura TUI" width="800">
</p>

<p align="center"><em>TUI page with multi-session concurrent work and HTML rich text support.</em></p>

The results below are grounded in published benchmark artifacts rather than an uncited aggregate. Tura is built around three core systems:

## Macro CLI Command Run

Most coding agents still depend on repetitive tool-calling loops: inspect, wait, patch, wait, build, wait, test, wait.

_**Tool-calling coding agent:**_

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

_**Tura macro CLI command:**_

```json
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

There is no ablation test proving that `command_run` alone causes Tura's lower turn and token usage. Across the full DeepSWE comparison, however, Balanced used 35.8% fewer turns and 31.1% fewer tokens than Codex CLI, while Direct used 69.1% fewer turns and 77.5% fewer tokens.[^debug-figure][^debug-manifests]

## Backward Reasoning

However impressive LLMs can be, an LLM is still, at its core, a statistical induction model over text-token probabilities.

For example, asking an LLM to choose among rock, paper, and scissors does not guarantee a uniform random result. If a true one-in-three distribution matters, the choice needs an external random-number source rather than an uncited assumption about model output probabilities.

In coding tasks, this is often fatal.

An agent is more likely to execute and generate code and logic that are statistically more common. But common code and common logic are often mediocre and under-considered.

Tura uses a different strategy.

During reasoning, a common agent reasons from the current state to the prompt goal. In that case, $s_1$ is the current state, and $s_n$ is the goal given by the user prompt.

$$
s_1 \rightarrow s_2 \rightarrow s_3 \rightarrow \cdots \rightarrow s_n
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

In programming tasks, this means that when an agent sees a goal like fixing a frontend bug, it is guided to reason through the full execution path, reconstruct the failure state, and identify the root cause before writing code. In the published DeepSWE comparison, Tura Balanced passed 10 more of 60 binary task verifiers than Codex CLI.

On the same 20-task subset, DeepSWE’s official mini-swe-agent results show an 8% gap between GPT-5.6 SOL High and Medium reasoning, while Tura Balanced leads Codex CLI by 16.7%. This indicates that higher reasoning effort alone does not explain Tura’s advantage.[^debug-manifests][^rewrite-manifest]

## Runtime Context and Prompt Manager

Skills are often just weaker prompts loaded into context.

In many agent frameworks, a long-lived session keeps accumulating skill files, tool outputs, and stale task history. When the context becomes too large, the agent enters a separate compaction turn, but that compaction usually preserves only a compressed summary. Important execution details can become vague or lost.

Tura treats context as part of the runtime state machine.

Instead of relying on users to manually reset sessions or letting Markdown skills pile up, Tura uses `task_status`, runtime prompts, and recursive execution manuals to keep the active context scoped to the current task.

| Area                  | Traditional Skill-Based Agent                                            | Tura                                                                  |
| --------------------- | ------------------------------------------------------------------------ | --------------------------------------------------------------------- |
| Session model         | Same session keeps running until the user manually starts a new one      | Session state can be renamed, refreshed, and managed automatically    |
| Skill loading         | Loads Markdown skill files into context                                  | Dynamically loads task-specific runtime prompts and execution manuals |
| Prompt strength       | Skills behave like weak contextual instructions or tool output           | Runtime prompts are tied to the active task state                     |
| Context pollution     | Old skills and irrelevant context remain active until compacted or reset | Irrelevant context can be removed, compacted, or replaced             |
| Compaction            | Separate long compaction turn                                            | Compaction is handled as a CLI operation                              |
| Information preserved | Usually compressed summaries only                                        | Code locations, patches, tests, and task status can be preserved      |
| Token cost            | High because stale context stays active                                  | Lower because context is task-scoped                                  |
| Failure mode          | Agent mixes old tasks, vague summaries, and irrelevant skills            | Agent keeps execution aligned with the current task                   |
| Tool/manual loading   | Broad skills are loaded even when only part is useful                    | CLI commands and manuals are loaded through a recursive task tree     |

Because compaction is a CLI operation, Tura can preserve exact execution state in `task_status.compact_context`. In the published benchmark sessions, Tura moved beyond read-only inspection and resumed execution an average of 2.6 rounds after compaction, compared with an estimated 5.4 rounds for Codex.[^compact-dynamodb][^compact-wasmi-r1][^compact-wasmi-r2][^compact-wasmi-r3][^compact-eza]

Tura's 2.6-round result is calculated from explicit `compact_context` events in its archived round contracts. Codex does not expose equivalent compaction events, so its 5.4-round result is estimated from points where input-token usage drops sharply, excluding identifiable media-reading boundaries.

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

| Entry                                | Use it for                                           |
| ------------------------------------ | ---------------------------------------------------- |
| `tura`                               | Interactive terminal UI.                             |
| `tura "prompt"`                      | Open the TUI with an initial prompt.                 |
| `tura exec "prompt"`                 | Direct Rust CLI prompt runner.                       |
| `tura run "prompt"`                  | Gateway-backed prompt with streaming/history.        |
| `tura bash`, `tura zsh`, `tura shel` | Prompt with a selected command-run shell surface.    |
| `tura_gateway`                       | Local HTTP/SSE gateway and optional web GUI serving. |
| `tura_gui`                           | Desktop GUI workspace client.                        |

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
- [Benchmark methodology](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md)
- [Current test-set evidence record](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md)
- [Benchmark artifacts](https://github.com/Tura-AI/benchmark/tree/main/results)

## License

Tura is licensed under AGPL-3.0-or-later. See [LICENSE](LICENSE).

## Benchmark notes and sources

The benchmark's scope, selection rules, scoring boundaries, invalid-run policy, and reporting protocol live in the benchmark repository's [methodology](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md). The [current test-set evidence record](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md) is the claim ledger for the July 2026 artifacts. It documents the raw-to-normalized-to-published data path, same-cohort guarantees, physical continuation batches, the mid-run prompt revision, retained failures and long tails, design observations, and missing ablations.[^test-set-record]

The DeepSWE headline compares named system configurations: Tura Balanced and Tura Direct GPT-5.6 SOL High against Codex CLI GPT-5.6 SOL Medium. It is not a controlled effort ablation. "Success" is the official binary verifier result over 60 published sessions per agent. The three [replicate manifests](https://github.com/Tura-AI/benchmark/tree/main/results/debug) retain the same 20 task IDs and expose each task, agent, effort, replicate, status, round count, and artifact path.[^debug-manifests]

The published DeepSWE claims can be checked directly:

- Codex CLI passed `38/60` (`63.3%`), using `333,538,349` tokens and `3,140` rounds.
- Tura Balanced passed `48/60` (`80.0%`), using `229,695,477` tokens and `2,017` rounds: 10 additional passes, 31.1% fewer tokens, and 35.8% fewer rounds than Codex.
- Tura Direct passed `39/60` (`65.0%`), using `75,108,167` tokens and `969` rounds: one additional pass, 77.5% fewer tokens, and 69.1% fewer rounds than Codex.
- The worst Balanced observation was not trimmed: it reached `35,464,917` tokens and `242` rounds before timeout. Eight Balanced agent executions and one Codex execution have timeout/non-zero outcomes in the published source summaries.
- A later batch introduced a recorded TDD-oriented prompt revision. The final 180 observations remain task/model/environment/verifier aligned, but they are not one immutable prompt-version cohort. This prevents attributing the aggregate result to the prompt change or to any single runtime feature.

The design/front-end evidence is a smaller process and artifact audit: two tasks, two agents, and two replicates, with both agents on GPT-5.6 SOL High. Across those eight runs, Tura Direct used 43.6% fewer tokens and 24.1% fewer turns while recording 264 tool actions versus Codex CLI's 43. Tool-action counts are not a quality score; they show that lower model usage coexisted with more archived source discovery, media inspection, link probing, browser checks, and responsive verification.[^test-set-record]

The design claims are also inspectable rather than decorative. In the two squid runs per agent, all 20 Tura video destinations are resolvable, specific YouTube pages and 18/20 have titles matching the exact displayed dish; the other two are neighboring method evidence. Tura likewise has 18/20 exact-dish recipe pages and two method-level sources. All 20 Codex video destinations are YouTube search-result pages, and 12/20 Codex recipe citations are search or broad-index pages. For Paris, the evidence record links all four HTML artifacts so readers can inspect the reported Codex angle and layer-order problems. Tura's published contracts additionally record Playwright screenshots at 1440, 768, and 390 pixels, three `read_media` inspections, WebGL and console checks, and interaction assertions. The screenshots remained in ignored raw storage, so the durable public evidence is the contract plus the final HTML; publishing both agents' captures is an explicit next step.[^test-set-record]

No feature-level causal claim is made. There is no completed ablation for `command_run`, `compact_context`, backward-reasoning instructions, operation-manual loading, prompt generation, or reasoning effort. Compact-context continuation is asymmetric observational evidence because Tura emits explicit events while Codex comparison points are inferred from token drops. The planned controls are a frozen-prompt rerun, a crossed agent x Medium/High matrix, one-feature-at-a-time ablations, paired uncertainty analysis, deterministic design integrity checks, and blinded multi-reviewer design scoring.[^test-set-record]

Rewrite success remains separately defined as `sum(passed) / sum(total)` over the canonical manifest; it is not averaged across run percentages.[^rewrite-manifest]

[^debug-figure]: [DeepSWE and Rewrite Repo comparison figure](assets/data/benchmark-agent-comparison.svg). The figure states the task, session, verifier, turn, token, and aggregation scopes used by the README.

[^test-set-record]: [`tura-benchmark` current test-set record](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md), including direct links to all eight published design HTML artifacts and their run contracts.

[^debug-manifests]: [`tura-benchmark` DeepSWE replicate 1](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/manifest.json), [replicate 2](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/manifest.json), and [replicate 3](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r03/manifest.json). Each manifest contains 20 tasks across the same three agent configurations; together they contain 180 sessions.

[^rewrite-manifest]: [`tura-benchmark` GPT-5.6 Rewrite Repo canonical manifest](https://github.com/Tura-AI/benchmark/blob/main/results/rewrite/report-20260710-gpt56-sol/canonical-manifest.json). The cited totals are Tura Balanced 389/472 and Codex CLI 351/472 across 10 sessions each.

[^compact-dynamodb]: [`tura-benchmark` DynamoDB round 107 compact](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/dynamodb-toolbox-conditional-attribute-requirements/tura-balanced/dynamodb-toolbox-conditional-attribute-requirements-tura-balanced-run-01/metadata/contracts/rounds/round-0107.json) and [round 114 first later patch](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/dynamodb-toolbox-conditional-attribute-requirements/tura-balanced/dynamodb-toolbox-conditional-attribute-requirements-tura-balanced-run-01/metadata/contracts/rounds/round-0114.json).

[^compact-wasmi-r1]: [`tura-benchmark` Wasmi replicate 1 round 43 compact](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-01/metadata/contracts/rounds/round-0043.json) and [round 44 first non-read action](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r01/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-01/metadata/contracts/rounds/round-0044.json). The run ends at round 46 without a later patch or test action.

[^compact-wasmi-r2]: [`tura-benchmark` Wasmi replicate 2 round 26 compact](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-02/metadata/contracts/rounds/round-0026.json), [round 28 first non-read action](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-02/metadata/contracts/rounds/round-0028.json), and [round 39 first later patch/test](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r02/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-02/metadata/contracts/rounds/round-0039.json).

[^compact-wasmi-r3]: [`tura-benchmark` Wasmi replicate 3 round 39 compact](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r03/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-03/metadata/contracts/rounds/round-0039.json) and [round 41 first later patch/test](https://github.com/Tura-AI/benchmark/blob/main/results/debug/report-deepswe-v1.1-gpt56-sol-local-r03/wasmi-trap-coredumps/tura-balanced/wasmi-trap-coredumps-tura-balanced-run-03/metadata/contracts/rounds/round-0041.json).

[^compact-eza]: [`tura-benchmark` eza round 23 compact](https://github.com/Tura-AI/benchmark/blob/main/results/rewrite/report-20260710-gpt56-sol/eza/tura-balanced/eza-tura-balanced-gpt56-sol-run-02/metadata/contracts/rounds/round-0023.json) and [round 24 first later test](https://github.com/Tura-AI/benchmark/blob/main/results/rewrite/report-20260710-gpt56-sol/eza/tura-balanced/eza-tura-balanced-gpt56-sol-run-02/metadata/contracts/rounds/round-0024.json).
