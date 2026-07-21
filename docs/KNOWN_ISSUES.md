# Known Issues and Evidence Gaps

This is the list of places where Tura is not yet proven enough. Some entries are
reproduced failures; others are architectural risks or missing evidence. They do
not occur on every machine. Link a concrete GitHub issue when a failure can be
reproduced, and remove an entry only after its exit criteria and regression
coverage are satisfied. Optimism is useful. It is not a test result.

## Provider and reasoning-level benchmark coverage is narrow

**Status:** Partially covered

The provider catalog configures many protocol families, and repository tests
contain OpenAI, Google, Anthropic/Claude, Codex, and compatibility fixtures.
However, the published long-horizon benchmark evidence is concentrated on the
named GPT-5.6 SOL and Codex/OpenAI-style configurations. The current evidence
[record](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md)
now includes matched Tura and official Codex CLI runs at High reasoning effort
on all 20 DeepSWE and 5 rewrite tasks. It does not yet provide the same crossed
agent-by-reasoning-level matrix at Medium and High, or comparable long-horizon
evidence for other provider families. A configured provider or a mocked request
is not proof of production compatibility or comparative performance.

**Risk:** Provider-specific streaming events, tool-call formats, usage fields,
reasoning metadata, caching, retries, and cancellation can regress unnoticed.

**Required evidence:** Extend the matched Tura and Codex CLI comparison into a
crossed reasoning-level matrix, including Medium and High, while holding the
model version, official CLI/runtime builds, provider, task revisions, timeout,
evaluator, and run count fixed. Publish a provider matrix with protocol
fixtures and cost-bounded live smoke tests, followed by repeated long-horizon
runs for at least Anthropic/Claude, Google/Gemini, one OpenAI-compatible third
party, and one local endpoint. Record exact provider/model versions, settings,
dates, raw artifacts, failure taxonomy, and cost.

## Runtime/session context projection cost is not fully measured

**Status:** Lifecycle ownership converged; performance evidence incomplete

`SessionState`, `SessionAggregate`, and `SessionManagement` are canonically
defined in `crates/lifecycle`. Runtime, Gateway, Router, and Session DB consume
typed lifecycle contracts directly; noncanonical DB schemas and queued payloads
are rejected instead of entering compatibility parsers. Context rebuilding,
record projections, IPC, and SQLite still cross JSON boundaries that can repeat
deserialization, cloning, and serialization during a turn.

**Risk:** Duplicate work increases latency and allocation pressure, while
separate parser/normalization paths can drift on old records, recovery, terminal
states, or compaction.

**Required evidence:** Measure parse count, bytes, wall time, allocations, queue
latency, and retained context size at the typed create/command/delta boundaries.
Preserve transition, checkpoint/recovery, compaction, malformed-record, and
noncanonical-schema rejection fixtures. Do not add compatibility parsers or
remove a current parser merely because two functions have similar names.

## Cross-crate communication contracts are bundled with implementations

**Status:** Open architecture issue

Communication contracts are not isolated at either the frontend or internal
Rust boundaries. Frontend-facing request, response, and event types live under
`crates/gateway/src/contracts`. Router IPC request/response types live in the
router crate; session DB commands, responses, and snapshots live in the
`session_log` crate; and runtime status and session types live in the runtime
crate. A caller such as gateway cannot express a dependency on only those wire
types. It must depend on the implementation crates even where a boundary needs
only their communication contracts.

Cargo therefore places the full implementation crates in the build graph; it
cannot compile only their protocol modules. Runtime brings provider, agent,
persona, router, tool, and session-log dependencies. Router brings agent,
persona, session-log, tool, process, and OS-specific dependencies. Session log
also bundles its SQLite storage and service implementation with its protocol.

**Risk:** Contract reuse pulls unrelated implementation dependencies into Cargo
dependency, compile, link, and rebuild surfaces. It increases build cost, makes
crate ownership and release boundaries less clear, and can encourage dependency
cycles or mirrored types. Mirrored definitions also make serialization behavior
and protocol versions easier to drift between processes and clients.

**Required work:** Split transport-neutral request, response, event, status,
snapshot, and shared serialization types into lightweight contract crates owned
by each protocol boundary, including gateway, router, runtime, and session DB.
Do not replace the current coupling with one monolithic catch-all contract
crate. Implementation crates should depend on their contract crates; callers
that only exchange protocol messages should not depend on the implementations.
A contract crate must not depend on gateway, runtime execution, provider, router
execution, tools, process management, HTTP servers, desktop code, or storage
implementations. Preserve current JSON fixtures, serde behavior, protocol
versioning, and wire compatibility. Add dependency-direction and cycle checks,
then record dependency counts plus clean and incremental build times before and
after the split.

## Runtime timing coverage is incomplete

**Status:** Instrumentation available; attribution incomplete

Runtime timing events are emitted by `crates/runtime/src/profile_timings.rs` when
`TURA_PROFILE_TURN_TIMINGS` or `TURA_PROFILE_TIMINGS` is enabled. Optional JSON
payload-size measurement uses `TURA_PROFILE_TURN_TIMING_BYTES` or
`TURA_PROFILE_TIMING_BYTES`. Existing call sites cover context construction,
prompt preparation, request options, checkpoints, session-log calls, and gateway
event assembly, but not every expensive boundary is attributable yet.

**Risk:** Optimization work may target visible symptoms instead of the dominant
cost, or move cost between stages without improving end-to-end latency.

**Required evidence:** Add stable structured labels around uncovered boundaries,
correlate them by session/runtime ID, and compare end-to-end plus stage-level
distributions. Profiling must be disabled by default and its own overhead must
be measured.

## TUI performance needs broader baselines

**Status:** Open evidence gap

Existing tests cover heavy history and full-chain stress, but there is no
published cross-OS budget covering cold startup, first render, keystroke latency,
stream update rate, resize, session switching, memory growth, and long-running
terminal behavior across representative terminals.

**Required evidence:** Repeatable workloads with p50/p95/p99 latency, CPU,
resident memory, event-loop delay, and output correctness on Linux, macOS, and
Windows. Include narrow/wide terminals, Unicode, large tool output, concurrent
sessions, and resumed histories.

## GUI performance needs broader baselines

**Status:** Open evidence gap

The GUI has transcript virtualization, rich-text history, plan, and full-chain
performance tests. Coverage still needs stable budgets for application startup,
session switching, streamed rich text, large transcripts, plan scale, calendar
navigation, memory retention, and desktop-webview differences.

**Required evidence:** Repeatable browser and packaged-desktop measurements with
render latency, long-task/event-loop delay, memory, DOM size, dropped updates,
and screenshot/behavior correctness. Test Linux, macOS, and Windows where the
desktop stack differs.

## Benchmark suite does not cover all operational costs

**Status:** Open

Published agent benchmarks measure important long-horizon task outcomes, turns,
and token usage. They do not by themselves cover local startup, runtime/session
parsing, memory, disk growth, UI responsiveness, process recovery, or installer
robustness.

**Missing categories:** Cold/warm startup; idle and peak memory; CPU and disk
I/O; session DB growth and migration; parse/serialization cost; concurrent
sessions; cancellation and process death; queue recovery; TUI/GUI latency;
install/update/unregister; PATH behavior; and bounded soak tests.

**Required evidence:** Maintain a coverage matrix mapping each claim to a test,
metric, threshold, OS, artifact, and owner. Passing one aggregate benchmark must
not be used as a proxy for uncovered behavior.

## Cross-OS robustness needs continued testing

**Status:** Active

The repository has OS and installation suites, but OS-specific behavior remains
a risk around process trees, signals, sockets, file locking, shell profiles,
permissions, path separators, spaces and Unicode in paths, antivirus delays, and
desktop dependencies.

The source-install workflow is expected to run the default complete installer
and verify `tura --help` from a fresh shell on Ubuntu, macOS, Windows Server 2022,
and Windows Server 2025. Broader lifecycle and soak coverage remains necessary.

**Required evidence:** Keep installation/PATH checks in the required CI path;
run process and lifecycle suites serially where they own global resources; save
failure logs as artifacts; and add a regression case for every OS-specific bug.

## Planning UI remains a 0.2 workstream

**Status:** Planned

Plan-related GUI code and tests exist, but the complete Plan, Calendar, and
Session Tasks experience is not yet the stable 0.1.x contract. New work must
avoid creating separate state models for those views.

See the [Roadmap](../ROADMAP.md) for 0.1.x and 0.2 priorities.
