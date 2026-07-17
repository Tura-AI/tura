# OpenAI GPT-5.6 Sol and Terra: Benchmark

Source: https://www.coderabbit.ai/blog/gpt-5-6-sol-and-terra-benchmark

CodeRabbit is now in the Claude Marketplace![Read the Claude Marketplace announcement](https://www.coderabbit.ai/blog/CodeRabbit-in-Claude-Marketplace)[![CodeRabbit logo](https://www.coderabbit.ai/images/logo-orange.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)![CodeRabbit logo](https://www.coderabbit.ai/images/logo-dark.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://www.coderabbit.ai/)[Agent](https://www.coderabbit.ai/agent)[Enterprise](https://www.coderabbit.ai/enterprise)[Customers](https://www.coderabbit.ai/customers)[Pricing](https://www.coderabbit.ai/pricing)[Blog](https://www.coderabbit.ai/blog)Resources- [Docs](https://docs.coderabbit.ai)
- [Trust Center](https://trust.coderabbit.ai/)
- [Contact Us](https://www.coderabbit.ai/contact-us)
- [FAQ](https://www.coderabbit.ai/faq)
- [Reports & Guides](https://www.coderabbit.ai/whitepapers)

[Log In](https://app.coderabbit.ai/login)[Get a free trial](https://app.coderabbit.ai/login?free-trial)[![CodeRabbit logo](https://www.coderabbit.ai/images/CR_mark_orange.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)![CodeRabbit logo](https://www.coderabbit.ai/images/CR_mark_orange.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://www.coderabbit.ai/)Products

[Agent](https://www.coderabbit.ai/agent)[Discord](https://www.coderabbit.ai/discord)[Pull Request Reviews](https://www.coderabbit.ai/#features)[IDE Reviews](https://www.coderabbit.ai/ide)[CLI Reviews](https://www.coderabbit.ai/cli)[Plan](https://www.coderabbit.ai/plan)[OSS](https://www.coderabbit.ai/oss)Navigation

[About Us](https://www.coderabbit.ai/about-us)[Features](https://www.coderabbit.ai/?#features)[FAQ](https://www.coderabbit.ai/faq)[System Status](https://status.coderabbit.ai)[Careers](https://www.coderabbit.ai/careers)[DPA](https://www.coderabbit.ai/dpa)[Startup Program](https://www.coderabbit.ai/startup-program)[Vulnerability Disclosure](https://www.coderabbit.ai/vulnerability-disclosure)Resources

[Blog](https://www.coderabbit.ai/blog)[Docs](https://docs.coderabbit.ai/)[Changelog](https://docs.coderabbit.ai/changelog/)[Case Studies](https://www.coderabbit.ai/case-studies)[Trust Center](https://trust.coderabbit.ai/)[Brand Guidelines](https://www.coderabbit.ai/brand)[Reports & Guides](https://www.coderabbit.ai/whitepapers)Contact

[Support](https://www.coderabbit.ai/contact-us/support)[Sales](https://www.coderabbit.ai/contact-us/sales)[Pricing](https://www.coderabbit.ai/pricing)[Partnerships](https://www.coderabbit.ai/partnership)SubscribeBy signing up you agree to our [Terms of Use](https://www.coderabbit.ai/terms-of-service) and authorize CodeRabbit to provide occasional updates about products and solutions. You understand that you can opt out at any time and that your data will be handled in accordance with [CodeRabbit Privacy Policy](https://www.coderabbit.ai/privacy-policy)

[![discord icon](https://www.coderabbit.ai/images/icons/discord-white.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://discord.gg/coderabbit)[![x icon](https://www.coderabbit.ai/images/icons/x-white.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://x.com/coderabbitai)[![linkedin icon](https://www.coderabbit.ai/images/icons/linkedin-white.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://www.linkedin.com/company/coderabbitai)[![rss icon](https://www.coderabbit.ai/images/icons/rss-white.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://www.coderabbit.ai/feed)Select languageEnglish![footer-logo shape](https://www.coderabbit.ai/_next/image?url=%2Fimages%2Fshapes%2Ffooter-logo.png&w=3840&q=75&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)[Terms of Service ](https://www.coderabbit.ai/terms-of-service)[Privacy Policy](https://www.coderabbit.ai/privacy-policy)CodeRabbit, Inc. ©  2026

[![CodeRabbit logo](https://www.coderabbit.ai/images/CR_mark_orange.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)![CodeRabbit logo](https://www.coderabbit.ai/images/CR_mark_orange.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://www.coderabbit.ai/)Products

[Agent](https://www.coderabbit.ai/agent)[Discord](https://www.coderabbit.ai/discord)[Pull Request Reviews](https://www.coderabbit.ai/#features)[IDE Reviews](https://www.coderabbit.ai/ide)[CLI Reviews](https://www.coderabbit.ai/cli)[Plan](https://www.coderabbit.ai/plan)[OSS](https://www.coderabbit.ai/oss)Navigation

[About Us](https://www.coderabbit.ai/about-us)[Features](https://www.coderabbit.ai/?#features)[FAQ](https://www.coderabbit.ai/faq)[System Status](https://status.coderabbit.ai)[Careers](https://www.coderabbit.ai/careers)[DPA](https://www.coderabbit.ai/dpa)[Startup Program](https://www.coderabbit.ai/startup-program)[Vulnerability Disclosure](https://www.coderabbit.ai/vulnerability-disclosure)Resources

[Blog](https://www.coderabbit.ai/blog)[Docs](https://docs.coderabbit.ai/)[Changelog](https://docs.coderabbit.ai/changelog/)[Case Studies](https://www.coderabbit.ai/case-studies)[Trust Center](https://trust.coderabbit.ai/)[Brand Guidelines](https://www.coderabbit.ai/brand)[Reports & Guides](https://www.coderabbit.ai/whitepapers)Contact

[Support](https://www.coderabbit.ai/contact-us/support)[Sales](https://www.coderabbit.ai/contact-us/sales)[Pricing](https://www.coderabbit.ai/pricing)[Partnerships](https://www.coderabbit.ai/partnership)Select languageEnglishSubscribeBy signing up you agree to our [Terms of Use](https://www.coderabbit.ai/terms-of-service) and authorize CodeRabbit to provide occasional updates about products and solutions. You understand that you can opt out at any time and that your data will be handled in accordance with [CodeRabbit Privacy Policy](https://www.coderabbit.ai/privacy-policy)

[![discord icon](https://www.coderabbit.ai/images/icons/discord-white.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://discord.gg/coderabbit)[![x icon](https://www.coderabbit.ai/images/icons/x-white.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://x.com/coderabbitai)[![linkedin icon](https://www.coderabbit.ai/images/icons/linkedin-white.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://www.linkedin.com/company/coderabbitai)[![rss icon](https://www.coderabbit.ai/images/icons/rss-white.svg?dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://www.coderabbit.ai/feed) # GPT-5.6 Sol and Terra: Where they fit for coding agents and code review

by ![Juan Pablo Flores](https://www.coderabbit.ai/_next/image?url=%2Fcontent%2Fassets%2Fjuanpa.jpeg&w=128&q=75&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)![Gowtham Kishore Vijay](https://www.coderabbit.ai/_next/image?url=%2Fcontent%2Fassets%2Fauthor-gowtham-profile.png&w=128&q=75&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)Juan Pablo Flores

Gowtham Kishore Vijay

July 09, 2026

15 min read

July 09, 2026

15 min read

- [What's new in GPT-5.6](https://www.coderabbit.ai/blog/#heading-whats-new-in-gpt-56)
- [The clearest signal was follow-through](https://www.coderabbit.ai/blog/#heading-the-clearest-signal-was-follow-through)
- [Coding runs: Sol follows through](https://www.coderabbit.ai/blog/#heading-coding-runs-sol-follows-through)
- [What this means for agent loops](https://www.coderabbit.ai/blog/#heading-what-this-means-for-agent-loops)
- [Code review: Sol finds more, then filtering has to earn trust](https://www.coderabbit.ai/blog/#heading-code-review-sol-finds-more-then-filtering-has-to-earn-trust)
- [How Sol compares to Fable 5 and Sonnet 5](https://www.coderabbit.ai/blog/#heading-how-sol-compares-to-fable-5-and-sonnet-5)
- [What Sol costs you](https://www.coderabbit.ai/blog/#heading-what-sol-costs-you)
- [Should you switch?](https://www.coderabbit.ai/blog/#heading-should-you-switch)

[Back to blog](https://www.coderabbit.ai/blog)![Cover image](https://www.coderabbit.ai/_next/image?url=%2Fcontent%2Fassets%2Fgpt5-6.png&w=3840&q=90&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)Share

[![Share on Reddit](https://www.coderabbit.ai/_next/image?url=%2Fcontent%2Fassets%2Freddit.png&w=64&q=75&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://www.reddit.com/submit?url=https://coderabbit.ai/blog/gpt-5-6-sol-and-terra-benchmark&text=GPT-5.6%20Sol%20and%20Terra%3A%20Where%20they%20fit%20for%20coding%20agents%20and%20code%20review&utm_source=blog&utm_medium=web&utm_campaign=social_share_blog)[![Share on X](https://www.coderabbit.ai/_next/image?url=%2Fcontent%2Fassets%2Fx.png&w=64&q=75&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://x.com/intent/tweet?url=https://www.coderabbit.ai/blog/gpt-5-6-sol-and-terra-benchmark&text=GPT-5.6%20Sol%20and%20Terra%3A%20Where%20they%20fit%20for%20coding%20agents%20and%20code%20review&utm_source=blog&utm_medium=web&utm_campaign=social_share_blog)[![Share on LinkedIn](https://www.coderabbit.ai/_next/image?url=%2Fcontent%2Fassets%2Flinked-in.png&w=64&q=75&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](http://www.linkedin.com/shareArticle?url=https://coderabbit.ai/blog/gpt-5-6-sol-and-terra-benchmark&text=GPT-5.6%20Sol%20and%20Terra%3A%20Where%20they%20fit%20for%20coding%20agents%20and%20code%20review&utm_source=blog&utm_medium=web&utm_campaign=social_share_blog)Cut code review time & bugs by 50%

Most installed AI app on GitHub and GitLab

Free 14-day trial

[Get Started](https://coderabbit.link/01oeOX7)## Catch the latest, right in your inbox.

Subscribe[Add us to your feed.![RSS feed icon](https://www.coderabbit.ai/_next/image?url=%2Fimages%2Fblog%2Ffeed.png&w=96&q=75&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://www.coderabbit.ai/feed)![newsletter decoration](https://www.coderabbit.ai/_next/image?url=%2Fimages%2Fblog%2Fnewsletter.png&w=3840&q=75&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)## Catch the latest, right in your inbox.

Subscribe[Add us to your feed.![RSS feed icon](https://www.coderabbit.ai/_next/image?url=%2Fimages%2Fblog%2Ffeed.png&w=96&q=75&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)](https://www.coderabbit.ai/feed)## Keep reading

[![Close the loop after every merge: the agent that reviewed your PR can now follow through](https://www.coderabbit.ai/_next/image?url=%2Fcontent%2Fassets%2Fclose-the-loop-after-every-merge-cover.png&w=3840&q=75&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)### Close the loop after every merge: the agent that reviewed your PR can now follow through

Post-Merge Actions use pull request context to handle changelogs, documentation, tickets, and other work that should happen after merge.

](https://www.coderabbit.ai/blog/close-the-loop-after-every-merge)[![The hidden cost of your security stack](https://www.coderabbit.ai/_next/image?url=%2Fcontent%2Fassets%2Fhidden-cost-security-stack.png&w=3840&q=75&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)### The hidden cost of your security stack

Security tool sprawl creates hidden costs in alerts, context switching, backlogs, and verification work as teams ship more AI-generated code.

](https://www.coderabbit.ai/blog/hidden-cost-security-stack)[![2026 is becoming the year of AI quality](https://www.coderabbit.ai/_next/image?url=%2Fcontent%2Fassets%2F2026-year-of-ai-quality.png&w=3840&q=75&dpl=dpl_2BZLJRcBrFJG7jwsMxcyFoMexwwV)### 2026 is becoming the year of AI quality

As AI makes code generation fast and abundant, the real engineering constraint is shifting from producing code to understanding, trusting, and taking accountability for it.

](https://www.coderabbit.ai/blog/2026-is-becoming-the-year-of-ai-quality)Get
Started in
2 clicks.

No credit card needed

Your browser does not support the video.[Install in VS Code](https://app.coderabbit.ai/login?free-trial)Your browser does not support the video.OpenAI has released [GPT-5.6](https://openai.com/index/gpt-5-6/), and the practical question for engineering teams is where Sol and Terra fit in the coding stack. This review looks at the coding experience, long-running agent work, and CodeRabbit review benchmarks to separate where each model is useful from where the older routing playbook still holds.

If you use [GPT-5.5](https://www.coderabbit.ai/blog/gpt-5-5-benchmark-results) or a previous OpenAI model for coding agents, start testing Sol. It follows through better. It works through messy repo tasks, takes long checklists seriously, and finds more review issues when we run it in our harness. Terra is the cheaper lane to test for scoped work. Its long coding run is a reminder to measure cost per solved task alongside price per token.

Sol does not erase the other frontier models. [Fable 5](https://www.coderabbit.ai/blog/fable-5-model-review) still feels stronger when you want architectural judgment or planning taste. [Sonnet 5](https://www.coderabbit.ai/blog/claude-sonnet-5-review) still has a cleaner comment-quality story in some review workflows. Sol wins on a more practical axis: you can hand it work and expect it to keep pushing.

That is the switching question for this release. Do you need the model that sounds the smartest, or the model that finishes more of the work?

## **What's new in GPT-5.6**

OpenAI describes GPT-5.6 as a family with capability tiers. Sol is the flagship model. Terra is the lower-cost option. Luna is the fastest and lowest-cost tier. The important change is that you can route work by depth now instead of treating GPT-5.6 as one model.

The release changes four things for engineering teams:

1. Sol gives you a stronger long-horizon coding lane. It is the model to use when the task needs persistence across files, tests, and follow-up fixes.
2. Terra gives you a cheaper path for scoped implementation and first-pass review.
3. Luna gives you a low-reasoning lane for high-volume work where speed and cost are the main constraints.
4. Prompt caching is more predictable, with explicit cache breakpoints and a 30-minute minimum cache life.

The price table also makes the competitive shape clearer. Sol is priced below Claude Fable 5 and close to Opus 4.8 on input price, while Terra and Luna create cheaper routing lanes for lighter work.

| GPT-5.6 Sol     | $5.00  | $30.00 |
| --------------- | ------ | ------ |
| GPT-5.6 Terra   | $2.50  | $15.00 |
| GPT-5.6 Luna    | $1.00  | $6.00  |
| Claude Fable 5  | $10.00 | $50.00 |
| Claude Opus 4.8 | $5.00  | $25.00 |

![Bar chart comparing API input and output token pricing for five different AI models.](https://www.coderabbit.ai/content/assets/image3-4.png)

OpenAI also added more predictable prompt caching, including explicit cache breakpoints and a 30-minute minimum cache life. For long agent jobs, that can change how expensive the models can be. A model that reads the same repository for hours looks different once cache reads are working.

Here is the working map:

| Sol   | Long-horizon coding, harder review passes, multi-file implementation, Codex runs where completion is the goal.                                                   |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Terra | First-pass implementation, review triage, scoped fixes with escalation available.                                                                                |
| Luna  | Low-reasoning work such as quick summaries, simple code explanations, PR summaries, lightweight review prechecks, test-name generation, and changelog scaffolds. |

Luna is the tier to watch for top-of-funnel agent work. Use it where a cheap first pass has value on its own or can reduce the load before a Terra or Sol escalation. The key is low reasoning. If the task is mostly summarizing, labeling, extracting, or drafting a scaffold, Luna is the place to start.

## **The clearest signal was follow-through**

One task made the pattern easy to see. The agent had to add stepped slicing to a programming language. That meant parser support, runtime behavior for arrays and strings, assignment behavior, Unicode correctness, and exact error messages.

A weaker agent can make that look done. It edits the parser, runs one happy-path test, and stops. Sol did the useful version of the job. It inspected the parser and evaluator, added focused tests, handled edge cases like zero-length assignments and Unicode rune counts, ran the core suites, noticed an unrelated `go vet` warning, and submitted after verification.

The process carried the signal. Sol did the dull work around the feature.

We saw the same pattern in the live puzzle testing from the transcript. On the three-student word puzzle, Sol split the candidate words into letters and reasoned through each student's knowledge state. Terra reached the answer too, but its path looked more heuristic. The answer matched. The confidence did not.

For coding agents, that gap changes how much you can trust the run. A model that guesses its way through a puzzle will guess inside your repo too. A model that builds a method and checks it has a better chance when the next task has no obvious path.

Sol is best pictured as a persistent engineer: plain-spoken and stubborn about the list. I would still bring in another model for open-ended architecture. I would pick Sol when I need the boxes checked.

We noted the same split in task terms. Fable tends to be better for architectural discussion, UI flow, and high-level judgment. Sol was stronger on lists, long-running implementation, existing code patterns, computer-use workflows, subagent coordination, and multi-day Codex runs. That is the useful distinction for teams. Use Fable when you want the smartest discussion. Use Sol when the queue needs to move.

## **Coding runs: Sol follows through**

We also looked at a long-horizon coding run with more than 100 tasks across TypeScript, Go, Python, JavaScript, and Rust. Each task asks the agent to inspect a repository, change the code, and pass behavioral checks. The score is useful because it tracks completed software changes instead of answer style.

![Bar chart shows Sol model achieved 63.7% coding task pass rate, Terra 48.7%.](https://www.coderabbit.ai/content/assets/gpt-5-6-sol-and-terra-benchmark-media-03.png)

| Sol   | 63.7% | 100% | 20,968 |
| ----- | ----- | ---- | ------ |
| Terra | 40.7% | 100% | 55,594 |

Sol passed 63.7% of tasks with no trial errors. That matches the hands-on experience: it stays oriented, keeps checking requirements, and works through the unglamorous parts of a repo task.

Terra passed 40.7% of tasks. It also used more output tokens. Terra costs half as much per output token as Sol, while this run averaged 55,594 output tokens per completed task versus 20,968 for Sol.

![Bar chart compares output tokens and cost for Sol and Tera AI coding models.](https://www.coderabbit.ai/content/assets/gpt-5-6-sol-and-terra-benchmark-media-04.png)

Terra may be cheaper for bounded work. For long coding jobs, measure cost per solved task before routing large volumes to it.

## **What this means for agent loops**

The coding results point to a specific routing pattern. Sol should not be treated as a replacement for every model in the SDLC. It is strongest once the work has enough shape for an agent to execute, test, and improve.

If you split code generation into an agent loop, I would route the work this way:

| Planning  | Fable 5 for architecture, Sol for execution plans                      | Fable is better for open-ended tradeoffs. Sol is better when the plan needs to become a checklist the agent will actually follow.         |
| --------- | ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Research  | Sol                                                                    | It is strong at reading existing code patterns, tracing dependencies, and staying oriented across a large repo.                           |
| Execution | Sol                                                                    | This is the clearest fit. Sol is persistent, follows long task lists, and keeps working through the boring parts of implementation.       |
| Testing   | Sol                                                                    | It is better suited to running suites, interpreting failures, adding targeted tests, and looping until the patch holds.                   |
| Improving | Sol for recall, Sonnet 5 for comment quality, Terra for cheaper triage | Sol finds more issues. Sonnet 5 is useful when developer attention is the constraint. Terra can screen lower-risk work before escalation. |

The main change is where you can trust the agent to keep going. Planning still benefits from a second model when the problem is ambiguous. Once the plan is clear, Sol is the model I would put in the loop for implementation, test repair, and follow-up fixes.

## **Code review: Sol finds more, then filtering has to earn trust**

Code review has a different bar. A coding agent can be verbose if it lands the patch. A reviewer has to catch the right issue and explain it in a way a developer will act on.

We ran the new models through CodeRabbit's review benchmarks, using production-style pull requests with known issues. The goal was to see what happens when Sol or Terra is added in the review system.

The baseline is our current ensemble. The delta shows the change when the model is added to that ensemble. A positive delta means the model found issues the current ensemble was more likely to miss. A negative delta means the model did not add enough useful signal in that run.

The table uses six metrics:

- `Pass` counts expected issues found in a way a developer could act on.
- `Delta vs baseline` is the lift or drop after adding the model lane to the current ensemble.
- `Pass full` gives broader credit for valid findings that may still need tighter wording or product filtering.
- `Precision` is the share of actionable comments that were correct enough to keep.
- `Comments` is raw model comment volume before product filtering.
- `Nitpicks` are low-priority comments that can create review noise.

Sol moved in the right direction for recall. In our internal review discussion, it found roughly eight more bugs than the baseline. The dashboard data shows Sol at 69 of 99 actionable passes, or 69.7%, with a +7.4 point lift after adding the model lane to the ensemble.

![Code review performance charts comparing pass rates and actionable precision for Sol and Terra models.](https://www.coderabbit.ai/content/assets/gpt-5-6-sol-and-terra-benchmark-media-02.png)

| Sol   | 69 of 99, 69.7%  | +7.4pp | 74 of 99, 74.7%  | 31.6% | 231 | 61  |
| ----- | ---------------- | ------ | ---------------- | ----- | --- | --- |
| Terra | 53 of 101, 52.5% | -8.6pp | 58 of 101, 57.4% | 35.7% | 143 | 21  |

Sol is the stronger candidate for the main review pass. It is more critical and more complete, and it is less likely to wave a risky diff through. The cost is precision. Its actionable precision was 31.6%, down 8.2 points versus the baseline in that report, and it produced 231 raw comments.

That tradeoff is workable in CodeRabbit because filtering is easier than recovering a missed bug. If the model sees more valid issues, the product can suppress weak comments, tune presentation, and route only the useful findings to developers. A model that never spots the issue gives you less to work with.

Terra is the quieter lane. It produced 143 comments and 21 nitpicks, much lower than Sol. Its pass rate was also lower at 52.5%, with an 8.6 point drop versus the baseline average in that report. Its best category was Logic Error at 20 of 33, or 60.6%. It also produced 29 critical comments, 64 major comments, 50 minor comments, and only 9 outside-diff comments.

![Stacked bar chart showing review comment severity for Sai and Terra, dominated by major findings.](https://www.coderabbit.ai/content/assets/gpt-5-6-sol-and-terra-benchmark-media.png)

I would test Terra for triage and cheaper review surfaces before I trusted it with the final pass on high-risk changes. The data points to restraint rather than strength.

## **How Sol compares to Fable 5 and Sonnet 5**

The comparison needs one caveat: these numbers come from different harnesses. Our coding-run results for Sol and Terra are new in this review. The public Fable 5 and Sonnet 5 posts used code-review and coding-task evidence with different setups and denominators.

Fable 5 was the autonomy model. In our Fable 5 review, it stayed close to the baseline on coverage, with 65 of 105 actionable review tasks versus 66 of 105 for the baseline and Opus 4.8. It reached 74 of 105 full review-task passes, slightly ahead of the baseline at 72 of 105. The concern was precision and volume: 32.8% actionable precision, 19.4% full precision, and 253 comments.

That made Fable 5 compelling for autonomous coding projects and harder to recommend as the default production reviewer. It could explore, plan, and build. It also ran long and produced enough review noise to make rollout harder.

Sol feels more practical for execution-heavy coding work. Our findings point to the same split we noted above: Fable is better when the task benefits from taste or high-level judgment. Sol is better when you give it a long list and want every item handled.

Sonnet 5 tells a different story. In our Sonnet 5 review, precision rose from about 29% on Sonnet 4.6 to roughly 38% to 40%. The catch was strict bug-catching. The baseline caught about 57%, Sonnet 5 landed around 50% to 51%, and Sonnet 4.6 caught about 63% while creating more noise.

That makes Sonnet 5 a good fit when comment quality and developer attention are the main concern. Sol is more recall-first in our current read. It finds more, comments more, and needs product filtering around the output.

![Bubble chart showing code review model coverage, precision, and user comment volume.](https://www.coderabbit.ai/content/assets/image1-2.png)

The routing guide looks like this:

| Architectural discussion                              | Fable 5             |
| ----------------------------------------------------- | ------------------- |
| Long implementation run                               | Sol                 |
| Review pass where missing the bug is costly           | Sol, with filtering |
| Review pass where comment quality is the main concern | Sonnet 5            |
| Scoped repeatable work at lower unit price            | Terra               |

The frontier models now feel different enough that one default model leaves useful capability on the table.

## **What Sol costs you**

Sol's main weakness is fuzzy judgment. For architectural debate, product tradeoffs, or a plan with several defensible paths, I would still run Fable or Sonnet in parallel. Sol is strongest once the work has a clear shape.

It can also get stuck. In our findings, one simple change took eight turns because Sol fell into an unhelpful path. We saw fewer of those moments than with GPT-5.5, but they still happen. Use stop points, checkpoints, and a second review thread for multi-hour or multi-day runs.

Review trust is the other cost. Sol finds more issues, but developers only see the comments that survive the product. A raw model that posts too much can train people to ignore it. The review product still has to rank, filter, and explain.

Terra has its own cost question. Its list price is lower, but the long coding run used more output tokens than Sol. For high-volume jobs, run your own cost-per-resolution test before moving large traffic. Cheaper tokens do not always mean cheaper solved tasks.

## **Should you switch?**

Switch to Sol for coding agents if you are on GPT-5.5 or a previous OpenAI model. The long coding run is strong, and the hands-on behavior matches the score. Sol stays oriented longer, follows more requirements, and does more of the unglamorous work that makes an agent useful.

Use Terra as a second lane. It is worth testing for scoped coding tasks, first-pass review, and workflows where escalation to Sol is available. Watch the total output bill.

Use Luna as a first-pass lane. Save model-switch decisions for Sol and Terra, and route bigger tasks upward when speed and cost stop being the main constraint.

Keep Fable and Sonnet in your toolkit. Fable remains the stronger pick for architecture and higher-level planning. Sonnet 5 remains attractive when you want cleaner review comments. Sol is the model I would reach for when I need the work finished.

For most engineering teams, the switch path is clear: start with Sol on long-running implementation and harder review jobs, add Terra where the task is bounded, and keep your current best planning model for the parts where judgment needs more finesse.

## Media links

- <https://www.coderabbit.ai/content/assets/gpt5-6.png>
- <https://www.coderabbit.ai/favicon-16x16.png?v=4>
- <https://www.coderabbit.ai/favicon-32x32.png?v=4>
- <https://www.coderabbit.ai/apple-touch-icon.png?v=4>
- <https://storage.googleapis.com/coderabbit_public_assets/website/content/footer-cta-two-clicks.mp4>
- <https://www.coderabbit.ai/content/assets/image3-4.png>
- <https://www.coderabbit.ai/content/assets/gpt-5-6-sol-and-terra-benchmark-media-03.png>
- <https://www.coderabbit.ai/content/assets/gpt-5-6-sol-and-terra-benchmark-media-04.png>
- <https://www.coderabbit.ai/content/assets/gpt-5-6-sol-and-terra-benchmark-media-02.png>
- <https://www.coderabbit.ai/content/assets/gpt-5-6-sol-and-terra-benchmark-media.png>
- <https://www.coderabbit.ai/content/assets/image1-2.png>
- <https://www.coderabbit.ai/images/CR_mark_orange.png>
