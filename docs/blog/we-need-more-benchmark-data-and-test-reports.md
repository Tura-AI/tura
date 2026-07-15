# We Need More Benchmark Data and Test Reports

Tura has useful published results, but they cover a limited set of configurations and tasks.

Across the published DeepSWE runs, the same 20 tasks were repeated three times for Tura Balanced, Tura Direct, and Codex CLI. The benchmark repository also contains rewrite tasks and separately reviewed design tasks, with prompts, tool calls, token usage, patches, and verifier results.

These results support claims about those configurations. They do not establish performance across other models, reasoning levels, providers, agent architectures, repositories, frontends, or operating systems. They also do not isolate the contribution of each Tura feature.

The current scope is recorded in the benchmark repository's [current test-set record](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md). Execution and evaluation rules are in the [benchmark methodology](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md).

## Compare more reasoning levels and models

Reasoning effort and model choice can change tool usage, token consumption, latency, and correctness. The next benchmark matrix should vary both while keeping the agent, tasks, provider, timeout, environment, and evaluator fixed.

For each supported model:

- run every reasoning level the provider exposes, such as low, medium, high, or xhigh;
- record the exact model version, provider, effective reasoning parameter, date, and limits;
- repeat each cell enough times to report variance rather than one result;
- report verifier success, turns, input and output tokens, wall time, and cost;
- do not treat similarly named reasoning settings from different providers as automatically equivalent.

The matrix should include more than one model family and more than one provider protocol. Useful coverage includes OpenAI/Codex models, Anthropic/Claude, Google/Gemini, an OpenAI-compatible third party, and a local model when hardware permits. Protocol fixtures and cost-bounded smoke tests should pass before expensive long-horizon runs.

The provider and reasoning-level gaps are tracked in [Known Issues](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md#provider-and-reasoning-level-benchmark-coverage-is-narrow).

## Compare more agent architectures

Model comparisons alone cannot explain the effect of the agent harness. We also need reproducible comparisons among different execution designs, including single-action tool loops, typed micro-tools, bash-oriented agents, macro execution, and planner/executor systems.

Each comparison should publish the agent version, system prompt or equivalent configuration, tool surface, permissions, model settings, retry policy, timeout, task revision, and scoring path. Hidden differences in any of these can dominate the result.

## Run controlled ablations

The current results compare complete configurations. They do not prove that `command_run`, operation manuals, backward reasoning, or another individual feature caused the measured difference.

Useful ablation directions include:

| Experiment | Control | Candidate | What it isolates |
| --- | --- | --- | --- |
| Tura without macro execution | Tura with `command_run` | Tura with `command_run` disabled and equivalent commands executed through single-action model turns | The effect of batching and fewer model round trips inside Tura |
| `command_run` outside Tura | Original mini-swe-agent | A mini-swe-agent fork with the same `command_run` contract, permissions, and command set | Whether macro execution helps independently of Tura's other features |
| Operation manuals | Tura without task-specific operation manuals | Tura with the relevant manual enabled | The effect of task-selected execution instructions |
| Backward reasoning | A matched general engineering prompt | The same prompt plus backward-reasoning guidance | The effect of reasoning direction |
| Context management | Ordinary transcript summarization | Structured compact-context checkpoints | Long-session continuity after compaction |
| Durable task state | Chat-only continuation after interruption | Persisted task state and recovery | Recovery quality across process or session interruption |
| Preset composition | A fixed base configuration | Direct or Balanced components enabled one at a time | The contribution of each preset component |

The mini-swe-agent experiment is especially useful because it tests both directions: remove `command_run` from Tura, then add it to another small agent and compare that fork with the original. If both comparisons move in the same direction under matched conditions, the causal case is stronger.

An ablation must change one factor at a time. Keep the model version, reasoning level, provider, task revisions, permissions, timeout, machine, evaluator, and run count fixed. Repeat each cell at least three times and publish failures and excluded observations. Correctness must be reported alongside efficiency; lower token use with a lower pass rate is a trade-off, not a free improvement.

## Test the rest of the product

Agent benchmarks do not cover application startup, session recovery, process cleanup, frontend responsiveness, memory growth, or installation behavior.

TUI reports should measure startup, first render, keystroke latency, stream updates, resize behavior, session switching, memory growth, large output, Unicode, and resumed histories across representative terminals.

GUI and desktop reports should cover startup, long transcripts, streamed rich text, session switching, DOM and memory growth, dropped updates, and browser-versus-webview behavior. Timing reports must also verify that the rendered result is correct.

Cross-OS reports should include process trees, signals, sockets, file locking, PATH behavior, permissions, Unicode paths, antivirus delays, sleep and resume, abrupt process death, queue recovery, and repeated restart. Record the exact OS version, shell, path, command, exit code, and relevant log tail.

## Submit reproducible reports

Early results do not need to satisfy the complete benchmark schema. A failed run, raw output, or small fixture is useful when its scope and uncertainty are clear.

For a reproducible report, include:

- the Tura commit and agent configuration;
- the model, provider, version, reasoning level, and date;
- the operating system, hardware, shell, and relevant runtime versions;
- the exact command, workload, timeout, warm-up policy, and sample count;
- expected and observed behavior;
- correctness results and performance measurements;
- failures, retries, timeouts, and excluded observations;
- sanitized raw output or a small reproducible fixture;
- what was not tested.

Do not publish API keys, OAuth material, cookies, authorization headers, private prompts, session content, or personal filesystem paths. Raw data should be sufficient to recompute the result without exposing credentials or private data.

The complete evidence and sanitization format is in [docs/contributing-guide.md](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md#performance-and-efficiency-evidence). Test ownership and entrypoints are in [tests/README.md](https://github.com/Tura-AI/tura/blob/main/tests/README.md).

Negative and neutral results are welcome. They prevent unsupported claims and identify configurations that need work.

## Current position

Tura is not mature. Provider coverage, frontend baselines, operational testing, and controlled ablations remain incomplete.

The architecture is based on macro execution, explicit task state, task-selected instructions, durable sessions, and shared backend ownership. Those are design choices, not universal empirical results. They need testing across more reasoning levels, models, providers, architectures, frontends, and operating systems.

Open a GitHub [issue](https://github.com/Tura-AI/tura/issues) for a reproducible bug, proposal, or report that should be tracked. Early feedback can also be shared by mentioning `@tura-ai-agent` or using `#tura-ai-agent`.

The goal is to make Tura the strongest-performing open-source coding agent. The evidence must come before the claim.

## Reference documents

- [Tura README.md](https://github.com/Tura-AI/tura/blob/main/README.md) - current benchmark claims and artifacts.
- [Current test-set record](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md) - configurations and published runs.
- [Benchmark methodology](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md) - execution and evaluation rules.
- [Known Issues](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md) - open evidence gaps.
- [Roadmap](https://github.com/Tura-AI/tura/blob/main/ROADMAP.md) - stabilization priorities and exit criteria.
- [Contributing guide](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md) - evidence and sanitization requirements.

## Contact

Email: [info@turaai.net](mailto:info@turaai.net)
