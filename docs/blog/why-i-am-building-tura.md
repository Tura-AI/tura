# Why I'm Building Tura

Coding-agent plugins and skills often promise lower token use, better context, stronger planning, or higher task success. Those claims should be tested on repeatable coding tasks with prompts, outputs, correctness, cost, and failures available for review.

I started Tura to test whether changing the architecture around the model could produce measurable improvements on real work.

## Reduce unnecessary model round trips

Many agent loops ask the model to select one small tool action, wait for the result, and repeat. This increases latency, repeats context, and makes long tasks more sensitive to interruptions.

Tura exposes one model-facing execution tool, `command_run`. It groups related commands into explicit dependency steps: independent reads can run together, edits wait for discovery, and builds or tests run after edits. Commands remain structured, mutating operations are barriers, and permissions, file locks, cancellation, and output limits still apply.

The design and safety boundaries are documented in [Why Tura Uses One Tool](https://github.com/Tura-AI/tura/blob/main/docs/core/command-run.md#why-tura-uses-one-tool) and the complete [command-run documentation](https://github.com/Tura-AI/tura/blob/main/docs/core/command-run.md).

## Use the saved budget deliberately

Fewer model round trips can reduce repeated context and token use. Tura provides two configurations for using that budget:

- Direct keeps execution lean.
- Balanced spends more on investigation, reasoning, and verification.

In the published DeepSWE comparison, Direct used 77.5% fewer aggregate tokens than Codex CLI with a comparable verifier success rate. Balanced used 31.1% fewer tokens and reached a higher success rate on that test set.

These are bounded results for named configurations. They do not establish the same outcome for every model, reasoning level, provider, operating system, or repository. The current claims and artifacts are in the [README](https://github.com/Tura-AI/tura/blob/main/README.md), and open gaps are in [Known Issues](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md).

We still need cross-model and cross-reasoning-level comparisons, different agent architectures, and controlled ablations. The proposed work is listed in [We Need More Benchmark Data and Test Reports](https://github.com/Tura-AI/tura/blob/main/docs/blog/we-need-more-benchmark-data-and-test-reports.md).

## Keep context task-specific

Loading every skill and instruction into every turn wastes context and can introduce conflicting guidance. Tura keeps explicit task state and loads the operation manual and capabilities relevant to the current work.

When a long session needs compaction, Tura stores a structured checkpoint of active work so execution can continue after context is reduced. The retained state and rebuilding process are documented in [Context Management](https://github.com/Tura-AI/tura/blob/main/docs/core/context-management.md).

## Make sessions durable

Development work is interrupted, resumed, and opened from different interfaces. Tura stores sessions, messages, task state, todos, and workspace history as durable data. The session database has one owner, while the CLI, TUI, GUI, and desktop shell use the same backend path.

This requires explicit recovery, process ownership, compatibility, and state-transition rules. The current boundaries are documented in [ARCHITECTURE.md](https://github.com/Tura-AI/tura/blob/main/ARCHITECTURE.md).

## Keep benchmark logic inspectable

Agent results depend on prompts, tool contracts, runner behavior, scoring rules, and failure classification as well as the model. Project-controlled logic needed to reproduce a public claim should therefore be inspectable. Performance contributions should include sanitized raw evidence sufficient to verify the claim.

## Stabilize before expanding

Tura is not mature. Current 0.1.x work focuses on installation, session persistence, recovery, process cleanup, provider evidence, cross-OS behavior, and repeatable performance baselines. Speculative features and abstractions should wait for demonstrated requirements.

The current priorities and exit criteria are in [ROADMAP.md](https://github.com/Tura-AI/tura/blob/main/ROADMAP.md).

I believe macro execution, explicit task state, selective runtime instructions, durable sessions, and shared backend ownership are useful directions for coding agents. The project still needs broader evidence to establish where they help and what they cost.

## Why Tura

I want a coding agent that reads before editing, groups related work, survives context changes, verifies claims at the owning layer, and states what it did not test. Performance claims should remain close to reproducible evidence.

The goal is to make Tura the strongest-performing open-source coding agent.

## Reference documents

- [README.md](https://github.com/Tura-AI/tura/blob/main/README.md) - product scope and benchmark results.
- [ARCHITECTURE.md](https://github.com/Tura-AI/tura/blob/main/ARCHITECTURE.md) - process, runtime, session, provider, and tool ownership.
- [ROADMAP.md](https://github.com/Tura-AI/tura/blob/main/ROADMAP.md) - priorities and exit criteria.
- [Command Run](https://github.com/Tura-AI/tura/blob/main/docs/core/command-run.md) - macro execution and safety boundaries.
- [Context Management](https://github.com/Tura-AI/tura/blob/main/docs/core/context-management.md) - checkpoints and compaction.
- [Known Issues](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md) - known limitations and evidence gaps.

## Why the name Tura?

The name comes from the Sanskrit word *tura* (तुर), which can mean quick, swift, strong, or excelling. The dictionary entries are collected [here](https://kosha.sanskrit.today/word/sa/tura).

## Contact

Email: [info@turaai.net](mailto:info@turaai.net)
