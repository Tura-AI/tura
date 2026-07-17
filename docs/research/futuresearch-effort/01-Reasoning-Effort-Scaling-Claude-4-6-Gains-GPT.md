# Reasoning Effort Scaling: Claude 4.6 Gains, GPT-5 and Gemini Don't

Source: https://futuresearch.ai/effort-scaling/

[![FutureSearch Logo](https://futuresearch.ai/images/future-search-logo-128.webp)](https://futuresearch.ai/)[futuresearch](https://futuresearch.ai/)☰- [Pricing](https://futuresearch.ai/pricing/)
- [Research](https://futuresearch.ai/writings/)
- [Docs](https://futuresearch.ai/docs/)
- [Evals](https://evals.futuresearch.ai/)
- [Markets](https://markets.futuresearch.ai/)
- [Blog](https://futuresearch.ai/blog/)
- [Company](https://futuresearch.ai/company/)
- [Careers](https://futuresearch.ai/careers/)
- [Try it for free](https://futuresearch.ai/app/)

[← Back to Research](https://futuresearch.ai/writings/)# Reasoning Effort Scaling: Claude 4.6 Gains, GPT-5 and Gemini Don't

February 18, 2026 • Updated March 30, 2026[![Peter Mühlbacher](https://futuresearch.ai/authors/peter-muhlbacher-byline.webp)By Peter Mühlbacher](https://futuresearch.ai/#peter-mühlbacher)New Sonnet 4.6 results show both Claude 4.6 models, unlike GPT-5 and Gemini 3 Flash, benefit from higher reasoning effort on web research tasks.

Last week we published [a finding that surprised us](https://futuresearch.ai/effort-paradox): for most frontier models, cranking up reasoning effort on web research tasks produces equal or worse results at higher cost. At the time, Claude 4.6 Opus was the sole exception. We now have results for Sonnet 4.6, and they confirm this wasn't a fluke—it's a pattern unique to Anthropic's latest model generation.

## The updated picture

We ran Sonnet 4.6 at low and high effort through the same 150+ real-world web research tasks on [Deep Research Bench](https://evals.futuresearch.ai). Here's the full effort-scaling picture across the three major labs:

| Model             | Effort | Score | Cost/task | Time  |
| ----------------- | ------ | ----- | --------- | ----- |
| Claude 4.6 Opus   | low    | 53.1% | $0.24     | 73 s  |
|                   | high   | 55.0% | $0.55     | 183 s |
| Claude 4.6 Sonnet | low    | 50.4% | $0.27     | 130 s |
|                   | high   | 54.9% | $0.46     | 262 s |
| Claude 4.5 Opus   | low    | 54.9% | $0.31     | 159 s |
|                   | high   | 54.8% | $0.46     | 140 s |
| GPT-5             | low    | 49.6% | $0.25     | 230 s |
|                   | medium | 48.6% | $0.35     | 183 s |
|                   | high   | 48.1% | $0.39     | 217 s |
| Gemini 3 Flash    | low    | 49.9% | $0.05     | 96 s  |
|                   | high   | 47.9% | $0.14     | 182 s |

Full leaderboard at [evals.futuresearch.ai](https://evals.futuresearch.ai).

***Note on runtime numbers:** Since the [effort paradox post](https://futuresearch.ai/effort-paradox), we changed how we measure task runtime. Previously we reported naive wall-clock time; we now estimate runtime as average duration per step × (number of steps + 1). This better reflects actual model execution time by removing variability from queuing and infrastructure delays, but means the numbers in this post are not directly comparable to those in the earlier post.*

## Three patterns, one standout

The data splits cleanly into three categories:

**Effort helps — Claude 4.6 only.** Both Opus 4.6 (+1.9 points) and Sonnet 4.6 (+4.5 points) score meaningfully higher at high effort. Sonnet 4.6 shows the largest effort-driven improvement of any model we've tested: going from 50.4% to 54.9% nearly closes the gap to Opus, which reaches 55.0%.

**Effort is irrelevant — Claude 4.5 Opus.** The previous-generation Opus shows virtually identical scores at low and high effort (54.9% vs 54.8%), while high effort costs 48% more. You pay more for nothing.

**Effort actively hurts — GPT-5 and Gemini 3 Flash.** GPT-5 scores decrease monotonically from low (49.6%) through medium (48.6%) to high (48.1%). Gemini 3 Flash drops 2 points from low to high. In both cases, you pay substantially more for worse results.

## What changed with the 4.6 generation?

The pattern is notable: neither Claude 4.5 Opus nor any non-Anthropic model benefits from higher effort on web research tasks, but both 4.6 models—Opus and Sonnet—do. Something in the 4.6 generation seems to have improved how these models allocate additional reasoning budget on agentic web research.

As [we discussed before](https://futuresearch.ai/effort-paradox), web research is fundamentally different from the math and coding tasks where chain-of-thought reasoning traditionally shines. The bottleneck in research is information retrieval and source evaluation, not step-by-step deduction. Most models waste their extra reasoning budget second-guessing good findings or chasing marginal sources. The 4.6 models seem to actually use those extra tokens productively—finding better sources, cross-referencing more carefully, and synthesising more accurately.

## Effort scaling isn't the whole story

While Sonnet 4.6's effort scaling is impressive, it doesn't make it a good deal overall. A look at the [Pareto frontier plots on our leaderboard](https://evals.futuresearch.ai) tells the full story: Sonnet 4.6 isn't on the efficiency frontier for either cost or speed. At high effort it matches Opus 4.5 (low) on accuracy (54.9% vs 54.9%) but costs 48% more and takes 65% longer. At low effort it trails Opus 4.6 (low) by 2.7 points while being slower and slightly more expensive.

In short, the Opus line dominates Sonnet 4.6 at every operating point on DRB. Effort scaling is a useful property—it means you can tune the accuracy-cost trade-off—but it doesn't automatically put a model on the Pareto frontier.

## The practical takeaway

If you're building research workflows with LLMs, effort configuration should depend on which model you're using:

- **Claude 4.6 Opus or Sonnet**: High effort is worth considering. But for cost-efficiency, Opus 4.6 at low effort ($0.24/task, 73 s) or Opus 4.5 at low effort ($0.31/task, 159 s) remain the best value.
- **Claude 4.5 Opus**: Stick with low effort. You'll get the same accuracy at 33% lower cost.
- **GPT-5 or Gemini 3 Flash**: Use the lowest effort setting. Higher effort will cost more and deliver worse results.

As always, we keep the live leaderboard at [evals.futuresearch.ai](https://evals.futuresearch.ai) current as new models and configurations ship.

### Related

- [Higher Effort Settings in LLMs Can Reduce Accuracy](https://futuresearch.ai/effort-paradox)
- [How Much Does Deep Research Cost? A Model-by-Model Breakdown](https://futuresearch.ai/cost-of-deep-research)
- [Deep Research Benchmark: Evaluating LLM Web Research Agents](https://futuresearch.ai/deep-research-bench)

[View All Research](https://futuresearch.ai/writings/)![FutureSearch Logo](https://futuresearch.ai/futuresearch-logo.webp)General inquiry? You can reach us at hello@futuresearch.ai.

#### Company

[Team](https://futuresearch.ai/company/)[Careers](https://futuresearch.ai/careers/)[Press](https://futuresearch.ai/press/)[Privacy Policy](https://futuresearch.ai/privacy/)[Terms of Service](https://futuresearch.ai/terms/)#### Developers

[SDK Docs](https://futuresearch.ai/docs/)[API Reference](https://futuresearch.ai/docs/api/)[Case Studies](https://futuresearch.ai/docs/case-studies/)[GitHub](https://github.com/futuresearch/futuresearch-python)[Support](https://futuresearch.ai/support/)#### Integrations

[Claude Code](https://futuresearch.ai/docs/#tab-claude-code-plugin)[Cursor](https://futuresearch.ai/docs/#tab-cursor-mcp)[ChatGPT Codex](https://futuresearch.ai/docs/#tab-codex-mcp)[Claude.ai](https://futuresearch.ai/docs/claude-ai/)#### Track Record

[Trading Results](https://markets.futuresearch.ai/)[Accuracy Evals](https://evals.futuresearch.ai/)[Tournament Standings](https://evals.futuresearch.ai/#metaculus)#### Follow Us

[X (Twitter)](https://x.com/FUTURESEARCHAI)[@dschwarz26](https://x.com/dschwarz26)[LinkedIn](https://www.linkedin.com/company/futuresearch)

## Media links

- <https://futuresearch.ai/futuresearch-logo.webp>
- <https://futuresearch.ai/icon.png?dfd4c66afe1db727>
- <https://futuresearch.ai/images/future-search-logo-128.webp>
- <https://futuresearch.ai/authors/peter-muhlbacher-byline.webp>
