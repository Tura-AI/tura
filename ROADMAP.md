# Tura Roadmap

This roadmap describes priorities and exit criteria. It is not a promise of a
specific release date. Security, data-loss, and release-blocking regressions may
change the order.

## Engineering rule

Tura follows YAGNI (You Aren't Gonna Need It): do not add speculative code,
state, compatibility layers, or abstractions before a demonstrated requirement
exists. A performance or efficiency change that cannot demonstrate an
improvement with a relevant benchmark or evaluation should not exist. Every bug
fix must include a regression test and must run the test flow that
previously allowed the bug to escape.

Benchmark evidence must identify the baseline and candidate revisions, hardware
and OS, provider/model and settings, workload, warm-up policy, sample count,
raw results, and pass/fail thresholds. Correctness and reliability must not be
traded for a faster median.

## 0.1.x - Stabilize the foundation

The 0.1.x line focuses on existing behavior rather than adding broad product
surface area.

### Reliability and current issues

- Triage and fix reproducible open issues, prioritizing security, data loss,
  installation, state corruption, hangs, and user-visible regressions.
- Require a minimal reproduction, regression test, and coverage in the owning
  business, OS, performance, release, or live-test flow for every bug fix.
- Keep source installation as one command: environment setup, release build,
  and user PATH registration. Environment-only behavior must require the
  explicit `-EnvironmentOnly` or `--environment-only` option.
- Exercise source installation and PATH discovery in fresh shells on Linux,
  macOS, Windows Server 2022, and Windows Server 2025.

### Runtime and session persistence

- Finish the single-source state contract between `runtime` and `session_log`.
  `SessionState` is already owned by `session_log` and re-exported by runtime,
  but snapshot parsing, normalization, transition validation, and persistence
  still cross multiple serialization boundaries.
- Inventory repeated JSON parsing and whole-session cloning on every turn,
  especially runtime checkpoint, context rebuild, queue, and session DB paths.
- Remove redundant parse/serialize cycles only after compatibility fixtures
  prove that old session records, current records, recovery, and compaction have
  identical behavior.
- Benchmark parse count, bytes processed, wall time, allocation pressure, and
  session DB queue latency for short, long-history, compacted, interrupted, and
  concurrent sessions.

### Profiling and performance

- Extend the existing runtime timing hooks in
  `crates/runtime/src/profile_timings.rs`. Current hooks can be enabled with
  `TURA_PROFILE_TURN_TIMINGS` or `TURA_PROFILE_TIMINGS`; optional payload-size
  fields use `TURA_PROFILE_TURN_TIMING_BYTES` or
  `TURA_PROFILE_TIMING_BYTES`.
- Add low-overhead timing boundaries where attribution is still missing. Keep
  labels stable, structured, and correlated by session/runtime identifier.
- Establish TUI budgets for startup, first render, input latency, streamed
  updates, resize, and long-history navigation.
- Establish GUI budgets for startup, session switching, transcript rendering,
  rich-text streaming, plan rendering, calendar navigation, and memory growth.
- Add performance regression gates only after repeatable baselines and variance
  envelopes exist on controlled runners.

### Provider evidence

- Expand real-provider and protocol-fixture coverage beyond the currently
  published Codex/OpenAI-centered benchmark evidence.
- Prioritize distinct protocol families: Anthropic/Claude, Google/Gemini,
  OpenAI-compatible providers such as OpenRouter, local Ollama-compatible
  endpoints, and configured cloud routes where credentials and cost controls
  permit.
- Test streaming, tool calls, parallel tool calls, reasoning metadata, prompt
  caching, usage accounting, retries, rate limits, timeout/cancellation, malformed
  events, and fallback routing.
- Publish provider/model/version, date, settings, failures, and raw artifacts.
  Catalog configuration alone is not compatibility evidence.

### Benchmark and test gaps

- Add cold/warm startup, idle footprint, peak memory, CPU, disk I/O, session DB
  growth, parse/serialization cost, and shutdown/restart measurements.
- Cover short and long conversations, large tool outputs, compaction, concurrent
  sessions, cancellation, process death, queue recovery, and repeated resume.
- Add TUI and GUI interaction latency, dropped-frame/event-loop delay, transcript
  virtualization, plan-scale, and rich-text stress workloads.
- Run OS-sensitive installation, PATH, process-tree cleanup, signals, sockets,
  file locking, permissions, Unicode paths, spaces in paths, and shell-profile
  cases across supported operating systems.
- Separate correctness gates from performance measurements while requiring both
  for performance-oriented changes.

### 0.1.x exit criteria

- All release-blocking 0.1.x issues have regression coverage.
- The canonical state/serialization contract is documented and compatibility
  fixtures pass across runtime and session DB.
- Provider and benchmark matrices publish their covered and uncovered cells.
- TUI, GUI, runtime, and session DB have repeatable baselines with raw artifacts.
- The complete source-install flow passes on the supported CI OS matrix.

## 0.2 - Planning and task workspace

The 0.2 line builds on a stable 0.1.x foundation.

- Complete task-planning behavior, including durable plan state, dependency and
  status transitions, interruption, resume, and user approval boundaries.
- Complete the GUI Plan experience for creating, editing, scheduling, running,
  and auditing plans.
- Add a calendar view backed by the same canonical plan/task data, with timezone
  and rescheduling behavior covered by tests.
- Complete the session task-management page for filtering, grouping, assigning,
  resuming, archiving, and tracing tasks to sessions and execution evidence.
- Keep Plan, Calendar, and Session Tasks as views over one contract rather than
  separate state machines.

### 0.2 exit criteria

- Plan, Calendar, and Session Tasks share one durable task model and recovery
  contract.
- Core workflows have unit, business, GUI end-to-end, restart/recovery, and
  performance coverage.
- Accessibility, keyboard operation, large-plan performance, timezone behavior,
  and migration from 0.1.x session data are verified.

## Ownership

- Primary maintainer: Yohji Sakamoto (`yohji.sakamoto@gmail.com`)
- Project contact: `info@turaai.net`
- Issues and proposals: <https://github.com/Tura-AI/tura/issues>

See [Known Issues](docs/KNOWN_ISSUES.md) for current limitations and
[Contributing](.github/CONTRIBUTING.md) for evidence requirements.
