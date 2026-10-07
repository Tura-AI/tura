# From Company APIs to an Agent Runtime You Can Actually Check

A company rarely asks for an agent in those words. Someone in support wants to answer a customer before lunch. Someone in operations wants a paid order shipped once, not twice. Finance wants the refund in the right store and currency. The hard part is getting all of those small jobs to work together without letting the agent guess its way through a payment or a customer message.

We have been using an Atlas Outdoors scenario to work through what Tura should do in that situation. Atlas has six outdoor storefronts, several support channels, payment and shipping systems, and a new site waiting to launch. Its team supplies contracts for its internal and supplier APIs and explains the work in ordinary Markdown. Tura's job is to turn that material into an agent with clear permissions, test it on complete tasks, improve the failures, and package the version the team accepts.

This is a product workflow scenario with isolated service records. The scores and charges below describe that scenario. They are not results from a live Atlas customer or live supplier accounts.

![The Atlas Outdoors workspace showing the guided product workflow](../../assets/data/atlas-agent-runtime-blog/00-dashboard-start.png)

## Start with the systems the company already uses

An OpenAPI file tells us what a supplier can do. It does not tell us which store an employee may touch, who can approve a refund, or whether a second call would create a second shipping label. Atlas supplies nine API contracts and a separate permissions file so those decisions have a place to live.

In the scenario, Tura checks the contracts and registers 22 typed commands. Each command carries its source, store scope, authorization rules, and any required approval. Credentials stay behind vault references. The agent discovers the commands through MCP, but it never needs a raw Stripe key or a general invitation to call every operation in an API.

![Connecting registers the supplier contracts and permissions as scoped commands](../../assets/data/atlas-agent-runtime-blog/01-connecting-complete.png)

Then the team writes down the actual work. One task is a partial refund. Another is a delivery update when a carrier has not supplied an ETA. Others cover a conversation spread across email and messaging apps, fulfillment, a new store launch, daily reconciliation, and product images. A task description includes the result we expect to see in the supplier state, not just the answer we hope the model will write.

The product image task makes the point in a more visible way. Three different products need one consistent look, but each image still has to match its own SKU. Generation is only one step. Inspection, a creative approval, an asset hash, and a catalog receipt belong to the same job.

![Three Atlas product images reviewed as one visual set](../../assets/data/atlas-agent-runtime-blog/11-product-media-gallery.png)

## Run the whole errand before calling it a success

A refund is not finished because the agent says, "I've issued it." The payment has to belong to the order, the amount has to be refundable, an approver has to sign off, and the provider has to return a receipt. If a request times out, the retry must use the same idempotency key. The final check looks for exactly one refund.

The same rule applies to the less dramatic jobs. A delivery message cannot invent an arrival time. A site cannot be called live because a test checkout worked. Stripe live mode and publication are separate approvals, followed by separate state checks. When the evidence is missing, the agent should say what is pending.

Tura builds repeatable benchmark cases from seven workflows and the team's success criteria. The scenario holds the model route and command schemas steady, then runs each task three times with Codex CLI direct MCP, Tura Direct, and Tura Balanced. That makes 63 isolated cases per revision. The checks cover MCP setup, tool discovery, authorized operations, the final business state, and the receipts that support it.

![The Atlas benchmark run showing 63 completed cases](../../assets/data/atlas-agent-runtime-blog/06-baseline-benchmark-complete.png)

One completed answer does not settle the result. We want the run trace, the provider response, the time, the token use, and the cost next to the final state. If an agent wrote a convincing message but sent it to the wrong channel, the case should fail.

## Let a failure change the next version

In the Atlas scenario, the first revision passes 54 of 63 cases. The team reads the failed assertions and traces, updates the workflow and agent instructions, and reviews the generated dispatcher and runtime changes. Tura keeps those inputs together as a new revision and runs the same cohort again. The next revision passes 59 cases, and a third passes 62.

The remaining case matters. It involves a carrier status outside the configured vocabulary, so the workflow leaves it for human review. A score of 62/63 is useful only when the team can see which case remains unresolved and decide whether that is an acceptable boundary.

![Atlas revision history and the accepted benchmark result](../../assets/data/atlas-agent-runtime-blog/07-compare-accepted.png)

This is the feedback loop we want for agent development: make a change, rerun the same work, inspect what improved and what broke, then select a specific revision. The old scores and configurations remain available. If the team restores an earlier revision, it must rebuild the matching runtime instead of quietly continuing with a binary from another version.

Cost is part of that comparison. Tura can group dependent commands so the model does not need a fresh request just to carry an ID from one successful step to the next. That can reduce repeated context, but we still need to measure the complete run. Compare puts verified outcome next to model turns, tokens, tool calls, time, and charge. The number we care about is the cost of a task that actually passed.

![Atlas run comparison showing outcome, time, charge, tokens, turns, and commands](../../assets/data/atlas-agent-runtime-blog/07-r2-compare-runs.png)

## The handoff should carry the evidence with it

The proposed output is a runnable Agent Runtime for the selected operating system and workflow. Its manifest ties the binary to the accepted revision, command permissions, input hashes, model route, and benchmark record. The company gets a checksum and run instructions. It should be possible to answer a simple question after handoff: which version of the agent made this decision?

![Runtime packaging for the accepted Atlas revision](../../assets/data/atlas-agent-runtime-blog/09-runtime-complete.png)

Cost needs the same honesty as success. In the Atlas example, a $200 starting wallet covers a modeled $132.26 of generation and evaluation charges and a separate $50 build charge, leaving $17.74. Those are scenario terms, including a selected provider usage multiplier, not a general quote for every company.

The proposed runtime license is based on verified token cost savings against an agreed baseline for the same task and model. If the baseline costs $0.50 in model tokens and the runtime costs $0.20, the saving is $0.30 and a 22% fee is $0.066. A failed task or one with no saving owes no savings fee. Provider usage and infrastructure still have their own costs. The comparison only means something when the task outcome is verified first.

I like this workflow because it starts with a familiar problem. A company has systems, staff, rules, and customers who need real work done. An agent is useful when it can operate inside those rules, show what happened, and get better after a failed test. The runtime is the last step in that process, carrying forward the version the team actually checked.

The existing [Tura benchmark methodology](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md) explains how published benchmark claims are scoped and audited. The Atlas figures here remain illustrative until they are backed by a separate public run package.
