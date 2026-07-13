# We Need More Benchmark Data and Test Reports

*Written July 13, 2026.*

Tura has benchmark results I am happy to publish. It also has a much larger pile of questions those results do not answer.

Both statements can be true.

The current evidence is useful. Across the published DeepSWE runs, the same 20 tasks were repeated three times for Tura Balanced, Tura Direct, and Codex CLI. There are also rewrite tasks and separately reviewed design tasks, with prompts, tool calls, token usage, patches, and verifier results archived in the benchmark repository.

That is real evidence, not a screenshot with a number floating above it. It tells us something about those named configurations on those tasks.

It does not tell us that Tura wins with every model, every reasoning level, every provider, every repository, or every operating system. It does not tell us which Tura feature caused how much of the result. And it says very little about whether the GUI stays responsive after six hours on a machine I do not own.

So the next useful contribution may not be another feature. It may be a careful test report.

## The current benchmark is a starting point

The headline results are easy to repeat: in the published comparison, Direct used fewer aggregate tokens than Codex CLI at a comparable verifier success rate, while Balanced spent more of the saved budget on investigation and verification and reached a higher success rate on the tested set.

The important words are "published comparison" and "tested set."

Most of the long-horizon evidence is currently centered on GPT-5.6 SOL and Codex/OpenAI-style configurations. The repository contains fixtures and configuration for more provider families, but a configuration file is not a benchmark result. A mocked request is not proof that a real provider streams, retries, reports usage, and handles tool calls correctly under a long job.

The complete current scope is recorded in the benchmark repository's [current test-set record](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md). The rules used to produce and judge those runs are in the full [benchmark methodology](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md).

I trust the current results within that boundary. I do not want to quietly move the boundary after reading the result.

## We need a matrix, not one taller leaderboard

Running more tasks is useful, but task count is only one axis. Coding-agent performance is a product of at least the model, reasoning setting, provider protocol, harness, task, environment, and budget. Change several at once and the final score may be interesting, but it becomes difficult to explain.

The comparison I want to see crosses three large dimensions.

### More reasoning levels

The same model can behave very differently at low, medium, and high reasoning effort. It may make fewer calls, spend more tokens inside each call, investigate more deeply, or simply take longer to reach the same answer.

We should run matched tasks across reasoning levels while keeping the agent architecture, model version, provider, timeout, and evaluation fixed. Then repeat the same reasoning-level sweep for more than one agent. Otherwise we cannot tell whether a gain comes from the harness, from extra inference, or from an interaction between the two.

A single high-versus-medium number is a clue. A repeated cross-agent matrix with variance is evidence.

### More model providers

Tura needs repeated long-horizon runs for Anthropic/Claude, Google/Gemini, at least one OpenAI-compatible third party, and a local endpoint when hardware permits. Those runs should record exact model versions, dates, settings, cost, failures, and raw sanitized artifacts.

This is not only a leaderboard question. Providers disagree in the details: streaming events, parallel tool calls, reasoning metadata, prompt caching, usage accounting, retries, cancellation, and malformed responses. An agent can look excellent against a fixture and still fail because one live stream ends differently.

Cost matters here too. Live matrices can become expensive quickly. A sensible sequence is protocol fixtures first, cost-bounded smoke tests second, and repeated long-horizon runs only where the earlier layers are stable.

The provider gap and the evidence needed to close it are documented in [KNOWN_ISSUES.md](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md#provider-benchmark-coverage-is-narrow).

### More agent architectures

Comparing Tura only with one CLI would tell us too little. We need agents with materially different execution designs: typed micro-tools, a bash-only harness, macro execution, planner-and-executor systems, and multi-agent or delegated workflows where a reproducible public configuration exists.

The hard part is fairness. "Same model" is not enough if one agent gets a different reasoning budget, hidden system prompt, timeout, retry policy, or task patch. Each report should identify the agent version, public configuration, tool surface, model settings, task revision, limits, and scoring path.

I am less interested in manufacturing a universal winner than in learning where each architecture bends. Maybe macro execution helps most on tool-heavy repair work. Maybe another design is better for short edits. Maybe the ranking changes completely with a different provider. Those are useful answers.

## We need to ablate Tura itself

The current results compare complete configurations. They do not prove that one named feature caused the difference.

The project README is explicit about this: there is currently no ablation proving that `command_run` alone causes the lower turn and token usage. That caveat should become an experiment, not slowly disappear from the story.

Useful Tura ablations include:

- macro `command_run` batching versus equivalent commands executed through single-action model turns;
- task-specific operation manuals versus the same agent without those runtime instructions;
- backward-reasoning guidance versus a matched general engineering prompt;
- structured compact-context checkpoints versus ordinary transcript summarization on long sessions;
- explicit durable task state versus chat-only continuation after interruption;
- Direct and Balanced components separated one at a time, rather than treating two full presets as a clean causal comparison.

Each ablation should change one thing, keep the rest fixed, repeat enough times to show variance, and report correctness as well as tokens, turns, latency, and cost. If disabling a feature saves tokens but breaks more tasks, that is not a free improvement. If a feature helps only after context grows past a certain size, that boundary is more useful than one aggregate average.

Some of these experiments will be awkward to build. Good. If a feature cannot be isolated, we should be cautious about assigning it a percentage of the credit.

## One benchmark case I would like to see

Here is a concrete example. Take a small fixed set of long-horizon repair tasks and run the same model through four cells:

| Cell | Agent setup | Reasoning level | What it helps isolate |
| --- | --- | --- | --- |
| A | Tura Balanced | Medium | Tura's full reasoning-oriented configuration |
| B | Tura Balanced | High | The effect of additional reasoning within the same architecture |
| C | Tura Direct | Medium | The difference between Tura's two named configurations |
| D | Another reproducible agent | Medium | An architectural comparison at a matched reasoning level |

Keep the task revisions, model version, provider, timeout, machine, evaluation, and run count fixed. Repeat each cell at least three times. Record verifier success, turns, input and output tokens, wall time, cost, failures, and the raw run artifacts.

Then run one narrower Tura ablation on the same tasks: macro `command_run` batching enabled versus equivalent single-action model turns. That does not answer every question, but it starts separating the effect of reasoning effort, preset, agent architecture, and one specific Tura feature.

This is only one useful case. A provider matrix, a GUI memory-growth report, or a Windows process-cleanup failure can be equally valuable. If you have only one cell, one task, or one failed run, send that too. Partial evidence is how a complete benchmark often begins.

## Agent benchmarks do not test the whole product

A coding benchmark can finish with a correct patch while the local product still leaks memory, loses a session, hangs during shutdown, or renders a transcript badly. The verifier does not care. The user does.

Tura already has backend, GUI, TUI, performance, OS, release, and live test suites. That does not make those surfaces finished. It gives us somewhere to put the next regression.

For the TUI, we need broader measurements for startup, first render, keystroke latency, stream updates, resizing, session switching, memory growth, large tool output, Unicode, concurrent sessions, and resumed histories across representative terminals.

For the GUI and packaged desktop app, we need repeatable reports for startup, long transcripts, streamed rich text, session switching, plan scale, calendar navigation, DOM growth, memory retention, dropped updates, and differences between browser and desktop webviews.

A frontend report should include behavior correctness, not only timing. A fast blank transcript is technically fast and otherwise unhelpful.

The open TUI and GUI baselines are listed in the complete [Known Issues and Evidence Gaps](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md) document.

## The operating system always gets a vote

Tura manages processes, sockets, file locks, shell profiles, PATH entries, local databases, desktop dependencies, and child-process cleanup. These are exactly the things that look portable until they meet a second operating system.

The source installer already runs in CI on Ubuntu, macOS, Windows Server 2022, and Windows Server 2025. That is valuable coverage. It is not the end of the story.

We still need lifecycle and soak reports around signals, process trees, antivirus delays, permissions, path separators, spaces and Unicode in paths, sleep and resume, abrupt process death, queue recovery, repeated restart, and old session data. Linux, macOS, and Windows each have their own way of making a reasonable process assumption look naive.

When you find one of these failures, please keep the ugly details. The exact OS version, shell, filesystem path, command, exit code, and relevant log tail are often the difference between a fix and a ghost story.

## What a useful report can look like

You do not need to produce a paper, and you do not need to make every submission conform 100% to the benchmark contract or report schema. Those contracts matter when we turn a result into a reproducible public claim or merge it as a canonical benchmark artifact. They are not a ticket you must present before telling us something useful.

You can send raw output. You can send one failed run. You can describe a benchmark case you think we should add. You can report a bug without knowing its root cause. You can also send an unfinished idea or simply tell us that a workflow felt wrong. Please label what you know, what you do not know, and whether the material has been sanitized. We can help turn a promising observation into a structured case later.

When you do have enough information for a reproducible benchmark or test report, the following details make it much easier for another person to check:

For a benchmark or test report, include:

- the Tura commit and agent configuration;
- the model, provider, model version, reasoning level, and date;
- the operating system, hardware, shell, and relevant runtime versions;
- the exact command, workload, timeout, warm-up policy, and sample count;
- expected behavior and observed behavior;
- correctness results alongside latency, token, memory, or cost measurements;
- failures, retries, timeouts, and any excluded observation;
- sanitized raw output or a small reproducible fixture;
- what was not tested.

Do not publish API keys, OAuth material, cookies, authorization headers, private prompts, session content, or personal filesystem paths. "Raw data" means enough data to recompute the result, not a ceremonial dumping of secrets.

The repository's complete format for performance evidence and sanitization is in [docs/contributing-guide.md](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md#performance-and-efficiency-evidence). Use it when you are making a formal performance claim or preparing a canonical artifact; do not treat it as a reason to withhold an early result. General test ownership and entrypoints are in [tests/README.md](https://github.com/Tura-AI/tura/blob/main/tests/README.md).

Negative results are welcome. If a comparison shows no meaningful difference, that saves someone else from building a claim around noise. If Tura loses badly on a model or OS, that is not disloyal data. It is a map.

## Tura is not mature

There are parts of Tura I use and believe in. There are also unfinished product paths, narrow evidence, provider gaps, frontend baselines we have not established, and OS behavior that needs more hostile testing.

Calling it mature would make the project sound safer for about five minutes and make it harder to improve for much longer.

I believe Tura will change how later coding agents are architected. Macro execution reduces unnecessary model round trips. Explicit task state makes long work less dependent on a chat transcript. Runtime-selected instructions avoid loading every rule into every task. Durable sessions and a shared backend let different frontends continue the same work instead of creating parallel agents.

That is a design argument and a personal conviction. It is not yet a universal empirical result.

The way to make the argument stronger is not to repeat it louder. It is to test Tura with more models, more reasoning levels, more providers, more agent architectures, cleaner ablations, more frontends, and more operating systems—and to publish the runs that surprise us.

## Send me what you find

Formal GitHub [issues](https://github.com/Tura-AI/tura/issues) are welcome. They are the best place for a reproducible bug, a proposal that needs discussion, or a report with artifacts we should track to completion.

But an issue is not the only door. You do not have to wait until you have a polished benchmark report or a schema-complete submission. If Tura behaves strangely, performs unexpectedly well, loses badly, breaks on your operating system, or simply makes a workflow more annoying than it should be, share it on any social platform and mention `@tura-ai-agent` or use `#tura-ai-agent`.

I watch both. Write down your comment in plain language and include whatever you have: an idea, a screenshot, raw output, a command, an error message, or just the situation that felt wrong. They give me a practical way to find feedback outside the repository and turn an isolated observation into a reproducible issue, a test, or a better benchmark case.

The goal is ambitious: I want to make Tura the strongest-performing open-source coding agent.

We will not get there by calling it the strongest before the evidence exists. We get there by collecting honest reports, testing the architecture from angles I have missed, and improving the parts that fail in public.

## The formal documents

This post is the conversational request for evidence. These complete documents define the current scope and reporting rules:

- [Tura README.md](https://github.com/Tura-AI/tura/blob/main/README.md) — current benchmark claims, scopes, caveats, and artifact links.
- [Current test-set record](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md) — the benchmark configurations and published artifacts currently in scope.
- [Benchmark methodology](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md) — execution, evaluation, normalization, and reporting rules.
- [docs/KNOWN_ISSUES.md](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md) — provider, runtime, frontend, operational, and cross-OS evidence gaps.
- [ROADMAP.md](https://github.com/Tura-AI/tura/blob/main/ROADMAP.md) — stabilization priorities and exit criteria.
- [docs/contributing-guide.md](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md) — test ownership, benchmark evidence, and artifact sanitization.
- [tests/README.md](https://github.com/Tura-AI/tura/blob/main/tests/README.md) — test layers, locations, and runner conventions.

## Contact

Email: [info@turaai.net](mailto:info@turaai.net)
