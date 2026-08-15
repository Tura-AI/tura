# What We Learned from the MCP Workflow Benchmark

MCP makes tools available to an agent. It does not make the workflow correct.

That difference is easy to miss in a demo. Calling one tool is usually simple. A useful workflow has to discover the available tools, read the right source, create an artifact, pass its identifier to another service, recover from a rejected call, and leave the remote state exactly as requested.

We built a small benchmark to test that whole path.

## What the benchmark does

The suite contains ten stateful workflows. They cover jobs such as turning a Drive brief into an image and Gmail draft, preparing a contract for signature, assembling a customer onboarding package, handling an incident, and producing a product-demo delivery.

The services are deterministic mocks, not live Google, Adobe, Slack, Stripe, or signing accounts. The protocol is still real MCP JSON-RPC over stdio. Each task starts its own server, requires the agent to complete `initialize`, discover schemas through `tools/list`, and perform the work through `tools/call`.

The mock matters for two reasons. It gives every run the same starting state, and it lets us publish the complete tool trace and final service state without exposing a real account. Where an official MCP schema exists, the task follows the fields and behavior used by that published contract. Where it does not, the benchmark labels the tool as a vendor API adapter instead of pretending it is official MCP.

Every run receives five deterministic checks:

1. MCP initialization completed;
2. all task tools were discovered;
3. every required operation succeeded;
4. operations respected the dependency order;
5. an independent verifier confirmed the final state and generated artifacts.

There is no LLM judge.

## The first result

The published pilot used GPT-5.6 SOL at Low reasoning. It ran ten tasks three times with Tura Balanced, Tura Direct, and Codex CLI: 90 runs in total.

| Configuration |    Passed | Requests | Total tokens | Estimated API-equivalent cost |
| ------------- | --------: | -------: | -----------: | ----------------------------: |
| Tura Direct   | **30/30** |      108 |    1,876,781 |                         $4.66 |
| Tura Balanced | **29/30** |      124 |    2,374,741 |                         $5.15 |
| Codex CLI     | **25/30** |      233 |    5,475,298 |                         $6.81 |
| Total         | **84/90** |      465 |    9,726,820 |                        $16.62 |

Direct completed every run. Balanced missed one event-promotion workflow. Codex missed one invoice follow-up, one recruiting package, and all three social-thumbnail approval runs. In each failed run, MCP initialization and tool discovery completed, but the run did not satisfy the required operations, dependency order, and final-state checks together.

The token total is not simply input plus cache input plus output. Cache input is already a subset of input, and reasoning is already a subset of output. The estimated cost applies the published GPT-5.6 SOL API rates to the recorded request-level usage; it is not a claim about a Codex subscription bill.

## One workflow, two execution shapes

The ecommerce ad task makes the difference easier to see. Both the [Codex CLI run](https://turaai.net/benchmark-run?run=workflow-ecommerce-ad-package-codex-cli-r01) and the [Tura Direct run](https://turaai.net/benchmark-run?run=workflow-ecommerce-ad-package-direct-r02) found the same assets, built the same Premiere project and Photoshop thumbnail, exported both artifacts, posted them to Slack, and passed all five checks.

| Recorded metric | Codex CLI | Tura Direct | Direct change |
| --------------- | --------: | ----------: | ------------: |
| Checks passed   |       5/5 |         5/5 |   Same result |
| Model requests  |        11 |           3 |   72.7% fewer |
| Commands        |        10 |          14 |    40.0% more |
| MCP tool calls  |         9 |          11 |    22.2% more |
| Input tokens    |   261,508 |      55,047 |   79.0% fewer |
| Total tokens    |   262,915 |      56,372 |   78.6% fewer |
| Estimated cost  |    $0.344 |      $0.130 |   62.4% lower |
| Duration        |     49.0s |       50.2s |   2.5% longer |

The Codex run spread setup, discovery, and execution across ten working requests, then used an eleventh request for the final answer. In the execution phase it generally waited for one tool result, returned that result to the model, and asked the model to form the next call. That is a natural agent loop, but every loop is another provider request with prompt, tool schemas, and accumulated history attached.

Direct gave names to earlier results such as `project`, `thumbnail_doc`, and `media_import`. Later steps could use values such as `project.project_id` and `thumbnail_doc.document_id` inside the same runtime batch. Calls in one step ran together; a later step waited for its dependencies. The runtime resolved the variables, so the model did not need a new turn just to read an ID and copy it into the next call.

The trace is useful because the first batch was not perfect. It expected the media import to return a `clips` array, while the tool actually returned `video_clip_id` and `audio_clip_id`. The trim and audio calls therefore did not run, and the premature export and Slack calls were rejected by workflow preconditions. On the next model request, Direct reused the completed project, import, and thumbnail work, applied the two returned clip IDs, then finished the export and post. The third request produced the final answer.

This is why the cost result is more interesting than a simple tool-count comparison. Direct issued 14 commands and 11 MCP calls, versus 10 and 9 for Codex, and its output was almost the same size: 1,325 versus 1,407 tokens. It did not save money by doing less work. It saved repeated model context. Input fell from 261,508 to 55,047 tokens, while uncached input fell from 38,020 to 13,831. Prompt caching discounted much of the repeated context, but it did not make those extra requests free, so the 78.6% token reduction became a 62.4% estimated cost reduction.

Fewer model round trips did not make this particular run faster. Direct took 50.2 seconds against 49.0 seconds for Codex, partly because it had to recover from the wrong variable path. This pair supports a token and cost explanation; it is not, by itself, evidence of a latency win.

## What I think this shows

First, MCP connectivity is a weak success criterion. All six failed runs connected and discovered the tool schemas. The failures happened in the workflow.

Second, a short tool call can carry a long dependency chain. An exported image URL may have to appear in a later draft. A created object may provide the identifier required by the next service. The useful unit is not the individual call. It is the verified state transition across calls.

Third, deterministic mocks are useful for debugging agent behavior. A live SaaS test can fail because of authentication, rate limits, changed account data, or a provider outage. Those are real product concerns, but they make a poor first layer for comparing orchestration. The mock layer lets us inspect the agent path before adding live-service uncertainty.

The result does not prove that one Tura feature caused the difference. Tura Direct, Tura Balanced, and Codex CLI are complete configurations with different runtimes, prompts, batching behavior, and stopping policies. Ten authored tasks at one model and one reasoning level are also too small for a universal ranking.

## The audit boundary matters

[Benchmark issue #1](https://github.com/Tura-AI/benchmark/issues/1) raised a fair broader question: a public result is only as strong as the evidence needed to check it.

The MCP workflow pilot is unusually inspectable because the task scenarios, mock server, verifier, normalized provider calls, MCP trace, final state, workspace, and result contract are published together. That still does not make it a live-service conformance test, and the current scoring contract does not claim to detect every unrelated workspace change.

The older DeepSWE and rewrite evidence has additional boundaries. The July DeepSWE reports name the upstream grader with the `v1.1` tag rather than a verifier commit and image digest, and the upstream verifier fixtures are not fully vendored here. The rewrite tasks do not yet ship one known-good target build through every harness. Those facts do not change the recorded scores, but they change how strongly we should describe reproduction.

There is also an obvious conflict to disclose: Tura-AI develops Tura, owns this benchmark repository, defines the Tura configurations, and reports comparisons against Codex. Public artifacts make the work auditable; they do not make it independent.

I would rather keep those sentences next to the result than hide them in an issue.

## What comes next

The next useful runs are not ninety more copies of the same matrix. We need more models and reasoning levels, a live-service validation layer, explicit off-task workspace guards, and frozen verifier identities. We also need independent reproduction and tasks written outside the Tura project.

The current pilot answers a narrower question: can an agent complete a multi-service MCP workflow, in the right order, against deterministic state, with a final result that code can verify?

On this batch, the answer was yes in 84 of 90 runs. The interesting part is the six times that tool access was not enough.

## The formal documents

- [MCP benchmark tasks and workflow harness](https://github.com/Tura-AI/benchmark/blob/main/doc/mcp-benchmarks.md) — task contracts, adapters, execution, artifacts, and validation.
- [Published MCP workflow manifest](https://github.com/Tura-AI/benchmark/blob/main/results/mcp/report-mcp-workflow-gpt56-sol-low-20260809/manifest.json) — all 90 run records and aggregates.
- [Current benchmark evidence record](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md) — the bounded results and provenance.
- [Benchmark methodology](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md) — selection, scoring, normalization, reporting, and audit limitations.
