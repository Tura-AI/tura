# Token-Saving Plugins: The Denominator Matters

A coding-agent plugin says it can cut 90% of tokens. That may be true. The first question is: 90% of what?

A command-output compressor can remove most of the text from one shell response and still make almost no difference to the final bill. There is no contradiction there. The local percentage describes the fragment the plugin touched. The bill covers the whole trajectory: every model request, repeated context, cache read, tool call, recovery step, and final answer needed to finish the task.

After comparing our own small experiment with larger independent tests, I think the fair conclusion is more useful than either “these plugins work” or “these plugins are useless”:

- Compressing a narrow slice of context often has little effect on the complete-task bill.
- Changing agent behavior can produce measurable savings, but the effect is usually smaller and more workload-dependent than a local headline percentage.
- For long-horizon work, the number of model round trips is often the highest-leverage variable because every additional request can carry prompt, tool schemas, and accumulated history again.
- Fewer rounds are not the only possible mechanism. Shorter model output, less unnecessary implementation work, better cache use, and smaller repeated context can also matter. What matters is whether the complete trajectory changes without reducing task quality.

## What we measured on a repository rewrite

[Full benchmark](https://turaai.net/benchmark)

Our first experiment was a demanding one: rewrite the Rust `eza` repository as a behavior-compatible Python implementation and face 52 harness assertions. We tested two plugins on that task.

Every published run used GPT-5.6-sol, High reasoning, and Codex CLI 0.144.1. The comparison contains two runs per arm:

- Ponytail r2/r3, with full hook and skill activation;
- RTK r2/r3, with isolated RTK activation; and
- two previously published no-plugin runs using the same task, model, reasoning level, and CLI version.

| Arm                         |   n | Harness score | Total tokens | Modeled cost | Rounds | Duration |
| --------------------------- | --: | ------------: | -----------: | -----------: | -----: | -------: |
| No plugin                   |   2 |        78.85% |       6.660M |    $5.281946 |   62.5 |     895s |
| Ponytail, full hook + skill |   2 |        80.77% |       -7.56% |       -8.87% | -9.60% |  +13.51% |
| RTK                         |   2 |        76.92% |      +13.20% |       +7.18% | +44.00% |  +40.69% |

The sanitized [per-run data](https://github.com/Tura-AI/benchmark/blob/main/blog_data/token-saving-plugin-eza/runs.json), [computed summary](https://github.com/Tura-AI/benchmark/blob/main/blog_data/token-saving-plugin-eza/summary.json), [methodology](https://github.com/Tura-AI/benchmark/blob/main/blog_data/token-saving-plugin-eza/methodology.json), and [293-round activation audit](https://github.com/Tura-AI/benchmark/blob/main/blog_data/token-saving-plugin-eza/round-activation-audit.jsonl) are public. All six processes exited successfully and produced complete usage and evaluator data. A run could still miss harness assertions; that is reflected in its score.

The numbers are interesting, but they are descriptive, not causal estimates. With only two runs per arm, they are not enough to identify a plugin effect.

## The two-run result is a direction, not a verdict

The same agent, model, task, and configuration produced materially different bills across the two repetitions:

| Arm       | Cost in the two runs  | Cost range / mean | Token range / mean | Round range / mean |
| --------- | --------------------: | ----------------: | -----------------: | -----------------: |
| No plugin | $4.139647 - $6.424245 |            43.25% |             53.02% |             40.00% |
| Ponytail  | $3.569452 - $6.057281 |            51.69% |             57.36% |             47.79% |
| RTK       | $4.789893 - $6.532388 |            30.78% |             39.75% |             26.67% |

“Range / mean” is the difference between the two runs divided by their mean. It is not a confidence interval.

Ponytail's observed 8.87% lower mean cost sits inside a 51.69% within-arm swing. RTK's observed 7.18% higher mean cost sits inside a 30.78% swing. Even the no-plugin pair moved 43.25% without a treatment change.

So I would not use these six runs to declare a winner or a loser. What they told us was narrower: the local saving claims did not reliably predict the end-to-end result, and we needed more repeated, paired evidence.

## This is where independent testing helped

JetBrains subsequently published larger paired evaluations using SkillsBench and Claude Code. They tested different tasks, a different agent, and more repetitions, with activation audits, quality checks, and paired statistical tests. That makes the comparison genuinely useful rather than a second copy of our setup.

In its [RTK evaluation](https://blog.jetbrains.com/ai/2026/07/rtk-claude-code-token-savings/), JetBrains tested 86 tasks at both low and high reasoning effort. Across 80 clean pairs at low effort, RTK was associated with a median 7.6% higher cost, 13.8% more turns, and 14.3% more cache reads. At high effort, the cost and turn differences were effectively zero. The study did not detect a quality difference in either condition.

This lines up with two mechanisms visible in our smaller experiment. First, RTK can directly compress only the command outputs that pass through its supported shell paths, which are a subset of total session context. Second, extra turns or re-reads can offset local output compression. The high-effort result is just as important: when the number of turns did not change, the measured cost penalty did not reproduce. RTK should not be described as universally harmful either.

JetBrains' [Ponytail evaluation](https://blog.jetbrains.com/ai/2026/07/ponytail-skill-claude-tested/) provides an important counterexample to any categorical claim that token-saving plugins are ineffective. Across 80 paired tasks, the study measured a median 10.3% lower cost and about 15% less code written, with no statistically detectable quality difference. The reduction was smaller than the advertised figures and was concentrated in tasks where the baseline agent had room to over-build. The authors also note that their quality test was not designed to prove equivalence or evaluate every security, accessibility, and validation concern.

I read the two studies together this way: local compression alone is not a reliable proxy for task-level savings, but a plugin that changes the agent's decisions and amount of work can produce a modest, measurable benefit on suitable workloads. Ponytail is evidence for the positive case, and it belongs in the conclusion.

## The part that matters most: model round trips

This is the part I think most token-saving discussions miss. A long-running coding agent repeatedly sends some combination of system instructions, tool schemas, task context, and accumulated history to the model. Prompt caching lowers the price of repeated input, but it does not make another request free. One extra model round can therefore matter more than many small reductions inside one tool result.

Our [MCP workflow benchmark report](https://turaai.net/blog#what-we-learned-from-the-mcp-workflow-benchmark) illustrates the execution shape. In one matched ecommerce workflow, Codex CLI completed the verified task in 11 model requests, while Tura Direct completed the same checks in 3 requests by passing successful tool outputs between dependent steps inside a macro. Direct made more commands and more MCP calls, but used 78.6% fewer total tokens and had a 62.4% lower estimated API-equivalent cost in that pair.

The useful difference was not the number of tools used. It was how often the model had to receive accumulated context just to decide the next call. The runtime could copy a returned identifier into a later step without asking the model to perform another read-decide-call cycle.

That example has clear limits. Tura-AI authored the benchmark and develops Tura; the services were deterministic mocks; and a single paired workflow does not isolate one causal feature. The broader pilot covered 90 runs, but still used one model and one reasoning level. The report discusses those audit boundaries directly. It should be read as evidence that request topology deserves measurement, not as a universal product ranking.

## The shape of the bill explains why

Our broader repository dataset contains 140 Codex CLI Medium and High runs: 10,365 agent rounds, 901,608,531 tokens, and $680.34 in modeled API cost. No Tura runs are included.

| What Codex consumed | Share of all tokens | Share of cost |
| ------------------- | ------------------: | ------------: |
| Cached input        |              96.46% |        63.91% |
| New uncached input  |               3.16% |        20.94% |
| Model output        |               0.38% |        15.14% |

The calculation is published in the [140-run summary](https://github.com/Tura-AI/benchmark/blob/main/assets/plugin-token-savings/summary.json). Under the repository's pricing model, uncached input costs $5/M, cached input $0.50/M, and output $30/M.

This is why I keep coming back to the denominator. Cached input accounted for 96.74% of Ponytail tokens and 97.27% of RTK tokens in the four plugin runs. A tool can substantially shorten one newly returned command result while leaving the accumulated history and the number of times it is reread largely unchanged.

The broader conclusion is also consistent with [Bai et al., _How Do AI Agents Spend Your Money?_](https://arxiv.org/abs/2604.22750), which reports that agentic coding is dominated by input consumption and that trajectories for the same task can vary substantially. This is another reason to use repeated, paired task-level measurements rather than extrapolating from one transformed fragment.

## Do the denominator math before paying for a benchmark

A simple exposure analysis can tell us when a local optimization is too narrow to support a large end-to-end claim.

Across the 140 runs, we classified 1,082 shell calls supported by RTK. Their returned payload contained 1,458,927 tokens, or 0.1618% of all task tokens. Applying a lossless 90% reduction to every eligible return produces a directly attributable modeled saving of 0.96% of total cost under our assumptions.

We also calculated a deliberately generous upper scenario in which every compressible result remains in context until the task ends and is reread on every subsequent round. With universal classification, lossless 90% compression, and no recovery behavior, the modeled saving reaches 5.72%. This is an upper scenario, not an observed result.

Prompt and line-of-code reductions require similar care. Ponytail's Codex rules contain about 569 tokens. If those rules appeared in every round and could be shortened by 90% with no behavioral change, the modeled saving across the dataset would be about 0.44% of total cost. Recoverable final production code represented 0.0568% of all consumed tokens. Fewer lines can improve maintainability and can indirectly change the trajectory, but line count alone is not a task-level cost measure.

This ceiling calculation cannot predict every behavioral effect. Ponytail is a good example: its rules can change what the agent chooses to build, not merely the length of the rules themselves. Once behavior changes, the only honest place to measure the effect is the full run.

## A practical evaluation standard

A credible token-saving evaluation should report at least:

1. **Complete-task cost and token usage**, not only bytes removed from one prompt or tool result.
2. **Verified task quality**, so savings are not purchased by leaving work incomplete.
3. **Model requests or turns**, cache reads, and tool calls, so the mechanism can be inspected.
4. **Paired tasks with repeated runs**, because agent trajectories have high variance.
5. **Confirmed activation**, distinguishing “the treatment had no effect” from “the plugin did not run.”
6. **The affected share of the denominator**, including which context paths the plugin cannot touch.
7. **Workload and model boundaries**, because an effect at one reasoning level or task mix may not transfer to another.

## Conclusion

Token-saving plugins are not one category with one verdict. RTK-style command-output compression can be useful for readability and may help on workloads dominated by supported shell output, but the task-level tests available today have not shown the advertised end-to-end savings. Ponytail-style guidance can reduce unnecessary work and has produced a statistically supported, workload-dependent cost reduction in an independent test.

For large savings on long-horizon tasks, the strongest mechanism is usually a change in the complete execution trajectory: fewer model round trips, less repeated context, less unnecessary generation, or fewer recovery steps. If none of those quantities changes, a large local compression percentage is unlikely to become a large reduction in the final bill.

So the question I would ask is not “How much text did the plugin remove?” It is “Did the agent reach the same verified outcome with a meaningfully cheaper trajectory?”

## Data and reports

- [Matched plugin-run package](https://github.com/Tura-AI/benchmark/tree/main/blog_data/token-saving-plugin-eza)
- [Broader token-distribution and scenario report](https://github.com/Tura-AI/benchmark/tree/main/assets/plugin-token-savings)
- [Tura MCP workflow benchmark report](https://turaai.net/blog#what-we-learned-from-the-mcp-workflow-benchmark)
- [JetBrains RTK evaluation](https://blog.jetbrains.com/ai/2026/07/rtk-claude-code-token-savings/)
- [JetBrains Ponytail evaluation](https://blog.jetbrains.com/ai/2026/07/ponytail-skill-claude-tested/)
