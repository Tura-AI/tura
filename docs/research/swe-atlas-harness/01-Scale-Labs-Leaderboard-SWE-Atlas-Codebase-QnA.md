# Scale Labs Leaderboard: SWE Atlas - Codebase QnA

Source: https://labs.scale.com/leaderboard/sweatlas-qna

[![Scale Labs](https://labs.scale.com/images/logo-scale-labs.svg?v=2)](https://labs.scale.com/)[[PAPERS]](https://labs.scale.com/papers)[[BLOG]](https://labs.scale.com/blog)[LEADERBOARDS][[SHOWDOWN]](https://labs.scale.com/showdown)⌘K⌘KAgentic[DrugDiscoveryBench](https://labs.scale.com/leaderboard/drugdiscoverybench)[SWE Atlas - Refactoring](https://labs.scale.com/leaderboard/sweatlas-refactoring)[SWE Atlas - Test Writing](https://labs.scale.com/leaderboard/sweatlas-tw)[SWE Atlas - Codebase QnA](https://labs.scale.com/leaderboard/sweatlas-qna)[HiL-Bench (Human-in-Loop Benchmark)](https://labs.scale.com/leaderboard/hil)[MCP Atlas](https://labs.scale.com/leaderboard/mcp_atlas)[SWE-Bench Pro (Public Dataset)](https://labs.scale.com/leaderboard/swe_bench_pro_public)[SWE-Bench Pro (Private Dataset) ](https://labs.scale.com/leaderboard/swe_bench_pro_private)[Remote Labor Index (RLI)](https://labs.scale.com/leaderboard/rli)SafetyFrontierLegacy2025 Scale AI. All rights reserved. # SWE Atlas - Codebase QnA

Evaluating deep code comprehension and reasoning

[Open-Source Dataset](https://github.com/scaleapi/SWE-Atlas)[View paper](https://labs.scale.com/papers/sweatlas)### Overview

SWE Atlas is a benchmark for evaluating AI coding agents across a spectrum of professional software engineering tasks. Rather than measuring a single skill in isolation, SWE Atlas consists of three leaderboards that target distinct and complementary capabilities:

1. **Codebase QnA** - Understand complex codebases through runtime analysis and multi-file reasoning
2. **Test Writing** - Write meaningful tests that exercise real functionality to increase code coverage
3. **Refactoring** - Restructure code to improve readability & maintainability while preserving behavior

We are releasing results for **Codebase QnA**, the first benchmark in the SWE Atlas suite, with additional results for Test Writing and Refactoring to come.

### Codebase QnA

Codebase QnA consists of 124 tasks that targets the upstream capability of Software Engineering - deep code comprehension that precedes any code change.

The agent is given access to a well-known, public repository inside a Docker container and must answer a deeply technical set of questions about how the system works. Questions require Agentic Reasoning by design - they require running the software, tracing execution across multiple files, and synthesizing findings. Simple codebase exploration is insufficient to solve these.

The benchmark consists of tasks drawn from 11 production repositories across 4 programming languages – Go, Python, C, and TypeScript. Top models achieve a 30% pass rate (at the strictest rubric threshold of 1.0), indicating substantial room for improvement.

See the full dataset [here](https://huggingface.co/datasets/ScaleAI/SWE-Atlas-QnA).

## Methodology

Each QA task is constructed through a multi-stage human-in-the-loop pipeline. ** **

**Repository Selection.** Repositories are selected from [SWE-Bench Pro](https://scale.com/leaderboard/swe_bench_pro_public), and represent real-world software complexity: mail servers, terminal emulators, object storage systems, observability platforms, secret scanners, etc. These are large, actively maintained open-source codebases with non-trivial architectures. They are also contamination-resistant, using strong copyleft licenses (e.g., GPL).

**Environment Construction.** For each repository, engineers build a reproducible Docker image pinned to a specific commit, with all dependencies pre-installed, such that the software can be built, run, and tested.

**Question Authoring. **Professional software engineers and technical experts with significant coding and agentic experience write problem statements that require multi-step reasoning across the codebase. Experts first spend significant time familiarizing themselves with each repository's functionality, implementation, and edge cases before authoring questions. Questions are written in natural language and intentionally underspecified to challenge agents' autonomous exploration capabilities.

We emphasize that task prompts be written in natural language to emulate how they interact with coding agents like Claude Code or Cursor, and are, by nature, underspecified. Agents would need to autonomously explore the codebase, set up the application, run it with real data, understand the flow and prepare a detailed response.

Tasks fall into the following categories (Example prompts shortened for visual clarity):

1. **Architecture & system design (35%):** Questions about how a system's components are structured and how they interact.

Example:  "When maddy is configured with a small max_tries and pointed at a non-responsive SMTP destination that times out, what exact sequence of connection attempts occurs? How long does each timeout take, and what log entries mark each retry attempt versus the final bounce decision?"
2. **Root-cause analysis (30%):** Questions that present confusing or seemingly broken behavior and ask the agent to determine why.

Example: "I'm sending probes to different gateways with Scapy's sr1, but responses from one gateway get matched to probes sent to another. I've verified with tcpdump that the packets on the wire are correct - so why is Scapy pairing them wrong?”
3. **Code Onboarding (23%):** Questions a new engineer would ask when getting oriented in an unfamiliar codebase.

Example: "I am joining a team that relies heavily on Scapy for custom packet manipulation and I need to understand what the runtime environment actually looks like. When you construct a basic ICMP ping packet, what's the actual structure of that packet object? I'm trying to understand whether Scapy creates a single composite object or something else - what layer types are involved, and how are they related?”
4. **Security (9%):** Questions about security properties, attack surfaces, or vulnerability patterns.

Example: "If someone submits a TruffleHog detector configuration pointing to an internal address or a cloud metadata endpoint, what security boundaries actually exist?"
5. **API & library integration (3%):** Questions about how to use a library's interfaces and understand their runtime behavior.

Example:  "If I create an IP/TCP packet without setting checksums and call bytes(), then tweak a payload byte and call bytes() again - do I get fresh checksums, or does Scapy hand back the same cached bytes from before?"

![](https://labs.scale.com/_next/image?url=https%3A%2F%2Fcdn.sanity.io%2Fimages%2F5uhyv5jy%2Fproduction%2F8138e8360730b788fa8f7dcc58650f5615067ff8-1074x852.jpg%3Fw%3D1200%26fit%3Dcrop%26auto%3Dformat&w=3840&q=75)![](https://labs.scale.com/_next/image?url=https%3A%2F%2Fcdn.sanity.io%2Fimages%2F5uhyv5jy%2Fproduction%2F0ccb660da04b4289cc4b711129800f07896ba954-1396x862.jpg%3Fw%3D1200%26fit%3Dcrop%26auto%3Dformat&w=3840&q=75)![](https://labs.scale.com/_next/image?url=https%3A%2F%2Fcdn.sanity.io%2Fimages%2F5uhyv5jy%2Fproduction%2F2b8f1b48e0a5f39eb75b58d1807c4afe7430a7cd-1430x818.jpg%3Fw%3D1200%26fit%3Dcrop%26auto%3Dformat&w=3840&q=75)![](https://labs.scale.com/_next/image?url=https%3A%2F%2Fcdn.sanity.io%2Fimages%2F5uhyv5jy%2Fproduction%2F7bd53ce2fc1a7210a8a39140c520e2cdf86a0460-1524x856.jpg%3Fw%3D1200%26fit%3Dcrop%26auto%3Dformat&w=3840&q=75)We illustrate an example task below from the  grafana/k6 repository from the Root-cause analysis category:

> I'm debugging some strange HTTP timing metrics in my k6 load test that I think might be a measurement bug or race condition. When I make multiple sequential requests to the same endpoint, the first request shows reasonable values for connecting time and TLS handshaking, but subsequent requests show exactly 0 for both of these metrics even though I can see network activity happening. What's weirder is that sometimes the "blocked" timing shows massive values like 500ms when other times it's near zero for identical requests.

> I also noticed that on one of our Windows test machines,occasionally ALL the timing metrics return 0 for random requests, which definitely seems like a race in the measurement code. The strangest part is when I look at the internal tracer state, sometimes a connection is flagged as "not reused" but the connect timestamps show the same value as the got connection timestamp, which should be impossible if a real TCP handshake occurred. I printed the tracer hook calls and saw that ConnectStart and ConnectDone sometimes get called multiple times for a single request, which looks like a double counting bug.

> At this point I'm not sure if the whole HTTP tracer component is fundamentally broken or if I'm just misunderstanding something. I need to know if I can trust these timing values at all for my performance analysis, or if I should report some of these issues upstream. Adding investigation scripts are fine but remove them after you're done.

**Rubric Construction.** For each question, human experts define a structured rubric of evaluation criteria (average: 12.3 per task). Each criterion is a specific, verifiable factual claim that a correct answer must contain. Rubrics follow standard design principles: specific (with no or little room for interpretation), atomic (testing one distinct aspect), self-containment (gradable without external knowledge).

Here is a a subset of rubrics for the prompt shared above:

> - Explains that zero connecting/TLS time on subsequent requests indicates connection reuse (connection pooling, no handshake needed for reused connections)

> - Explains that variable "blocked" timing reflects connection pool wait time (waiting for available connection, pool contention)

> - States that the Windows issue is not a race condition that needs fixing (OS timer limitation causes identical timestamps)

> - Explains that multiple ConnectStart/ConnectDone calls are due to dual-stack/Happy Eyeballs dialing (IPv4/IPv6 simultaneous connection attempts)

> - Explains that the Reused=false with matching timestamps scenario is a documented Go stdlib HTTP/2 quirk (abandoned connections get pooled and reused)

**Quality Assurance.** Tasks undergo a rigorous multi-stage pipeline to ensure high-quality. Throughout the process, experts are supplemented with LLM based agentic evaluators that monitor data quality in the same task environment that the expert is working in. Each task is then human-reviewed twice further in the pipeline and by Scale’s quality assurance team.

Post-creation, all tasks undergo a human consensus review of their rubrics. Three experts review each rubric, and flag rubric items that inaccurately evaluate the question's requirements or are over prescriptive. We only retain rubrics that are voted to keep by a majority of experts. After that, tasks are manually re-reviewed and those with insufficient evaluation coverage are filtered out.

### Eval Metric: Task Resolve Rate

During evaluation, the agent operates inside a sandboxed Docker container with the target repository mounted. The agent has access to standard shell tools (bash, grep, find, etc.) and can build and run the software. The agent explores the repository, runs experiments, and produces a final answer.

Evaluation is performed by an LLM judge (Claude Opus 4.5) that scores the agent's answer against each rubric criterion independently. Each criterion receives a binary score (met or not met) and is then aggregated.

The primary metric is the **Task Resolve Rate**: the percentage of tasks for which the agent's answer is comprehensive (i.e. passes all rubric items and scores 1.0), as graded by a set of task-specific rubrics.

Agents are instructed to avoid modifying source code files and to clean up any temporary scripts created. A programmatic check automatically fails any task that contains code changes.

### Results

We ran a suite of frontier closed and open coding models on the dataset, using the Mini-SWE-Agent harness running in a sandboxed container on Modal spun up through Harbor, which allows the agent to run bash commands. For the top frontier models, we also ran them on their native scaffolds like Claude Code and Codex CLI.

We observe that even the most frontier models (that report >80% on SWE-Bench) score 35% on the benchmark, highlighting the challenging nature of these tasks and the gap in capability in deeply understanding the codebase. GPT-5.4 xHigh, GPT 5.3 Codex and Claude Opus 4.6 top the leaderboard, while GLM-5 was the leading Open Model on the benchmark.

We observed improved performance on native scaffolds for frontier models, indicating that coding models are most effective when used in combination with the tools they are trained on.

Beyond the raw resolve rate, we see several interesting patterns in the behavior of frontier models.

![](https://labs.scale.com/_next/image?url=https%3A%2F%2Fcdn.sanity.io%2Fimages%2F5uhyv5jy%2Fproduction%2Fed760e49dd8287f0fe45d6645934cd47cf60d806-1600x425.jpg%3Fw%3D1200%26fit%3Dcrop%26auto%3Dformat&w=3840&q=75)- The top 3 models have a much longer solution trajectory, with the top model GPT-5.4 running hundreds of commands.
- GPT-5.4 xhigh searches and explores the codebase aggressively, performing more than twice the number of operations across file operations, searches and code executions as compared to Opus-4.6.

![](https://labs.scale.com/_next/image?url=https%3A%2F%2Fcdn.sanity.io%2Fimages%2F5uhyv5jy%2Fproduction%2F8beb7a98d39de3614b8a4033b53c55763cda9914-1600x1055.jpg%3Fw%3D1200%26fit%3Dcrop%26auto%3Dformat&w=3840&q=75)- Top models consistently produce longer answers (>1200 words). There is a positive correlation between a model’s answer length and its resolution rate.

| Model name               | Thinking settings | Temperature | Max input tokens |
| ------------------------ | ----------------- | ----------- | ---------------- |
| Claude Sonnet 4.6        | High              | 1.0         | 1,000,000        |
| Claude Opus 4.6          | High              | 1.0         | 1,000,000        |
| GPT-5.4 (xHigh)          | xHigh             | 1.0         | 400,000          |
| GPT-5.3 Codex            | xHigh             | 1.0         | 400,000          |
| Gemini 3.1 Pro (Preview) | High              | 1.0         | 1,000,000        |
| Gemini 3 Flash (Preview) | High              | 1.0         | 1,000,000        |
| MiniMax M2.5             | Default (High)    | 1.0         | 200,000          |
| Kimi K2.5                | Default (High)    | 1.0         | 200,000          |
| GLM-5                    | Default (High)    | 0.7         | 128,000          |

## Performance Comparison

1Opus 4.8 (Claude Code) xhigh

NEW57.26±5.08

2GLM 5.2 (Mini-SWE-Agent)

NEW48.12±5.07

2GPT 5.5 (Codex) xHigh

45.43±5.08

2Muse Spark 1.1 (Mini-SWE-Agent) xHigh

NEW42.20±5.08

5GPT 5.4 (Codex) xHigh

40.80±5.10

5Opus 4.7 (Claude Code)

40.32±5.06

7Gpt 5.4 xHigh (Mini-SWE-Agent)

36.30±4.90

7Opus 4.6 (Claude Code)

33.30±5.00

7GPT 5.3 (Codex) xHigh

32.60±4.90

7Sonnet 4.6 (Claude Code)

31.20±5.00

11Opus 4.6 (Mini-SWE-Agent)

30.00±4.90

11DeepSeek V4 Pro (Mini-SWE-Agent)

NEW27.15±4.74

13Muse Spark

24.20±4.60

13Glm 5 (Mini-SWE-Agent)

20.50±4.50

15Gemini 3.1 Pro (Mini-SWE-Agent)

13.50±3.90

15Kimi K2.5 (Mini-SWE-Agent)

13.10±4.10

15Minimax M2.5 (Mini-SWE-Agent)

10.30±3.50

18Gemini 3 Flash (Mini-SWE-Agent)

8.20±3.30

Legend***Rank (UB):** 1 + the number of models whose lower CI bound exceeds this model’s upper CI bound.*

*^ These models were additionally evaluated using the model’s native scaffolds using Harbor.*

** These models use a slightly different tool bundles (registry, defaults, search, edit_replace, submit) instead of the standard SWE-Agent tools used by all other models (registry, edit_anthropic, review_on_submit_m) because we observed significant performance degradations with the latter. In addition, Gemini API was given a 900s timeout instead of 300 because of issues in API stability leading to failed runs.*

[All leaderboards](https://labs.scale.com/leaderboard)

## Media links

- <https://labs.scale.com/apple-touch-icon.png>
- <https://labs.scale.com/logo.png>
