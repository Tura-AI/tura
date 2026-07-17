# AI Coding Agent Benchmarks & Leaderboard | Artificial Analysis

Source: https://artificialanalysis.ai/agents/coding-agents

[Artificial Analysis](https://artificialanalysis.ai/)[](https://artificialanalysis.ai/login)K[Artificial Analysis](https://artificialanalysis.ai/)
[](https://artificialanalysis.ai/login)K[](https://x.com/ArtificialAnlys)[](https://www.linkedin.com/company/artificial-analysis/)# Artificial Analysis Coding Agent Benchmarks

We measure real-world performance of coding agents on software engineering tasks, including cost, token usage, and execution time. We compare how performance changes across agents, models, and execution settings.

To compare language models see our [model benchmarks](https://artificialanalysis.ai/models).

[Benchmarks](https://artificialanalysis.ai/agents/coding-agents)[Comparisons](https://artificialanalysis.ai/agents/coding-agents/comparisons)[Features](https://artificialanalysis.ai/agents/coding)## Artificial Analysis Coding Agent Index

Composite index of 3 benchmarks:

- DeepSWESoftware engineering tasks, 113 tasks[By Datacurve](https://deepswe.datacurve.ai/)
- Terminal-Bench v2Agentic terminal use, 84 tasks[By Laude Institute](https://www.tbench.ai/benchmarks/terminal-bench-2)
- SWE-Atlas-QnATechnical Q&A, 124 tasks[By Scale AI](https://labs.scale.com/leaderboard/sweatlas-qna)

Index represents the average pass@1 across 3 runs of each benchmark. Index recently updated to v1.1. [See methodology for details](https://artificialanalysis.ai/methodology/coding-agents-benchmarking)

[Coding Agents](https://artificialanalysis.ai/agents/coding-agents)[General Work](https://artificialanalysis.ai/agents)[Chatbots](https://artificialanalysis.ai/agents/chatbots)[Presentations](https://artificialanalysis.ai/agents/presentations)[OCR](https://artificialanalysis.ai/agents/ocr)[Data Analysis](https://artificialanalysis.ai/agents/data)[Customer Support](https://artificialanalysis.ai/agents/customer-support)Highlights

### [Coding Agent Index](https://artificialanalysis.ai/agents/#coding-agents-index)

Artificial Analysis Coding Agent Index · Higher is better ### [Time per Task](https://artificialanalysis.ai/agents/#execution-time)

Average agent wall time per task · Lower is better ### [Cost per Task](https://artificialanalysis.ai/agents/#cost-to-run)

Average API cost per task (USD) · Lower is better [Performance](https://artificialanalysis.ai/agents/#coding-agents-index)[Harness Comparison](https://artificialanalysis.ai/agents/#harness-comparison)[Token Usage](https://artificialanalysis.ai/agents/#token-usage)[Cost](https://artificialanalysis.ai/agents/#cost-to-run)[Execution Time](https://artificialanalysis.ai/agents/#execution-time)## Performance

Performance across the Artificial Analysis Coding Agent Index.

IndexScore by BenchmarkDeepSWENewTerminal-Bench v2SWE-Atlas-QnA### Artificial Analysis Coding Agent Index

Composite average pass@1 across DeepSWE, Terminal-Bench v2, and SWE-Atlas-QnA · Higher is betterColor byModelAgent14 of 43 models### What This Metric Means

The Artificial Analysis Coding Agent Index is a composite score built from DeepSWE, Terminal-Bench v2, and SWE-Atlas-QnA.

It is useful for quick comparison, but it should be read alongside the per-eval breakdowns. Two agents with similar index values can still have different strengths across repository tasks, terminal workflows, and rubric-based evaluations.

## Harness Comparison

Artificial Analysis Coding Agent Index by harness for Claude Opus 4.7.

Artificial Analysis Coding Agent IndexDeepSWETerminal-Bench v2SWE-Atlas-QnA### Harness Comparison: Artificial Analysis Coding Agent Index

Composite average pass@1 across Claude Code, Cursor CLI, and Opencode for Claude Opus 4.7 · Higher is better### What This Chart Shows

This chart holds the underlying model constant at Claude Opus 4.7 and compares how it performs across different coding-agent harnesses, including Cursor, Claude Code, and OpenCode.

## Token Usage

Token consumption across the Artificial Analysis Coding Agent Index, including total usage, token mix, efficiency, and per-benchmark breakdowns.

Token UsageToken DistributionCache Hit RateInput vs. OutputTokens by Benchmark### Token Usage per Task

Average input, cache, and output tokens per task14 of 43 modelsOutputCached InputInputPrompt cache hit rates can vary significantly by provider routing, which can materially change effective cost.### Input Tokens

Non-cached input tokens sent to the model, including prompts, instructions, tool context, and task context that were not served from prompt cache.

### Cached Input Tokens

Reused prompt tokens billed through provider prompt caching when that telemetry is available, rather than being processed as a fully fresh input each time.

### Why Cache Usage Varies

Some providers route repeated requests across different backend replicas. When prompt cache state is not shared consistently across those replicas, a model may receive fewer cache hits even when the benchmark task flow is otherwise identical.

We do not add custom relay headers or provider-specific affinity controls to force higher cache reuse, because that would make the benchmark less representative of a typical user setup. As a result, reported costs reflect the cache behavior observed through the configured provider path, not an optimized best-case cache scenario.

### Output Tokens

Tokens returned by the model in its visible response during the task.

### Artificial Analysis Coding Agent Index vs. Total Tokens

Artificial Analysis Coding Agent Index vs. average total tokens per taskColor byModelAgent14 of 43 modelsMost attractive quadrantOpenAIAnthropicSpaceXAIMetaZ.aiCursorDeepSeekGoogle### How to Read This Chart

Each point represents a coding-agent variant. Farther right means higher benchmark performance, while lower token usage appears farther left. Agents toward the upper-left use fewer tokens for a given level of performance.

## Cost

Cost across the Artificial Analysis Coding Agent Index based on current per-token API pricing, including cache write pricing and cache discounts where available. Many users will access coding agent harnesses through subscription plan offerings rather than pay-per-token.

Cost to RunCost DistributionTotal Cost### Cost per Task

Average pay-per-token API cost per task (USD) · Lower is betterColor byModelAgent14 of 43 models### What Cost Is Measuring

This chart shows the average pay-per-token API cost per task across the Artificial Analysis Coding Agent Index, spanning DeepSWE, Terminal-Bench v2, and SWE-Atlas-QnA.

Where applicable, that cost model includes standard input pricing, discounted cached-input pricing, separate cache-write charges, and output pricing rather than treating all prompt tokens as if they were billed at the same uncached input rate.

It is intended to show pay-per-token API cost, not consumer plan pricing or the full operational cost of deploying the system in production. Infrastructure, engineering, and supervision costs are not the focus of this metric.

### Artificial Analysis Coding Agent Index vs. Cost per Task

Artificial Analysis Coding Agent Index vs. average pay-per-token API cost per task (USD)Color byModelAgent14 of 43 modelsMost attractive quadrantOpenAIAnthropicSpaceXAIMetaZ.aiCursorDeepSeekGoogle### How to Read This Chart

Each point represents a coding-agent variant. Farther right means higher benchmark performance, while lower on the chart means lower average cost per task. The most efficient agents sit toward the lower-right: stronger results at lower cost.

## Execution Time

Active agent runtime across the Artificial Analysis Coding Agent Index.

Execution TimeTurns### Time per Task

Average agent wall time per task · Lower is betterColor byModelAgent14 of 43 models### What Execution Time Is Measuring

This chart uses agent wall time: how long the agent process was actively running on each task.

It does not include environment startup, verifier or judge time, or other harness overhead, so it is a cleaner comparison of how long the agent itself was working.

### Artificial Analysis Coding Agent Index vs. Execution Time

Artificial Analysis Coding Agent Index vs. average agent wall time per taskColor byModelAgent14 of 43 modelsMost attractive quadrantOpenAIAnthropicSpaceXAIMetaZ.aiCursorDeepSeekGoogle### How to Read This Chart

Each point represents a coding-agent variant. Farther right means higher benchmark performance, while lower on the chart means shorter average agent runtime per task. Agents toward the lower-right deliver stronger results in less active agent time.

## Run Specifications

## Frequently Asked Questions

### What is the Artificial Analysis Coding Agent Index?

The Artificial Analysis Coding Agent Index is our composite score for coding-agent performance across the public benchmark suite on this page. It combines DeepSWE, Terminal-Bench v2, and SWE-Atlas-QnA to capture implementation, terminal workflow, repository-understanding, and broader software-engineering performance in a single headline metric.

### Which benchmarks are included in the index right now?

The current public index includes DeepSWE, Terminal-Bench v2, and SWE-Atlas-QnA. These benchmarks are combined because they stress different parts of the coding-agent workflow rather than repeating the same task format.

### What kinds of tasks are these benchmarks actually testing?

The public benchmark suite mixes several software engineering task styles. Some tasks are Q&A and repository-understanding tasks that focus on reading a codebase, understanding architecture or behavior, and producing a correct technical answer. Some are implementation and bug-fix tasks that require code changes and are closer to the classic make-a-patch-that-works framing. Some are terminal workflow tasks that test whether the agent can navigate a shell-driven environment, execute tools correctly, and complete a multi-step command-line workflow. The suite also mixes effectively binary outcomes with rubric-scored partial-credit outcomes, which matters because an agent can show useful progress on a difficult task without fully solving it.

### How do Q&A-style tasks differ from implementation-style tasks?

Q&A-style tasks emphasize repository understanding, code reading, tracing behavior, and producing a correct technical explanation. Implementation-style tasks are closer to shipping a working change: the agent has to understand the task, navigate the repository, edit files correctly, and satisfy an evaluator or test-based outcome under execution constraints. Those are related capabilities, but they are not identical. An agent can be strong at repository reasoning and still be weaker at reliable patch execution, or vice versa, which is one reason the composite index should be interpreted alongside the per-benchmark chart.

### How are agents scored on each benchmark?

The benchmark page reports component scores using average pass@1. This is the evaluator-assigned score for a task, and depending on the benchmark it can be either binary or partial credit. A passed run is not automatically the same thing as a solved task: a run can complete cleanly and still receive a zero score. In the current methodology, a task is counted as solved only when it passed and received a positive score. This matters especially for rubric-scored tasks such as SWE-Atlas-QnA, where partial credit can capture useful progress that would be lost in a strict pass-fail metric.

### How is the overall index weighted?

The index is computed from DeepSWE, Terminal-Bench v2, and SWE-Atlas-QnA. For the current Artificial Analysis Coding Agent Index, the public methodology is a simple average across those benchmark scores. Benchmark methodology can evolve as coverage improves, so comparability is best interpreted within the published benchmark suite and its current component set rather than as a timeless absolute score.

### What does execution time mean?

Execution time on this page refers to average wall-clock task runtime per task, not just raw model latency. It is meant to reflect the user-facing time cost of running the whole agent workflow. That includes time spent reasoning, issuing tool calls, reading and writing files, executing shell steps, and waiting on model responses. So an agent can have a fast underlying model and still be slower overall if its workflow is longer or more tool-heavy.

### What does token usage mean, and why does it matter?

Token usage is the average observed token consumption per task across the benchmark suite. On this page we break it out into input, cache, and output tokens. Input tokens are the tokens sent into the model, including prompts, instructions, tool context, and task context. Cache tokens are prompt tokens reused through prompt caching when the provider exposes that telemetry. Output tokens are tokens generated by the model in its response. Token usage matters because it often drives cost and can also indicate how much context an agent consumes to get work done, but token efficiency and cost are not identical because providers price token categories differently and caching can materially change the bill.

### Why can a higher-index agent still be worse for my use case?

A higher index score means stronger performance across the included benchmark mix, but it does not mean the agent is best for every workflow. The index is a balance across benchmark quality, not a direct measure of your specific latency, cost, tooling, or task-type priorities. Real-world choice still depends on whether your workflow looks more like repository Q&A, patching, or terminal execution, and on practical constraints such as IDE integration, model availability, and reliability.

### How realistic are these tasks, and what setup was used for each agent?

These benchmarks measure coding-agent performance across repositories, tools, multi-step workflows, and evaluator-based outcomes. Results on this page reflect specific evaluated agent variants, not just generic product names: model choice, settings, and execution configuration can materially change outcomes, which is why a single agent family may appear in multiple variants in the results. For more background on benchmark runs, task-level scoring, and methodology, see the coding-agents benchmarking methodology page. [View the coding-agents benchmarking methodology](https://artificialanalysis.ai/methodology/coding-agents-benchmarking)

[](https://artificialanalysis.ai/)Artificial Analysis

Get notified about new articles

Email addressSubscribeArtificial Analysis

Explore

- [LLM Leaderboard](https://artificialanalysis.ai/leaderboards/models)
- [Image Arena](https://artificialanalysis.ai/image/arena)
- [Video Arena](https://artificialanalysis.ai/video/arena)
- [AI Agents](https://artificialanalysis.ai/agents)
- [Evaluations](https://artificialanalysis.ai/evaluations)

Company

- [Methodology](https://artificialanalysis.ai/methodology)
- [Services](https://artificialanalysis.ai/services)
- [Contact](https://artificialanalysis.ai/contact)
- [Articles](https://artificialanalysis.ai/articles)
- [FAQ](https://artificialanalysis.ai/faq)

[X](https://x.com/ArtificialAnlys)[LinkedIn](https://www.linkedin.com/company/artificial-analysis/)[YouTube](https://www.youtube.com/@ArtificialAnalysisAI)[Rednote](https://www.xiaohongshu.com/user/profile/69ea6345000000000d034c02)[Discord](https://discord.gg/Mk298GPZ7V)© 2026 Artificial Analysis

[Terms of Use](https://artificialanalysis.ai/docs/legal/Terms-of-Use.pdf)[Privacy Policy](https://artificialanalysis.ai/docs/legal/Privacy-Policy.pdf)
