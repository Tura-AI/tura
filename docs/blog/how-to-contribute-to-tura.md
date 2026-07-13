# How to Contribute to Tura

*Written July 13, 2026.*

Let me start with the reassuring part: you do not need to understand all of Tura before contributing to it.

Honestly, trying to understand the whole repository first is a good way to spend an evening reading architecture notes and changing nothing. Tura has a runtime, router, session database, provider layer, gateway, TUI, GUI, desktop shell, tools, and enough process boundaries to make "I'll just fix this quickly" a dangerous opening line.

The better route is smaller: pick one observable problem, find the part that owns it, and prove that your change fixes that problem without asking the pull request to prove the entire universe.

That is the contribution model in one paragraph.

## First, decide what kind of change you are making

Before touching code, search the existing [issues](https://github.com/Tura-AI/tura/issues) and [pull requests](https://github.com/Tura-AI/tura/pulls). You may find the same bug, a design discussion, or a half-finished fix that changes what "small" means.

Then give the change one primary type:

| If you are... | Bring this kind of evidence |
| --- | --- |
| Fixing a bug | A reproduction, the root cause, and a regression test at the smallest layer that owns the failure |
| Adding or changing behavior | The user problem, observable acceptance criteria, and any compatibility impact |
| Claiming a performance improvement | A controlled before/after comparison, correctness results, and sanitized raw measurements |
| Adding provider compatibility | Protocol fixtures and narrowly scoped live evidence where fixtures cannot prove the behavior |
| Improving documentation | An accurate source, working links, and a readable rendered result |

Large features, state changes, migrations, compatibility breaks, new provider families, and performance claims need an issue first. A small documentation fix usually does not. Security problems are the exception: report those privately through [SECURITY.md](https://github.com/Tura-AI/tura/blob/main/.github/SECURITY.md), not in a public issue.

The exact decision table and ready-to-use PR templates are in the complete [CONTRIBUTING.md](https://github.com/Tura-AI/tura/blob/main/.github/CONTRIBUTING.md#choose-the-contribution-type).

## Set up a contributor environment, not a surprise PATH migration

Clone your fork, enter the repository, and run the dependency-only setup first:

```powershell
# Windows PowerShell
.\scripts\install.ps1 -EnvironmentOnly
```

```bash
# macOS or Linux
./scripts/install.sh --environment-only
```

That prepares the dependencies without registering a new `tura` command on your user `PATH`. The default installer is meant for end users: it performs a release build and registers Tura. Useful when you want it, mildly surprising when you only meant to run tests.

The complete list of installed files, side effects, and cleanup commands is in [docs/start/install.md](https://github.com/Tura-AI/tura/blob/main/docs/start/install.md).

## Find the owner before finding the fix

Tura tries to keep one owner for each important responsibility. Runtime decides what to do with model output. Provider performs and normalizes model calls. Tools own model-visible commands and execution policy. The router dispatches workers. The session database owns durable session state. Frontends do not get to invent parallel versions of those things.

That means a fix starts with a boundary question:

- Is this parsing or state-transition logic?
- Is it process, socket, PATH, or shutdown behavior?
- Is it provider protocol normalization?
- Is it a TUI or GUI interaction problem?
- Is it persistent session state or recovery?

Read the relevant section of [ARCHITECTURE.md](https://github.com/Tura-AI/tura/blob/main/ARCHITECTURE.md) before changing a system boundary. Most major directories also have their own `ARCHITECTURE.md` with the local contract.

This is not paperwork for its own sake. A five-line fix in the wrong owner often creates a second parser, state model, or retry policy. It works until the two versions disagree, usually at an inconvenient hour.

## Make the smallest change that proves the requirement

"Small" does not mean timid. It means the diff has one job.

If a parser accepts the wrong shape, reproduce that shape and fix the parser. If an installer damages PATH resolution, test the installer behavior at the OS layer. If a button loses state after a restart, the fix probably crosses a UI and persistence boundary, so the evidence should cross it too.

Try not to add an abstraction because a future feature might need it. Tura follows YAGNI: a demonstrated requirement gets code; a hypothetical requirement gets to wait outside.

The same rule applies to cleanup. Please do not mix a bug fix, nearby renaming, formatting, and a small redesign into one pull request. Each extra idea makes the real behavior harder to review.

## Run the test that owns the behavior

This part is easier than it may look. You are not expected to run every provider on every operating system through every frontend for every change.

Start at the smallest layer that can catch the actual regression:

- pure logic, parsing, schemas, or state transitions: the owning unit or crate test;
- a deterministic workflow across modules: the owning business test;
- processes, sockets, shell behavior, PATH, permissions, or shutdown: an OS test;
- a visible TUI or GUI interaction: the app's unit test and the affected end-to-end flow;
- a claim about latency, memory, tokens, or throughput: the owning performance runner plus a correctness check;
- provider behavior that fixtures cannot reproduce: an explicit opt-in live test.

Common quality and test commands are listed in [CONTRIBUTING.md](https://github.com/Tura-AI/tura/blob/main/.github/CONTRIBUTING.md#choosing-tests). The full ownership table, affected matrix, and acceptable substitutes when stable automation is not possible are in [docs/contributing-guide.md](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md#test-ownership).

There is one useful sentence to remember:

> A pull request does not need to prove everything. It needs to prove the behavior it owns.

If the change crosses a boundary, add the boundary check. If it does not, resist the urge to make CI reenact the whole product.

## If you say "faster," bring receipts

Performance work has a slightly higher bar because performance claims are very easy to make accidentally.

An internal timer going down is useful diagnostic evidence. It does not automatically mean a user-visible operation got faster. A serious comparison names the baseline and candidate commits, command, workload, hardware, operating system, settings, warm-up policy, sample count, failures, correctness, and the metric that decides pass or fail. Latency claims should include distribution information such as p50 and p95, not one especially photogenic run.

Do not silently delete outliers. Do not trade correctness for a faster median. And if the real improvement is lower complexity, bounded memory, or a better worst case, say that. It is a perfectly good result without dressing it up as a universal speedup.

The complete evidence format is in [docs/contributing-guide.md](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md#performance-and-efficiency-evidence).

## Test reports are contributions too

Tura needs code, but it also needs more independent evidence. The published benchmark does not yet cover enough reasoning levels, model providers, or agent architectures, and it does not isolate every Tura feature with a clean ablation. The GUI, TUI, packaged desktop app, and OS-specific lifecycle paths also have plenty of room for failures that one development machine will never reveal.

A useful contribution can therefore be a reproducible test report: the exact Tura commit, model and provider version, reasoning setting, operating system, hardware, command, workload, expected behavior, observed result, and sanitized artifacts. A failed run is not wasted work. If someone else can reproduce it, it has already narrowed the problem.

The longer version—including the comparisons, ablations, frontend cases, and cross-OS reports we need—is in [We Need More Benchmark Data and Test Reports](https://github.com/Tura-AI/tura/blob/main/docs/blog/we-need-more-benchmark-data-and-test-reports.md). Open gaps are tracked in [KNOWN_ISSUES.md](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md).

## Open a pull request that tells the truth

Create a focused branch and commit only related files:

```bash
git switch -c fix/short-description
git add <related-paths>
git commit -m "Fix short description"
git push -u origin fix/short-description
```

Then open the matching PR template. In the description, explain:

- the user-visible problem or requirement;
- the root cause, when it is a bug;
- what changed and why this layer owns the change;
- the exact checks you ran and their summarized results;
- affected checks you did not run, with the reason;
- compatibility, migration, or rollback concerns, when they exist.

"Not run" is useful information. Hiding it is not.

Before pushing, check the diff for credentials, authorization headers, private prompts, session data, personal paths, provider logs, and generated local state. Raw evidence should be detailed enough to recompute a result, not raw enough to leak someone's account.

You also need the right to submit every piece of code, data, prompt, fixture, or generated material in the change. Tura is licensed under AGPL-3.0-or-later, and contributions may be distributed under the repository's license.

AI or other tools can help. A human still has to be the primary submitter and take responsibility for correctness, licensing, provenance, verification, and what the PR says. "The model wrote it" is an explanation of process, not a transfer of responsibility.

## A good first contribution is usually boring

That is a compliment.

A documentation link that points to the right place. A reproducible parser edge case with one durable test. A clearer error message. A cleanup that removes duplicated work and proves behavior stayed the same. These changes are easy to understand, easy to review, and surprisingly useful.

You do not need to arrive with a new agent architecture. Start with one thing you can observe, fix, and verify. Once that is merged, the repository will make much more sense than it did from the doorway.

If a full issue or test report feels too formal, share what you found on any social platform and mention `@tura-ai-agent` or use `#tura-ai-agent`. I watch both, and the feedback helps me find the failures, environments, and workflows I would otherwise miss.

The goal is ambitious: I want to make Tura the strongest-performing open-source coding agent.

## The formal documents

This post is the friendly route through the process. These complete Markdown files are the source of truth:

- [.github/CONTRIBUTING.md](https://github.com/Tura-AI/tura/blob/main/.github/CONTRIBUTING.md) — contribution types, setup, common tests, PR steps, licensing, and authorship.
- [docs/contributing-guide.md](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md) — test ownership, affected matrices, performance evidence, and artifact sanitization.
- [ARCHITECTURE.md](https://github.com/Tura-AI/tura/blob/main/ARCHITECTURE.md) — repository boundaries and implementation ownership.
- [tests/README.md](https://github.com/Tura-AI/tura/blob/main/tests/README.md) — the complete testing reference.
- [docs/start/install.md](https://github.com/Tura-AI/tura/blob/main/docs/start/install.md) — setup behavior, installed files, PATH effects, and cleanup.
- [docs/KNOWN_ISSUES.md](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md) — current benchmark, frontend, runtime, and cross-OS evidence gaps.
- [.github/CODE_OF_CONDUCT.md](https://github.com/Tura-AI/tura/blob/main/.github/CODE_OF_CONDUCT.md) — community expectations.
- [.github/SECURITY.md](https://github.com/Tura-AI/tura/blob/main/.github/SECURITY.md) — private vulnerability reporting.

## Contact

Email: [info@turaai.net](mailto:info@turaai.net)
