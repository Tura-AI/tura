# DeepSWE Leaderboard & Scores — July 2026 | BenchLM.ai

Source: https://benchlm.ai/benchmarks/deepSwe

[Skip to main content](https://benchlm.ai/benchmarks/#main-content)[BenchLM](https://benchlm.ai/)[Leaderboard](https://benchlm.ai/)[Compare](https://benchlm.ai/compare)[Benchmarks](https://benchlm.ai/benchmarks)[Models](https://benchlm.ai/models)[Research](https://benchlm.ai/blog)[AI Race](https://benchlm.ai/ai-race)[Tools](https://benchlm.ai/tools)Browse[](https://benchlm.ai/)[External benchmark mirrors](https://benchlm.ai/benchmarks)DeepSWEBenchmark profile

# DeepSWE

A long-horizon software engineering benchmark from Datacurve for measuring frontier coding agents on original tasks drawn from active open-source repositories.

Data verified July 7, 2026## How BenchLM shows DeepSWE

BenchLM mirrors the public DeepSWE leaderboard JSON from Datacurve. The snapshot shows the best available mini-swe-agent configuration per model, while preserving 41 underlying effort-level rows in the source metadata.

DeepSWE evaluates coding agents on 113 original, long-horizon software engineering tasks across 91 repositories and 5 languages, using isolated task environments and program-based verifiers.

DeepSWE is display only on BenchLM. The public rows combine a model, mini-swe-agent harness, and reasoning-effort setting, so BenchLM does not use these scores as weighted model-only ranking inputs.

13 best-per-model rows113 tasks91 repositories5 languages41 effort rows in sourceDisplay only[DeepSWE leaderboard](https://deepswe.datacurve.ai/)[DeepSWE leaderboard JSON](https://deepswe.datacurve.ai/artifacts/v1.1/leaderboard-live.json)[DeepSWE methodology](https://deepswe.datacurve.ai/blog)[GitHub repository](https://github.com/datacurve-ai/deep-swe)## Pass@1 on DeepSWE — July 7, 2026

BenchLM mirrors the published pass@1 view for DeepSWE. [gpt-5-6-sol[max]](https://deepswe.datacurve.ai/) leads the public snapshot at 72.7% , followed by [claude-fable-5[max]](https://benchlm.ai/models/claude-fable) (69.7%) and [gpt-5-6-terra[max]](https://deepswe.datacurve.ai/) (69.6%). BenchLM does not use these results to rank models overall.

[1gpt-5-6-sol[max]

OpenAI

mini_swe_agent_gpt_5_6_sol_max

72.7%Overall —](https://deepswe.datacurve.ai/)[2Closedclaude-fable-5[max]

Anthropic

mini_swe_agent_claude_fable_5_max

69.7%Overall 91Context 1M+](https://benchlm.ai/models/claude-fable)[3gpt-5-6-terra[max]

OpenAI

mini_swe_agent_gpt_5_6_terra_max

69.6%Overall —](https://deepswe.datacurve.ai/)13 modelsExternal benchmark mirrorsCurrentDisplay onlyUpdated July 7, 2026## Pass@1 table (13 models)

Score1[gpt-5-6-sol[max]](https://deepswe.datacurve.ai/)OpenAI72.7%2[claude-fable-5[max]](https://benchlm.ai/models/claude-fable)Anthropic · Closed69.7%3[gpt-5-6-terra[max]](https://deepswe.datacurve.ai/)OpenAI69.6%4[gpt-5-6-luna[max]](https://deepswe.datacurve.ai/)OpenAI67.2%5[gpt-5-5[xhigh]](https://benchlm.ai/models/gpt-5-5)OpenAI · Closed67.0%6[claude-opus-4-8[max]](https://benchlm.ai/models/claude-opus-4-8)Anthropic · Closed59.0%7[claude-sonnet-5[max]](https://benchlm.ai/models/claude-sonnet-5)Anthropic · Closed53.8%8[gpt-5-4[xhigh]](https://benchlm.ai/models/gpt-5-4)OpenAI · Closed51.8%9[glm-5-2[max]](https://benchlm.ai/models/glm-5-2)Z.AI · Open weight43.8%10[gemini-3-5-flash[medium]](https://benchlm.ai/models/gemini-3-5-flash)Google · Closed37.4%11[kimi-k2-7-code](https://benchlm.ai/models/kimi-k2-7-code)Moonshot AI · Open weight30.5%12[claude-sonnet-4-6[high]](https://benchlm.ai/models/claude-sonnet-4-6)Anthropic · Closed29.9%13[gemini-3-1-pro-preview[high]](https://benchlm.ai/models/gemini-3-1-pro)Google · Closed11.8%The published DeepSWE snapshot is tightly clustered at the top: **gpt-5-6-sol[max]** sits at **72.7%**, while the third row is only 3.1 points behind. The broader top-10 spread is 35.3 points, so the benchmark still separates strong models even when the leaders cluster.

13 models have been evaluated on DeepSWE. The benchmark falls in the **External benchmark mirrors** category. BenchLM tracks this category separately from its weighted global scoring system, so these results are best compared on the dedicated Korean benchmark views. DeepSWE is currently displayed for reference but excluded from the scoring formula, so it does not directly affect overall rankings.

## About DeepSWE

Year

2026

Tasks

113 software engineering tasks across 91 repositories and 5 languages

Format

Pass@1 with confidence interval, cost, time, and token metadata

Difficulty

Long-horizon software engineering

DeepSWE includes original tasks with isolated environments and program-based verifiers. BenchLM mirrors the public DeepSWE leaderboard JSON as display-only, using the best available mini-swe-agent configuration per model and preserving cost, time, token, and effort-level source metadata. Each row combines a model, agent harness, and reasoning-effort setting rather than a pure model-only benchmark score.

[DeepSWE benchmark blog](https://deepswe.datacurve.ai/blog)[Public benchmark source](https://deepswe.datacurve.ai/)## BenchLM freshness & provenance

Version

DeepSWE 2026

Refresh cadence

Quarterly

Staleness state

Current

Question availability

Public benchmark set

CurrentDisplay onlyBenchLM uses freshness metadata to decide whether a benchmark should still be treated as a strong differentiator, a benchmark to watch, or a display-only reference. For the full scoring policy, see the [BenchLM methodology page](https://benchlm.ai/methodology#benchlm-defaults-and-caveats).

##  FAQ

### What does DeepSWE measure?

A long-horizon software engineering benchmark from Datacurve for measuring frontier coding agents on original tasks drawn from active open-source repositories.

### Which model leads the published DeepSWE snapshot?

gpt-5-6-sol[max] currently leads the published DeepSWE snapshot with 72.7% pass@1. BenchLM shows this benchmark for display only and does not use it in overall rankings.

### How many models are evaluated on DeepSWE?

13 AI models are included in BenchLM's mirrored DeepSWE snapshot, based on the public leaderboard captured on July 7, 2026.

Last updated: July 7, 2026 · mirrored from the public benchmark leaderboard### The AI models change fast. We track them for you.

A weekly brief for engineers and researchers covering new models, ranking shifts, and pricing changes.

Leave this field blankLoading...I agree to receive weekly BenchLM benchmark updates by email. I can unsubscribe anytime. See the [privacy policy](https://benchlm.ai/privacy).Free. No spam. Unsubscribe anytime.

[BenchLM](https://benchlm.ai/)Independent LLM benchmarks, pricing, and runtime evidence. Published results expose their evidence path and review date.

✓35 verified ranked models

Models tracked281Ranked models79Benchmarks296Data refreshedJuly 11, 2026[How BenchLM ranks models](https://benchlm.ai/methodology)## Explore

[LLM leaderboard](https://benchlm.ai/)[Compare models](https://benchlm.ai/compare)[Models](https://benchlm.ai/models)[Benchmarks](https://benchlm.ai/benchmarks)[Providers](https://benchlm.ai/providers)[Best models by use case](https://benchlm.ai/best)[Model alternatives](https://benchlm.ai/alternatives)## Rankings

[Coding models](https://benchlm.ai/coding)[Agentic models](https://benchlm.ai/agentic)[Reasoning models](https://benchlm.ai/reasoning)[Multimodal models](https://benchlm.ai/multimodal-grounded)[Best overall](https://benchlm.ai/best/overall)[Best open source](https://benchlm.ai/best/open-source)[Best Chinese models](https://benchlm.ai/best/chinese-models)## Dashboards

[AI Race](https://benchlm.ai/ai-race)[LLM pricing](https://benchlm.ai/llm-pricing)[Price vs performance](https://benchlm.ai/llm-price-performance)[LLM speed](https://benchlm.ai/llm-speed)[Agent benchmarks](https://benchlm.ai/llm-agent-benchmarks)[Pricing trends](https://benchlm.ai/llm-pricing-trends)[LLM statistics](https://benchlm.ai/stats)## Research & tools

[Research](https://benchlm.ai/blog)[All tools](https://benchlm.ai/tools)[LLM selector](https://benchlm.ai/tools/llm-selector)[Token counter](https://benchlm.ai/tools/token-counter)[Download data](https://benchlm.ai/data)[Methodology](https://benchlm.ai/methodology)[Evidence confidence](https://benchlm.ai/benchmark-confidence)[Leaderboard history](https://benchlm.ai/llm-leaderboard-history)[AI app stack](https://benchlm.ai/stack)### Weekly benchmark digest

Rankings, evidence, model launches, and pricing changes.

Leave this field blankLoading...I agree to receive weekly updates. Unsubscribe anytime. [Privacy](https://benchlm.ai/privacy).BenchLM never interpolates missing scores. © 2026 BenchLM.

[About](https://benchlm.ai/about)[Advertise](https://benchlm.ai/advertise)[Contact](https://benchlm.ai/contact)[Editorial policy](https://benchlm.ai/editorial-policy)[Affiliate disclosure](https://benchlm.ai/affiliate-disclosure)[Privacy](https://benchlm.ai/privacy)Analytics choices[Terms](https://benchlm.ai/terms)[llms.txt](https://benchlm.ai/llms.txt)[RSS](https://benchlm.ai/rss.xml)

## Media links

- <https://benchlm.ai/favicon-180.png>
