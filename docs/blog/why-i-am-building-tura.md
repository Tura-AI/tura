# Why I'm Building Tura

*Written July 13, 2026.*

I did not start Tura because the world needed another chat box with a terminal attached. We already have plenty of those.

The name comes from the Sanskrit word *tura* (तुर), which can carry senses such as quick, swift, or prompt, as well as strong, powerful, or excelling. I liked the combination: an agent should move quickly, but speed is not worth much unless the work is strong. The dictionary entries are collected [here](https://kosha.sanskrit.today/word/sa/tura).

I started it because coding agents were getting much better at writing code, while the part around the model still felt strangely clumsy. A normal session could look like this:

1. inspect a file;
2. wait for the model;
3. inspect another file;
4. wait again;
5. make an edit;
6. wait;
7. run a test that we already knew we needed.

The model might be smart, but the workflow around it was making the same short trip over and over. It was slow, it repeated context, and every extra round cost tokens. More importantly, it made long jobs feel fragile. One noisy tool result or one context reset could turn a careful debugging session into: "Right, where were we?"

That is the problem I wanted Tura to work on.

## The agent should do a chunk of work, not narrate every keystroke

Tura has one model-facing execution tool called `command_run`. The name is not especially glamorous. That is probably healthy.

The useful part is that it can group related work into explicit steps. Independent reads can happen together. An edit can wait for those reads. A build and the relevant tests can run after the edit. The model does not need a fresh conversation turn for every tiny action.

This is not the same as handing the agent one enormous shell script and hoping for the best. Commands stay structured. Dependencies are visible. Mutating operations act as barriers. File locks and permission checks still apply. The point is to remove conversational ceremony, not engineering discipline.

The full design is in [Why Tura Uses One Tool](https://github.com/Tura-AI/tura/blob/main/docs/core/command-run.md#why-tura-uses-one-tool), and the complete command document is [here](https://github.com/Tura-AI/tura/blob/main/docs/core/command-run.md).

## Saving tokens is useful. Spending them well is the actual point

Reducing model round trips naturally reduces repeated context and token use. That matters because tokens cost money, but I do not think "uses fewer tokens" is enough of a product idea on its own.

The more interesting question is what to do with the budget you save.

Tura currently has two answers. Direct tries to keep the workflow lean. Balanced spends more of the saved budget on investigation, reasoning, and verification. In the published DeepSWE comparison, Direct used 77.5% fewer aggregate tokens than Codex CLI while reaching a comparable verifier success rate; Balanced used 31.1% fewer tokens and reached a higher success rate in that test set.

Those numbers are evidence for named configurations on a bounded set of tasks. They are not a law of nature, and they do not prove that every provider, model, operating system, or repository behaves the same way. The project [README](https://github.com/Tura-AI/tura/blob/main/README.md) says that plainly, and the open evidence gaps live in [KNOWN_ISSUES.md](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md).

I would rather publish a limited claim that can be checked than a sweeping one that cannot.

And the current claim is still much narrower than the question I actually care about. We need comparisons across reasoning levels, model providers, and genuinely different agent architectures. We also need ablations that isolate Tura's own features instead of crediting the whole system for every result. I wrote the missing work down in [We Need More Benchmark Data and Test Reports](https://github.com/Tura-AI/tura/blob/main/docs/blog/we-need-more-benchmark-data-and-test-reports.md).

## Context should not become a junk drawer

A lot of agent systems treat context like luggage packed five minutes before a flight. Add a skill. Add another prompt. Add a pile of instructions "just in case." Eventually the model spends part of every turn sorting through advice that has nothing to do with the current job.

Tura takes a narrower approach. The runtime keeps explicit task state and loads the operation manual and capabilities that belong to the work being done. A frontend task and a deployment task should not carry the same instructions merely because both happen in a repository.

When a long session needs compaction, Tura stores a checkpoint of the active work instead of trusting a loose summary to reconstruct everything later. The goal is simple: after context is reduced, the agent should continue the job, not conduct an archaeological dig through its own transcript.

The formal version, including what is retained and how context is rebuilt, is in the complete [Context Management](https://github.com/Tura-AI/tura/blob/main/docs/core/context-management.md) document.

## A useful agent has to survive Tuesday afternoon

Short demos are forgiving. Real work is not.

Real work gets interrupted. A process exits. The laptop restarts. A task spans several sessions. You open the same project from the terminal in the morning and the GUI later. If the agent's memory only exists inside one live chat window, it is not really managing work. It is improvising near a text box.

That is why Tura treats sessions, messages, task state, todos, and workspace history as durable data. The session database has one owner, while the CLI, TUI, GUI, and desktop shell are different fronts over the same backend path. I do not want four slightly different agents depending on which window happens to be open.

This choice adds less visible work: recovery rules, process ownership, compatibility, state transitions, and tests for interruption. It is also the work that makes the visible features trustworthy. The current boundaries are documented in full in [ARCHITECTURE.md](https://github.com/Tura-AI/tura/blob/main/ARCHITECTURE.md).

## The harness has to be open too

Tura is local and open source, but for an agent that is only part of the story. The harness matters as much as the model: prompts, tool contracts, runner behavior, scoring rules, and failure classification can all change the result.

If a public performance claim depends on hidden harness logic, nobody can really inspect the claim. They can only repeat it.

So Tura's rule is that project-controlled logic needed to reproduce a public claim should be inspectable. Commercial model providers may remain external; that is reality. But the part we control should not disappear behind a benchmark screenshot.

This is also why contributions that claim something is faster or cheaper need the raw, sanitized evidence behind the sentence. It is slightly more work. It saves everyone much more work later.

## Tura is not mature, and I do not want to pretend otherwise

Tura is not a mature project. The tempting thing in an agent project is to hide that fact by adding surface area. There is always another provider, panel, mode, or clever abstraction nearby.

The current 0.1.x work is intentionally less exciting: installation, session persistence, recovery, process cleanup, provider evidence, cross-OS behavior, and repeatable performance baselines. The rule is YAGNI: do not add speculative machinery before there is a demonstrated requirement for it.

That is not a lack of ambition. It is the order of operations. Planning and task-workspace features are much more useful when the state underneath them does not wobble.

The honest list of what comes next, what still lacks evidence, and what counts as done is in the complete [ROADMAP.md](https://github.com/Tura-AI/tura/blob/main/ROADMAP.md).

I still believe Tura will change how later coding agents are architected. Not because one benchmark settles the question—it does not—but because macro execution, explicit task state, selective runtime instructions, durable sessions, and shared backend ownership address problems that keep reappearing across agent products. That is my conviction about the direction. The project still has to earn the evidence.

## So, why Tura?

Because I want a coding agent that feels less like supervising a very fast autocomplete and more like working with a careful engineer.

It should read before editing. It should group work that belongs together. It should remember the task after context changes. It should verify claims at the layer that owns them. It should admit what it did not test. And when it says it is better, the evidence should be close enough for someone else to check.

That is a slower sentence than "AI that writes your app for you."

It is also the product I actually want to use.

The goal is ambitious: I want to make Tura the strongest-performing open-source coding agent.

## The formal documents

This post is the conversational version. These complete Markdown files are the source of truth:

- [README.md](https://github.com/Tura-AI/tura/blob/main/README.md) — what Tura does and the bounded benchmark results.
- [ARCHITECTURE.md](https://github.com/Tura-AI/tura/blob/main/ARCHITECTURE.md) — process, runtime, session, provider, and tool ownership.
- [ROADMAP.md](https://github.com/Tura-AI/tura/blob/main/ROADMAP.md) — current priorities, evidence requirements, and exit criteria.
- [docs/core/command-run.md](https://github.com/Tura-AI/tura/blob/main/docs/core/command-run.md) — the macro command model and its safety boundaries.
- [docs/core/context-management.md](https://github.com/Tura-AI/tura/blob/main/docs/core/context-management.md) — context rebuilding, checkpoints, and compaction.
- [docs/KNOWN_ISSUES.md](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md) — known limitations and evidence gaps.
