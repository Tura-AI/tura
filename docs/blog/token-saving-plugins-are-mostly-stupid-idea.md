# Token-Saving Plugins Are Mostly Stupid Idea

I am tired of people believing every "save 90% of tokens" claim attached to a coding-agent plugin.

In simple sentence: They are useless if not worse.

The trick is embarrassingly simple: None of them benchmarked against real long-horizon tasks.

Confusing those statements is not optimism...

## We ran the plugins on a repository rewrite

[FULL BENCHMARK](https://turaai.net/benchmark)

This was not a toy prompt asking for one function. The task was to **rewrite the Rust eza repository as a behavior-compatible Python implementation**. The agent had to inspect the reference project, reproduce the CLI in another language, and face **52 harness assertions**.

Every published run used **GPT-5.6-sol, High reasoning, and Codex CLI 0.144.1**. The comparison contains exactly two runs per arm:

- Ponytail r2/r3, both with **full hook + skill** activation;
- RTK r2/r3, both with isolated RTK activation; and
- two previously published no-plugin runs with the same task, model, reasoning level, and CLI version.

| Arm                         |   n | Harness score | Total tokens | Modeled cost |      Rounds |    Duration |
| --------------------------- | --: | ------------: | -----------: | -----------: | ----------: | ----------: |
| No plugin                   |   2 |        78.85% |       6.660M |    $5.281946 |        62.5 |        895s |
| Ponytail, full hook + skill |   2 |        80.77% |   **-7.56%** |   **-8.87%** |      -9.60% |     +13.51% |
| RTK                         |   2 |        76.92% |  **+13.20%** |   **+7.18%** | **+44.00%** | **+40.69%** |

The sanitized [per-run data](https://github.com/Tura-AI/benchmark/blob/main/blog_data/token-saving-plugin-eza/runs.json), [computed summary](https://github.com/Tura-AI/benchmark/blob/main/blog_data/token-saving-plugin-eza/summary.json), [methodology](https://github.com/Tura-AI/benchmark/blob/main/blog_data/token-saving-plugin-eza/methodology.json), and [293-round activation audit](https://github.com/Tura-AI/benchmark/blob/main/blog_data/token-saving-plugin-eza/round-activation-audit.jsonl) are public. All six Codex processes exited 0 and produced complete usage and evaluator data. A run can still miss harness assertions; that is the score, not a crashed experiment.

Ponytail looks 8.87% cheaper. RTK looks 7.18% more expensive. If this were a plugin landing page, this is where somebody would choose the flattering row, enlarge the percentage, and quietly send the error bars on vacation.

## The "saving" is smaller than ordinary run variance

The same agent, model, task, and configuration did not produce remotely stable bills across two repetitions:

| Arm       |  Cost in the two runs | Cost range / mean | Token range / mean | Round range / mean |
| --------- | --------------------: | ----------------: | -----------------: | -----------------: |
| No plugin | $4.139647 - $6.424245 |        **43.25%** |             53.02% |             40.00% |
| Ponytail  | $3.569452 - $6.057281 |        **51.69%** |             57.36% |             47.79% |
| RTK       | $4.789893 - $6.532388 |        **30.78%** |             39.75% |             26.67% |

Here, "range / mean" is the gap between the two runs divided by their mean. It is not a confidence interval; with n=2, pretending to have one would be statistical cosplay.

But the scale still matters. Ponytail's apparent **8.87%** cost saving sits inside a **51.69%** within-arm cost swing. RTK's apparent **7.18%** cost increase sits inside a **30.78%** swing. Even the no-plugin pair moves **43.25%** without any plugin to praise or blame.

These data therefore do **not** identify a plugin effect. They show that natural trajectory variance is a more economical explanation for these small mean differences until a much larger repeated experiment separates signal from noise. Declaring victory from two runs while ignoring a within-group swing four to six times larger is not benchmarking. It is numerology with a README.

What the experiment does establish is simpler: a local compression claim does not reliably predict the complete-task bill. Ponytail's mean moved modestly down; RTK's moved up. Neither result resembles the giant percentage printed on the local optimization.

## Here is the actual coding-agent bill

The broader repository dataset contains **140 Codex CLI Medium and High runs**: **10,365 agent rounds, 901,608,531 tokens, and $680.34 in modeled API cost**. No Tura runs are included.

| What Codex consumed | Share of all tokens | Share of cost |
| ------------------- | ------------------: | ------------: |
| Cached input        |          **96.46%** |    **63.91%** |
| New uncached input  |               3.16% |    **20.94%** |
| Model output        |               0.38% |    **15.14%** |

The complete calculation is in the repository's [140-run summary](https://github.com/Tura-AI/benchmark/blob/main/assets/plugin-token-savings/summary.json). Under the repository pricing model, uncached input costs $5/M, cached input $0.50/M, and output $30/M. Cached input is one tenth the price of new input.

The four published plugin runs had the same shape: cached input was **96.74%** of Ponytail tokens and **97.27%** of RTK tokens. Apparently the denominator did not install the plugin.

A coding agent repeatedly carries prompt, history, commands, and command results into later rounds. Shortening one fragment can produce an impressive local percentage while barely touching the expensive complete trajectory.

## Prompt and LOC savings are especially good comedy

Ponytail's Codex rules contain about **569 tokens**. Give the claim every advantage: put those rules in every one of the 10,365 rounds and shorten them by 90% with zero quality loss. The modeled saving across all 140 runs is about **$2.98**, or **0.44%** of total cost.

That is roughly two cents per task. Please alert the finance department.

The LOC argument is worse. Recoverable final production code in the 140 runs contains **512,412 tokens**, only **0.0568%** of all tokens consumed. Suppose Ponytail magically removes **80% of every functional code token**, never deletes behavior, and never causes another reasoning step. Even valuing every removed token at the expensive output rate, the saving is **1.81%** of total task cost.

Less code can be better engineering. But using LOC reduction as evidence of a huge inference-cost reduction is like shortening item names on a restaurant bill and announcing that dinner is cheaper.

## RTK's 90% still belongs to a tiny slice

Across the 140 runs, we could uniquely classify **1,082 RTK-supported shell calls** containing **1,458,927 returned tokens**. That payload is just **0.1618%** of all task tokens. Apply a perfect 90% reduction to every eligible return and the directly attributable modeled saving is **0.96%** of total cost.

To manufacture a larger ceiling, we also assumed every compressible output remains in context until the task ends and gets reread on every later round. Under that deliberately generous fantasy, universal lossless 90% compression reaches **5.72%**.

So the marketing number can be 90% while the complete-task saving stays below 1%. It approaches 5% only after we grant permanent retention, perfect classification, perfect compression, and zero information loss. The rabbit is real; the hat is doing most of the work.


## Real Token Saving

Tura is a local, open-source coding agent for developers who are tired of vague skill claims, token-saving extensions with no evidence, and agents without judgment wreck their repos.

Across 20 DeepSWE v1.1 tasks, tested 60 sessions with GPT-5.6 SOL at High reasoning effort, Tura creates a substantial token-budget advantage by reducing repeated context and model round trips. You can spend that advantage in two ways. Direct turns most of it into lower cost: 83.5% fewer aggregate tokens than the official Codex CLI High configuration, with a verifier success rate of 65.0% versus 60.0%. Balanced puts more of the saved budget back into reasoning, investigation, and verification. It reached an 80.0% success rate—20 percentage points higher than Codex CLI High—while still using 49.6% fewer tokens

<img src="https://raw.githubusercontent.com/Tura-AI/tura/refs/heads/main/assets/data/benchmark-agent-comparison.svg" alt="High-to-High benchmark comparison" width="800">

[BENCHMARK](https://turaai.net/benchmark)

## One outside paper is enough

[Bai et al., _How Do AI Agents Spend Your Money?_](https://arxiv.org/abs/2604.22750) analyze trajectories from eight frontier models on SWE-bench Verified. They report that agentic coding consumes about **1,000x** more tokens than code reasoning or code chat, that **input rather than output drives total consumption**, and that runs on the same task can differ by up to **30x**. Higher token use also did not reliably mean higher accuracy.

That is the only external paper needed here. Coding-agent cost is a trajectory problem with huge run-to-run variance. A local compression ratio is not a task-level economic result. It is a numerator looking for an unsuspecting denominator.


## Dataset

The matched-run package lives in [`blog_data`](https://github.com/Tura-AI/benchmark/tree/main/blog_data/token-saving-plugin-eza). The broader distribution and scenario report lives in [`assets/plugin-token-savings`](https://github.com/Tura-AI/benchmark/tree/main/assets/plugin-token-savings).

Ponytail may be useful as an anti-overengineering discipline. RTK may be useful as a terminal-output formatter. Test those benefits honestly. Just stop waving "90%" around as if percentages are transferable between denominators.

They are not. The calculator is free.
