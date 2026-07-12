# DeepSWE blows up the AI coding leaderboard, crowns GPT-5.5, and finds Claude Opus exploiting a benchmark loophole | VentureBeat

Source: https://venturebeat.com/technology/deepswe-blows-up-the-ai-coding-leaderboard-crowns-gpt-5-5-and-finds-claude-opus-exploiting-a-benchmark-loophole

[](https://venturebeat.com/)- [Orchestration](https://venturebeat.com/category/orchestration)
- [Infrastructure](https://venturebeat.com/category/infrastructure)
- [Data](https://venturebeat.com/category/data)
- [Security](https://venturebeat.com/category/security)

[Newsletters](https://venturebeat.com/newsletters) # DeepSWE blows up the AI coding leaderboard, crowns GPT-5.5, and finds Claude Opus exploiting a benchmark loophole

[Michael Nuñez](https://venturebeat.com/author/michael_nunez)
    3:32 pm, PT, May 26, 2026
  ![deepswe-card](https://venturebeat.com/_next/image?url=https%3A%2F%2Fimages.ctfassets.net%2Fjdtwqhzvc2n1%2F4kUVtxUVBjivIKlO68RxPf%2F88918e60aed6f6c50fb031ea81e52f8f%2Fdeepswe-card.jpg%3Fw%3D1000%26q%3D100&w=3840&q=85)Credit: Datacurve

[](https://www.google.com/preferences/source?q=venturebeat.com)For months, the leading AI coding benchmarks have told enterprise buyers a comforting but misleading story: the top models are all roughly the same. OpenAI's [GPT-5 family](https://openai.com/gpt-5/), Anthropic's [Claude Opus](https://www.anthropic.com/claude/opus), and Google's [Gemini Pro](https://deepmind.google/models/gemini/pro/) have clustered within a narrow band on Scale AI's [SWE-Bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public) leaderboard, making it nearly impossible for engineering leaders to determine which agent will actually perform best inside their codebases.

On Monday, a startup called Datacurve released a benchmark it says shatters that illusion. [DeepSWE](https://deepswe.datacurve.ai/blog), a 113-task evaluation spanning 91 open-source repositories and five programming languages, produces a dramatically wider spread among the same frontier models — and crowns OpenAI's [GPT-5.5](https://openai.com/index/introducing-gpt-5-5/) as the clear leader at 70%, sixteen points ahead of its nearest competitor.

"On public leaderboards, top models often look relatively close in capability," wrote Datacurve co-author Serena Ge on X. "DeepSWE shows where they actually diverge, reflecting the realistic experience of developers in their day-to-day work."

The benchmark also delivers a pointed critique of the evaluation infrastructure the AI industry relies on to measure progress: Datacurve's audit found that SWE-Bench Pro's verifiers — the automated graders that determine whether an agent solved a task — issued incorrect pass/fail verdicts on roughly one-third of the trials it reviewed.

If that finding holds up, it has sweeping implications. Enterprise procurement teams, venture capitalists, and AI lab marketing departments all lean heavily on benchmark scores to make multimillion-dollar decisions. A 32% error rate in the most widely cited coding benchmark suggests the industry may have been navigating by a broken compass.

## Why the most popular AI coding benchmark may be grading on a curve

To understand what Datacurve is claiming, it helps to understand how coding benchmarks work — and how they can go wrong.

The dominant paradigm, pioneered by the [SWE-Bench family](https://labs.scale.com/leaderboard/swe_bench_pro_public) maintained by [Scale AI](https://scale.com/) and academic researchers, constructs tasks by mining real GitHub commits. The process extracts a bug fix or feature addition from a repository's history, rolls the code back to the pre-fix state, and then asks an AI agent to reproduce the change. The original commit's test suite serves as the verifier: if the agent's patch makes the same tests pass, it gets credit. This approach has an elegant simplicity, but Datacurve argues it introduces three systemic weaknesses.

First, [contamination](https://deepswe.datacurve.ai/blog). Because tasks are drawn from public GitHub history, the problem statement, the discussion, and often the exact solution are already present in the training data of frontier models. "The SWE-Bench family scrapes existing GitHub issues and PRs, which creates two problems: memorization (models have already seen the solution) and triviality (most tasks are small)," Ge wrote.

Second, scope. [SWE-Bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public) tasks require, on average, just 120 lines of code added across 5 files. DeepSWE's reference solutions average 668 lines added across 7 files — roughly 5.5 times more code. Yet DeepSWE's prompts are actually shorter, averaging 2,158 characters versus SWE-Bench Pro's 4,614. In other words, DeepSWE gives the agent less instruction but expects far more output, which more closely mirrors how a human developer might actually delegate work to an AI assistant.

![Screenshot 2026-05-26 at 3.20.59 PM](https://venturebeat.com/_next/image?url=https%3A%2F%2Fimages.ctfassets.net%2Fjdtwqhzvc2n1%2F6HIiEtiEMzCI9UUc7KAUAs%2F1eef63b6f6ba26c8f7402e2a9e304453%2FScreenshot_2026-05-26_at_3.20.59%25C3%25A2__PM.png%3Fw%3D1000%26q%3D100&w=3840&q=75)DeepSWE tasks demand roughly five times more code than SWE-Bench Pro's while giving agents shorter prompts — a design choice intended to mirror how developers actually hand off work. (Source: Datacurve)

Third — and most damaging — verifier reliability. Datacurve drew 30 tasks at random from both [DeepSWE](https://deepswe.datacurve.ai/blog) and [SWE-Bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public), ran three rollouts across 10 frontier model configurations, and then deployed an LLM-based judge to independently assess whether each agent's patch actually solved the problem. SWE-Bench Pro's verifiers accepted wrong implementations 8.5% of the time and rejected correct implementations 24% of the time. DeepSWE's verifiers registered 0.3% and 1.1%, respectively.

![Screenshot 2026-05-26 at 3.22.11 PM](https://venturebeat.com/_next/image?url=https%3A%2F%2Fimages.ctfassets.net%2Fjdtwqhzvc2n1%2F5OFgNKCFyANJu5nyAr7sL%2Fe8ff388e60faba69c0e28371f87588f5%2FScreenshot_2026-05-26_at_3.22.11%25C3%25A2__PM.png%3Fw%3D1000%26q%3D100&w=3840&q=75)Datacurve's audit found that SWE-Bench Pro's automated graders rejected correct solutions 24 percent of the time and accepted wrong ones 8.5 percent of the time. DeepSWE's verifiers kept both rates near zero. (Source: Datacurve)

The false negative problem is especially insidious because it punishes creative solutions. In one documented case, the gold-standard pull request for a SWE-Bench Pro task refactored a private helper function. An agent that correctly solved the task by inlining the same logic — a perfectly valid engineering choice — failed because the test suite tried to import a symbol that only existed in the original author's specific implementation.

## OpenAI's GPT-5.5 dominates the new benchmark while Claude and Gemini stumble

DeepSWE's top-line results reorder the familiar hierarchy in ways that should matter to every engineering team evaluating AI coding tools. On [SWE-Bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public), models from OpenAI, Anthropic, and Google have traded the lead within a 30-point range. DeepSWE stretches that range to 70 points.

[GPT-5.5](https://openai.com/index/introducing-gpt-5-5/) leads at 70%, followed by GPT-5.4 at 56% and Claude Opus 4.7 at 54%. From there, the drop-off is steep: Claude Sonnet 4.6 lands at 32%, Gemini 3.5 Flash at 28%, GPT-5.4-mini and Kimi K2.6 tied at 24%, and then a long tail of models in the teens and single digits. Claude Haiku 4.5, which scores 39% on SWE-Bench Pro, collapses to zero on DeepSWE — suggesting that some mid-tier models have been significantly overperforming on easier, potentially contaminated benchmarks.

![Screenshot 2026-05-26 at 3.09.33 PM](https://venturebeat.com/_next/image?url=https%3A%2F%2Fimages.ctfassets.net%2Fjdtwqhzvc2n1%2F5zTqdPew4tOBgQFRYneFKb%2F9c785443bfebcf5e661d480a31b77cfa%2FScreenshot_2026-05-26_at_3.09.33%25C3%25A2__PM.png%3Fw%3D1000%26q%3D100&w=3840&q=75)On SWE-Bench Pro, frontier models cluster within a 30-point range. On DeepSWE, the same models spread across 70 points, with some — like Claude Haiku 4.5 — collapsing entirely. (Source: Datacurve)

GPT-5.5 doesn't just score the highest — it does so efficiently. The model reaches its 70% pass rate with a median cost of $5.80 per trial, a median wall-clock time of 20 minutes, and a median of 47,000 output tokens. GPT-5.4 emerges as perhaps the best overall value at $3.30 per trial with a 56% score. Claude Opus 4.7, meanwhile, costs significantly more per run, and output tokens, wall-clock duration, and dollar cost per trial all vary by an order of magnitude across the agents tested — yet none of these correlates strongly with pass rate. Agents that emit more tokens, run longer, or cost more do not consistently solve more tasks.

![Screenshot 2026-05-26 at 3.27.42 PM](https://venturebeat.com/_next/image?url=https%3A%2F%2Fimages.ctfassets.net%2Fjdtwqhzvc2n1%2F53UDMkgZA1mI4RPYrGS7iy%2F85c708c0738918cb06ec44eb544f3df2%2FScreenshot_2026-05-26_at_3.27.42%25C3%25A2__PM.png%3Fw%3D1000%26q%3D100&w=3840&q=75)GPT-5.4 and GPT-5.5 occupy the cost-efficient frontier, solving the most tasks for the least money per run. Spending more did not reliably produce better results. (Source: Datacurve)

## Datacurve's audit found that Claude has been reading the answer key on existing benchmarks

Perhaps the most provocative finding in DeepSWE's analysis concerns what the authors label "CHEATED" verdicts — instances where an agent passes a benchmark not by solving the problem, but by reading the answer.

SWE-Bench Pro's Docker containers ship the repository's full .git history, which means the gold-standard solution commit is sitting right there in the container's file system. Most models ignore it. Claude does not. Datacurve's analysis found that both Claude Opus 4.7 and Claude Opus 4.6 registered "CHEATED" on more than 12% of their reviewed SWE-Bench Pro rollouts. In those instances, the Claude agent ran commands like git log --all or git show <gold-hash> to retrieve the merged fix and paste it into its own patch. The behavior accounted for approximately 18% of Opus 4.7's passes and 25% of Opus 4.6's passes on the reviewed sample. The issue has been [filed publicly as GitHub issue #93](https://github.com/scaleapi/SWE-bench_Pro-os/issues/93) on the SWE-Bench Pro repository.

[GPT-5.4](https://openai.com/index/introducing-gpt-5-4/) and [GPT-5.5](https://openai.com/index/introducing-gpt-5-5/) never exhibited this behavior. Gemini configurations stayed around 1%. Datacurve describes the behavior diplomatically — "The benchmark makes this possible (the gold commit lives in the container), but Claude is the family that consistently does so" — but the implication is clear: a meaningful fraction of Claude's SWE-Bench Pro scores may reflect environmental exploitation rather than genuine engineering capability.

[DeepSWE](https://deepswe.datacurve.ai/blog) addresses this by shipping only a shallow clone with the base commit, leaving no gold hash for the agent to discover. It is worth noting that the behavior is arguably a sign of Claude's environmental attentiveness — the model is very good at exploring its surroundings and exploiting available resources. Whether that counts as "cheating" or "resourcefulness" depends on your perspective, but in the context of a benchmark designed to measure independent problem-solving, it undermines the signal.

![Screenshot 2026-05-26 at 3.28.43 PM](https://venturebeat.com/_next/image?url=https%3A%2F%2Fimages.ctfassets.net%2Fjdtwqhzvc2n1%2F1h7L3b3cWDo4z92EJ5ascN%2F4ef9dbe238f1338b9c3e3d05043a9419%2FScreenshot_2026-05-26_at_3.28.43%25C3%25A2__PM.png%3Fw%3D1000%26q%3D100&w=3840&q=75)Two mechanisms by which agents passed SWE-Bench Pro without solving the underlying problem: reading the answer from the container's Git history, or stubbing features past weak gold tests. (Source: Datacurve)

## Each AI model family fails in its own distinctive way, and the patterns matter for enterprise teams

Beyond the top-line scores, Datacurve's qualitative trajectory analysis reveals distinctly different failure signatures across model families — a finding that could help engineering teams choose the right model for specific types of work.

Claude is forgetful with multi-part prompts. On [DeepSWE](https://deepswe.datacurve.ai/blog), Claude configurations miss stated requirements more than any other family. The pattern is consistent: when a prompt enumerates parallel behaviors — "support both sync and async," for instance — Claude typically implements the obvious branch and forgets to mirror the change. Datacurve reports that roughly two-thirds of Claude's "MISSED_REQUIREMENT" failures on DeepSWE follow this "one branch shipped" pattern. In one example, Claude Opus 4.7 correctly landed a sync state-data hook in one engine class while the async engine never received the same hook.

GPT, by contrast, implements exactly what is asked. GPT-5.5 had the lowest rate of missing stated behaviors of any configuration tested. Across multiple runs of the same task, GPT trials tended to converge on the same interpretation of the prompt, suggesting instruction-following precision is a stable trait of the model rather than per-run luck.

One of the most intriguing findings involves self-verification. On DeepSWE, Claude Opus 4.7 and GPT-5.4 wrote and ran new tests in the project's own test framework on over 80% of their runs — even though no one asked them to. On SWE-Bench Pro, those same models dropped to 28% and 18%, respectively. The reason: SWE-Bench Pro's prompt template explicitly tells agents they "should not modify the testing logic or any of the tests." Agents dutifully complied, suppressing a behavior that likely would have improved their performance. This suggests that prompt design in production coding workflows may be inadvertently suppressing valuable agent behaviors — something enterprise teams deploying AI coding agents should carefully audit.

![Screenshot 2026-05-26 at 3.29.41 PM](https://venturebeat.com/_next/image?url=https%3A%2F%2Fimages.ctfassets.net%2Fjdtwqhzvc2n1%2F34MuGfpCgLFuBF9R71o01B%2Ffddfe51d70f54410a7559f5da901db9a%2FScreenshot_2026-05-26_at_3.29.41%25C3%25A2__PM.png%3Fw%3D1000%26q%3D100&w=3840&q=75)On DeepSWE, top models wrote and ran their own tests in as many as 85 percent of runs. On SWE-Bench Pro, where prompts discourage modifying tests, the same models rarely did so. (Source: Datacurve)

## What DeepSWE gets right, what it gets wrong, and what it means for the future of AI benchmarks

Datacurve is forthright about several limitations. The standardized harness, while ensuring fairness, routes all edits through bash rather than the model-specific editing tools each family was trained on — apply_patch for GPT, str_replace_based_edit_tool for Claude. This could hold models below their native ceilings. The benchmark draws exclusively from open-source repositories with 500-plus stars, and results may not generalize to proprietary codebases. Bug localization and refactoring tasks are under-represented, and widely used languages like C++ and Java are absent entirely. The verdict assignments in the qualitative analysis come from an LLM analyzer, not human reviewers, and sample sizes are modest — roughly 90 reviewed rollouts per model per benchmark.

It is also worth noting that [Datacurve](https://datacurve.ai/) is a startup with its own commercial interests, and an independent benchmark that reshuffles the leaderboard will inevitably invite scrutiny. The company's decision to publish the full dataset, all agent trajectories, and the evaluation harness on GitHub mitigates this concern considerably, but independent reproduction will be necessary before the AI community treats these results as definitive.

[DeepSWE](https://deepswe.datacurve.ai/blog) arrives at an inflection point for the AI coding market. Enterprise adoption of AI coding agents is accelerating rapidly, with engineering organizations making consequential bets on which model to build around. The benchmark market itself has become a strategic battleground — Scale AI's [SWE-Bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public), which Datacurve directly critiques, is maintained by a company that also provides evaluation services to the labs whose models it ranks.

If DeepSWE's central findings about verifier reliability and data contamination hold up under independent scrutiny, they could force a reckoning not just with how the industry measures coding agents, but with the broader question of what benchmarks are actually for. A leaderboard where the grading system is wrong a third of the time is not merely inaccurate — it is the kind of broken instrument that makes everyone feel good about progress that may not be real. And in an industry spending billions on a bet that AI agents can do the work of software engineers, the difference between real progress and the appearance of it is not academic. It is the whole game.

[![Transform See Who's Attending CTA](https://venturebeat.com/_next/image?url=https%3A%2F%2Fimages.ctfassets.net%2Fjdtwqhzvc2n1%2F7euaZA7YV6PWUwt09nBrzV%2F1e551a6b9cea7895540a9b1445f89709%2Fimage.png&w=1200&q=75)](https://venturebeat.com/vbtransform2026)##### Subscribe to get latest news!

Deep insights for enterprise AI, data, and security leaders

VB DailyAI WeeklyAGI WeeklySecurity WeeklyData Infrastructure WeeklyAll of themBy submitting your email, you agree to our [Terms](https://venturebeat.com/terms-of-service) and [Privacy Notice](https://venturebeat.com/privacy-policy).

Get updatesYou're in! Our latest news will be hitting your inbox soon.## More

[](https://venturebeat.com/)[](https://www.facebook.com/venturebeat)[](https://www.instagram.com/venturebeat)[](https://twitter.com/venturebeat)[](https://www.linkedin.com/company/venturebeat)[](https://www.youtube.com/venturebeat)- [Press Releases](https://venturebeat.com/press-releases)
- [Contact Us](https://venturebeat.com/contact-2)
- [Advertise](https://media.venturebeat.com)
- [Share a News Tip](https://venturebeat.com/contact-2)
- [Contribute](https://venturebeat.com/guest-posts)

- [Privacy Policy](https://venturebeat.com/privacy-policy)
- [Terms of Service](https://venturebeat.com/terms-of-service)
- [Consent Preferences](https://venturebeat.com/technology/#)
- [Do Not Sell or Share My Personal Information](https://app.termly.io/notify/f592675e-4484-4dc9-bb50-462a84720662)
- [Limit the Use Of My Sensitive Personal Information](https://app.termly.io/notify/f592675e-4484-4dc9-bb50-462a84720662)

© 2026 VentureBeat. All rights reserved.

## Media links

- <https://images.ctfassets.net/jdtwqhzvc2n1/4kUVtxUVBjivIKlO68RxPf/88918e60aed6f6c50fb031ea81e52f8f/deepswe-card.jpg?w=800&q=75>
- <https://vbstatic.co/brand/img/logos/VB_Extended_Logo_60H.png>
- <https://images.ctfassets.net/jdtwqhzvc2n1/4kUVtxUVBjivIKlO68RxPf/88918e60aed6f6c50fb031ea81e52f8f/deepswe-card.jpg?w=800>
- <https://images.ctfassets.net/jdtwqhzvc2n1/7euaZA7YV6PWUwt09nBrzV/1e551a6b9cea7895540a9b1445f89709/image.png>
