# Known Issues and Evidence Gaps

This is the list of places where Tura is not yet proven enough. Some entries are
reproduced failures; others are architectural risks or missing evidence. They do
not occur on every machine. Link a concrete GitHub issue when a failure can be
reproduced, and remove an entry only after its exit criteria and regression
coverage are satisfied. Optimism is useful. It is not a test result.

## Provider benchmark coverage is narrow

**Status:** Open

The provider catalog configures many protocol families, and repository tests
contain OpenAI, Google, Anthropic/Claude, Codex, and compatibility fixtures.
However, the published long-horizon benchmark evidence is concentrated on the
named Codex/OpenAI-style configurations. A configured provider or a mocked
request is not proof of production compatibility or comparative performance.

**Risk:** Provider-specific streaming events, tool-call formats, usage fields,
reasoning metadata, caching, retries, and cancellation can regress unnoticed.

**Required evidence:** A published provider matrix with protocol fixtures and
cost-bounded live smoke tests, followed by repeated long-horizon runs for at
least Anthropic/Claude, Google/Gemini, one OpenAI-compatible third party, and one
local endpoint. Record exact provider/model versions, settings, dates, raw
artifacts, failure taxonomy, and cost.

## Runtime/session parsing is not end-to-end single-source

**Status:** Partially converged

`SessionState` is canonically defined in
`crates/session_log/src/session_state.rs` and re-exported by
`crates/runtime/src/state_machine/session_management.rs`. This removes one class
of enum drift. The wider session contract still crosses runtime snapshots,
context rebuilding, JSON payloads, queue records, IPC, and SQLite helpers. Those
boundaries can repeatedly deserialize, normalize, clone, and serialize session
data during a turn.

**Risk:** Duplicate work increases latency and allocation pressure, while
separate parser/normalization paths can drift on old records, recovery, terminal
states, or compaction.

**Required evidence:** First measure parse count, bytes, wall time, allocations,
queue latency, and snapshot size. Then establish one versioned session/state
contract or generated compatibility layer. Preserve fixtures for legacy/current
records and test transition parity, checkpoint/recovery, compaction, malformed
records, and mixed-version replay. Do not remove a parser merely because two
functions have similar names.

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
