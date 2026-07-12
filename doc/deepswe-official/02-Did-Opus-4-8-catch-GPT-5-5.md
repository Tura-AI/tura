# Did Opus 4.8 catch GPT-5.5 on DeepSWE?

Source: https://deepswe.net/deepswe-opus-4-8/

[
          ![DeepSWE Logo](https://deepswe.net/deepswe-logo.webp)
          DeepSWE Information Hub
        ](https://deepswe.net/) [Conclusion](https://deepswe.net/deepswe-opus-4-8/#conclusion) [Four benchmarks](https://deepswe.net/deepswe-opus-4-8/#official-signals) [Effort routing](https://deepswe.net/deepswe-opus-4-8/#effort-routing) [Reddit reactions](https://deepswe.net/deepswe-opus-4-8/#reddit-views) [Back to DeepSWE home](https://deepswe.net/) # Did Opus 4.8 catch GPT-5.5 on DeepSWE?

Short answer: no. Opus 4.8 did improve, and its max setting is stronger and cheaper than Opus 4.7 max. But on DeepSWE it still trails GPT-5.5. The real story is not a single score, but how effort, cost, latency, and task type change the routing decision.

Sponsored Quick take ## Quick answer

The DeepSWE leaderboard now includes Claude Opus 4.8. GPT-5.5 [xhigh] still leads at 70% +/- 4%, while Claude Opus 4.8 [max] sits at 58% +/- 5%. That puts it ahead of Claude Opus 4.7 [max] at 54% +/- 5% and in the same band as GPT-5.4 [xhigh] at 56% +/- 5%.

Leader **GPT-5.5 [xhigh] 70%** Opus 4.8 ceiling **max 58%** Practical default **Do not default to max** Contents 1. [Conclusion: real progress, no overtake](https://deepswe.net/deepswe-opus-4-8/#conclusion)
2. [What is Claude Opus 4.8?](https://deepswe.net/deepswe-opus-4-8/#what-is-opus-48)
3. [What changed from Claude Opus 4.7?](https://deepswe.net/deepswe-opus-4-8/#changed-from-47)
4. [Official Opus 4.8 benchmark signals and DeepSWE](https://deepswe.net/deepswe-opus-4-8/#official-signals)
5. [Why DeepSWE is an important benchmark](https://deepswe.net/deepswe-opus-4-8/#why-deepswe-matters)
6. [What DeepSWE also tells us](https://deepswe.net/deepswe-opus-4-8/#effort-routing)
7. [What Reddit users are saying](https://deepswe.net/deepswe-opus-4-8/#reddit-views)
8. [Practical routing advice](https://deepswe.net/deepswe-opus-4-8/#routing-advice)
9. [FAQ](https://deepswe.net/deepswe-opus-4-8/#faq)

Quick answer The DeepSWE leaderboard now includes Claude Opus 4.8. GPT-5.5 [xhigh] still leads at 70% +/- 4%, while Claude Opus 4.8 [max] sits at 58% +/- 5%. That puts it ahead of Claude Opus 4.7 [max] at 54% +/- 5% and in the same band as GPT-5.4 [xhigh] at 56% +/- 5%.

So the takeaway is not that Opus 4.8 failed to improve. It is that the improvement is not enough to overtake GPT-5.5 on DeepSWE. More precisely, Opus 4.8 raises the ceiling and improves efficiency along the Claude route, while GPT-5.5 still has the stronger default efficiency baseline.

Anthropic's official benchmarks paint a more optimistic picture for Opus 4.8: it beats Opus 4.7 on SWE-bench Verified, SWE-bench Pro, and Terminal-Bench 2.1. DeepSWE, however, tests original tasks, long-horizon repo work, and a shared mini-swe-agent harness. That is why Anthropic's launch charts and the DeepSWE leaderboard can point to different engineering conclusions.

> Treat Opus 4.8 as the stronger Claude route for high-value work, not as a reason to turn max on for every issue. For everyday low-to-medium complexity coding, GPT-5.5 still looks more like the default route. For complex reasoning, design discussion, and tasks where failure is expensive, consider Opus 4.8 high or max.

## Conclusion: Opus 4.8 improved, but it did not catch GPT-5.5

The May 30, 2026 DeepSWE results give a direct answer: Claude Opus 4.8 closed part of the gap, but it did not catch GPT-5.5. The leader, GPT-5.5 [xhigh], scores 70% +/- 4%. Opus 4.8 [max] scores 58% +/- 5%, GPT-5.4 [xhigh] scores 56% +/- 5%, and Opus 4.7 [max] scores 54% +/- 5%.

This chart is the best place to start. It puts Opus 4.8 on the same DeepSWE leaderboard, so you can compare score, average cost, average runtime, and output tokens at once. Read in isolation, 58% looks like a clear loss. Read next to GPT-5.4 and Opus 4.7, it looks less like Opus falling behind and more like Opus failing to cross GPT-5.5's efficiency line.

![DeepSWE 2026-05-30 leaderboard showing GPT-5.5 xhigh at 70%, Claude Opus 4.8 max at 58%, GPT-5.4 xhigh at 56%, and Claude Opus 4.7 max at 54%.](https://deepswe.net/leaderboard-2026-05-30.png) DeepSWE leaderboard for 2026-05-30: Opus 4.8 [max] moves into the leading group, but GPT-5.5 [xhigh] remains ahead. That is the core judgment of this article: Opus 4.8 is neither a failed release nor a GPT-5.5 killer. It is a meaningful correction to the Opus 4.7 path: a higher ceiling and lower max-setting cost, but that ceiling still requires higher effort, longer runs, and more output tokens.

> The good news for Opus 4.8 is that it really improved. The bad news is that it still has not knocked GPT-5.5 off the top.

## What is Claude Opus 4.8?

Claude Opus 4.8 is Anthropic's next Opus model, released on May 28, 2026 as the direct successor to Claude Opus 4.7. Anthropic positions it as a flagship model for complex reasoning, agentic coding, long-running task execution, and highly autonomous workflows.

For developers, the important point is not the vague idea that Opus 4.8 is better at chat. It is that Anthropic is explicitly placing the model inside long-horizon engineering work: Claude Code dynamic workflows, effort control, fast mode, the API model ID claude-opus-4-8, and honesty evaluations that reward the model for flagging problems in its own code instead of silently letting them pass.

-
                  Release date: May 28, 2026. Anthropic says Opus 4.8 was available on launch day.

-
                  API model ID: claude-opus-4-8. Standard pricing matches Opus 4.7 at $5 per million input tokens and $25 per million output tokens.

-
                  Fast mode can run up to 2.5x faster. Anthropic says it is three times cheaper than the previous Opus fast mode, although fast mode itself uses higher token prices.

## What changed from Claude Opus 4.7?

Anthropic's launch materials make Opus 4.8 look less like a generational leap and more like an engineering-focused correction: better long-horizon execution, clearer effort levels, stronger self-questioning, and Claude Code dynamic workflows that are better suited to breaking down large tasks.

### 1. Effort control becomes the main knob

Opus 4.8 uses high effort by default. Anthropic also offers higher settings such as extra / xhigh and max, but its own guidance is not to push every task to the top. Extra effort is meant for difficult tasks and long asynchronous workflows.

### 2. Claude Code dynamic workflows

Anthropic says Claude Code can have Claude plan tasks, run many parallel subagents, verify outputs, and report back. That capability sits in the same problem space as DeepSWE's long-horizon engineering tasks, but it is not the same benchmark harness.

### 3. Fast mode comes to Opus 4.8

Fast mode trades a higher token price for faster output. It is useful for latency-sensitive workflows, but it should not be treated as a proxy for the highest pass rate on DeepSWE.

### 4. Honesty and self-correction

Anthropic emphasizes that Opus 4.8 is more likely to call out uncertainty and problems in its own code. In the official evaluations, it was less likely than its predecessor to let flaws in self-generated code pass without warning.

> Opus 4.8 feels more like a cleanup of the 4.7 engineering curve than a one-shot reshuffling of every coding benchmark.

## Official Opus 4.8 benchmark signals and DeepSWE

Why do Anthropic's official charts look strong while DeepSWE still has Opus 4.8 behind GPT-5.5? The key is that the four benchmarks do not measure the same thing. SWE-bench Verified is more mature and closer to saturation. SWE-bench Pro is harder, but still closely tied to known software-engineering task distributions. Terminal-Bench focuses on terminal execution. DeepSWE emphasizes original tasks, long-horizon repo changes, a shared mini-swe-agent harness, cost, and runtime.

Anthropic's launch materials report that Opus 4.8 improves over Opus 4.7 on SWE-bench Verified, SWE-bench Pro, and Terminal-Bench 2.1. The official DeepSWE leaderboard also shows Opus 4.8 [max] above Opus 4.7 [max], but still below GPT-5.5 [xhigh].

| Benchmark          | Opus 4.7         | Opus 4.8         | How to read the difference                                                                                                                                                     |
| ------------------ | ---------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| SWE-bench Verified | 87.6%            | 88.6%            | The gain is real, but frontier models are already tightly clustered on this benchmark. One percentage point does not automatically imply an advantage on long real-repo tasks. |
| SWE-bench Pro      | 64.3%            | 69.2%            | This is the clearest coding gain in Anthropic's official signals, and it also beats the published GPT-5.5 comparison number. But it is not DeepSWE.                            |
| Terminal-Bench 2.1 | 66.1%            | 74.6%            | The terminal-execution gain is large. Anthropic's note also says GPT-5.5 reaches 78.2% in the Terminus-2 public harness and 83.4% in the Codex CLI harness.                    |
| DeepSWE            | 54% +/- 5% [max] | 58% +/- 5% [max] | On DeepSWE, Opus 4.8 beats 4.7, but GPT-5.5 [xhigh] is 70% +/- 4%, so Opus 4.8 does not overtake it.                                                                           |

> The same model can look strong on SWE-bench Pro while DeepSWE exposes differences in cost, runtime, and long-horizon execution. Similar benchmark names do not mean they measure the same engineering capability.

## Why DeepSWE is an important benchmark

DeepSWE is valuable because it shifts the question from whether a model can fix a known issue to whether it can complete a new requirement in an unfamiliar repo. The official page describes DeepSWE as a benchmark of original, long-horizon engineering tasks. In this version, it covers 113 tasks, 91 repos, and 5 languages.

DeepSWE also runs every model through mini-swe-agent with fixed prompts, tools, and execution environment. That choice is not perfect, because developers using Codex CLI, Claude Code, Cursor, or Gemini CLI get each model's native tooling and product prompts. But a fixed harness reduces the degree to which the comparison is really measuring whose product scaffolding is stronger.

### 1. Lower contamination risk

DeepSWE tasks are newly written rather than directly adapted from existing commits, pull requests, or public patches. That makes it harder for a model to rely on training-memory leakage or public history to guess the answer.

### 2. More realistic engineering tasks

DeepSWE prompts are not necessarily long, but the solutions often cross files, modules, and behavior paths. That makes it better at surfacing failures in long-horizon exploration and multi-constraint implementation.

### 3. Results go beyond pass rate

DeepSWE also shows cost, runtime, and output tokens. That matters for engineering teams because a stronger model that costs three times as much and takes twice as long calls for a very different routing strategy.

### 4. A fixed harness makes the debate clearer

mini-swe-agent is controversial, but it gives the debate a clean boundary. The question is how models perform with the same bash tools and shared prompt, not how Claude Code or Codex CLI performs as a full product.

## What else DeepSWE tells us about Opus 4.8

The most important point: do not default Opus 4.8 to max. DeepSWE's all-effort comparison shows Opus 4.8 moving from 47% to 51% to 58% as it goes from medium to high to max. Max is stronger, but that strength is not free.

![DeepSWE effort routing comparison for Claude Opus 4.8 medium, high, max, Claude Opus 4.7 max, and GPT-5.5 medium with score, cost, output tokens, and time.](https://deepswe.net/opus-48-effort-routing.png) Opus 4.8 effort routing: medium to high is a modest step up, while max is the expensive final gear. The cost curve is the point of the chart. Opus 4.8 [high] averages about $3.98. Opus 4.8 [max] jumps to $12.58. Average output grows from 48k tokens to 136k, and average runtime rises from about 21 minutes to roughly 44 minutes. In other words, max is the expensive final gear: useful for high-value tasks with high failure cost and real long-horizon exploration, but a poor default for every everyday issue.

Opus 4.8's improvement is best understood as getting above Opus 4.7 max with better strength and lower cost. Opus 4.8 [max] scores 58%, versus 54% for Opus 4.7 [max]. At the same time, Opus 4.8 [max] averages $12.58, below Opus 4.7 [max] at $18.19. So 4.8 did improve, but the improvement is mainly about efficiency and ceiling within the same family of routes.

GPT-5.5's advantage is its efficiency baseline. The chart shows GPT-5.5 [medium], not the leaderboard-topping GPT-5.5 [xhigh]. Even so, GPT-5.5 [medium] already reaches 48%, costs $2.34, takes 10m53s, and outputs 18.6k tokens. That is close to Opus 4.8 [medium] at 47%, but cheaper, faster, and lighter on tokens. In practice, this means GPT-5.5 looks more like the default route for simple to moderately complex coding tasks, while Opus 4.8 is better reserved for deep reasoning, design discussion, and complex context judgment.

> Treat max as the final gear, not the default. That is the value of DeepSWE: it tells you not just who wins, but what winning costs.

## What Reddit users are saying: why the feel and the DeepSWE result can diverge

The Reddit discussion roughly splits into a few camps. One group sees DeepSWE as one of the few benchmarks that matches their hands-on experience: GPT-5.5 feels steadier for medium-complexity and default agentic-coding routes, while Opus 4.8 is better than Opus 4.7 but does not overtake 5.5 overall. One r/developersIndia post says that, after heavy GPT-5.5 use, the DeepSWE result helps explain why it feels smoother for delegated tasks and workflows such as /goal.

A second camp emphasizes that Opus 4.8 does not feel weak in practice. In r/ClaudeCode, some users describe 4.8 as feeling more like a stronger version of 4.6 than 4.7, especially in multi-stage agent tasks with processes and gates. That does not contradict DeepSWE. It suggests Opus 4.8 has real improvements, but not that it automatically beats GPT-5.5 in a shared-harness, cost-aware, long-horizon benchmark.

A third camp questions whether mini-swe-agent favors some models. In the r/singularity discussion, commenters point out that DeepSWE gives the model only bash tools, which may understate an Opus model that has been reinforced and product-tuned inside Claude Code. Others ask why the benchmark does not use each model's native harness. DeepSWE's official blog response is that a small pilot found no clear disadvantage for any one model family under mini-swe-agent on the same task set.

A fourth camp is more practical: task type should decide the route. Some users say Opus 4.8 is strong on low-level C, assembly, memory management, high-concurrency work, lock-free code, and complex design discussion. Others say Codex/GPT-5.5 is stronger on the everyday work of business apps, React, SQL, backend implementation, and acceptance tests. Taken together, those experiences make DeepSWE look more like a routing signal than a one-model loyalty test.

> User feel is not a substitute for a benchmark. A benchmark is not a substitute for user feel either. Together, they look more like the evidence engineering decisions actually need.

## Practical routing advice: when to use GPT-5.5 and when to use Opus 4.8

If you only want a default strategy, frame it this way: GPT-5.5 is the everyday coding default, while Opus 4.8 is the upgrade route for complex reasoning and high-value tasks. That is not because GPT-5.5 is smarter in every scenario. It is because DeepSWE shows a better balance between medium-to-high success rate, low cost, fewer tokens, and shorter runs.

For solo developers: use GPT-5.5 for routine bug fixes, CRUD, React / SQL / backend implementation, test coverage, and small refactors. Switch to Opus 4.8 high when a task requires long exploration, architecture tradeoffs, difficult uncertainty judgment, or an upfront design discussion before implementation.

For teams: do not test only pass rate once. Run a four-part bake-off on your own repo: one real bug, one multi-file feature, one refactor, and one test / verification task. Track pass rate, review burden, missed requirements, runtime, tokens, cost, and how much the human reviewer still has to fix.

-
                  Default: GPT-5.5 medium / high for speed, cost control, and stable implementation.

-
                  Upgrade: Opus 4.8 high for complex context, design judgment, and long-horizon exploration.

-
                  Final gear: Opus 4.8 max only for high-value tasks where failure is costly and deeper search is worth paying for.

> DeepSWE is not telling you which model to choose forever. It is telling you how to route by task value and failure cost.

## FAQ

### Did Opus 4.8 catch GPT-5.5 on DeepSWE?

No. On DeepSWE, GPT-5.5 [xhigh] scores 70% +/- 4%, while Claude Opus 4.8 [max] scores 58% +/- 5%. Opus 4.8 is ahead of Opus 4.7 [max], but it has not overtaken GPT-5.5.

### Does that mean Opus 4.8 did not improve?

No. DeepSWE shows Opus 4.8 [max] scoring above Opus 4.7 [max] while also costing less on average. The more accurate statement is that Opus 4.8 improved, but mainly by raising the ceiling and efficiency of the same route, not by knocking GPT-5.5 off the top.

### Why do Anthropic's official benchmarks look stronger?

Because the benchmarks measure different things. SWE-bench Verified, SWE-bench Pro, and Terminal-Bench 2.1 all show Opus 4.8 improving over Opus 4.7. DeepSWE tests original tasks, long-horizon repo work, a shared mini-swe-agent harness, cost, and runtime. Both are valid, but they answer different questions.

### Should I default Opus 4.8 to max?

Usually, no. Max scores higher, but cost, output tokens, and runtime all increase sharply. A more sensible strategy is to use GPT-5.5 or Opus 4.8 medium/high for everyday tasks, and reserve Opus 4.8 max for high-value work where failure is expensive and long-horizon exploration is worth the cost.

### Could mini-swe-agent make DeepSWE biased toward GPT-5.5?

It is a reasonable question. DeepSWE fixes the harness to mini-swe-agent to reduce product-scaffolding differences, but that also means it does not directly represent the full Claude Code, Codex CLI, Cursor, or Gemini CLI product experience. The DeepSWE official blog reports a small harness comparison suggesting mini-swe-agent does not clearly favor one model family, but it remains an important caveat when interpreting the result.

### What kinds of tasks is Opus 4.8 best suited for?

Taken together, DeepSWE and user discussion suggest that Opus 4.8 is better suited to complex reasoning, design discussion, long-context judgment, low-level systems work, and tasks with high failure cost. GPT-5.5 is the better everyday coding default when you need fast, cheap, low-token, stable delivery.

## Media links

- <https://deepswe.net/favicon-48.png>
- <https://deepswe.net/apple-touch-icon.png>
- <https://deepswe.net/deepswe-opus-4-8-social-card.png>
- <https://deepswe.net/deepswe-logo.webp>
- <https://deepswe.net/leaderboard-2026-05-30-720.avif>
- <https://deepswe.net/leaderboard-2026-05-30-1120.avif>
- <https://deepswe.net/leaderboard-2026-05-30.avif>
- <https://deepswe.net/leaderboard-2026-05-30-720.webp>
- <https://deepswe.net/leaderboard-2026-05-30-1120.webp>
- <https://deepswe.net/leaderboard-2026-05-30.webp>
- <https://deepswe.net/leaderboard-2026-05-30.png>
- <https://deepswe.net/opus-48-effort-routing-720.avif>
- <https://deepswe.net/opus-48-effort-routing-1120.avif>
- <https://deepswe.net/opus-48-effort-routing.avif>
- <https://deepswe.net/opus-48-effort-routing-720.webp>
- <https://deepswe.net/opus-48-effort-routing-1120.webp>
- <https://deepswe.net/opus-48-effort-routing.webp>
- <https://deepswe.net/opus-48-effort-routing.png>
