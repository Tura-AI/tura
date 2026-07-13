# Contribution Guide

This guide turns Tura's contribution principles into a practical review
contract. It defines how to choose evidence without making every change satisfy
the same checklist.

## Contribution paths

| Change | What must be demonstrated | What is not required by default |
| --- | --- | --- |
| Bug fix | The failure, its root cause, and durable coverage at the owning layer | A full provider, OS, UI, and performance matrix |
| Feature | A current user problem, narrow acceptance criteria, and compatibility impact | Benchmark evidence unless performance is claimed |
| Performance | A named end-to-end claim, controlled comparison, correctness, and raw sanitized measurements | Unaffected providers and surfaces |
| Provider | Protocol behavior, fixture coverage, external-system metadata, and any live limitations | Every model sold by the provider |
| Documentation | Accuracy against an owning source, valid links, and readable structure | Code regression tests unless executable behavior changed |

Security reports follow the
[security policy](https://github.com/Tura-AI/tura/blob/main/.github/SECURITY.md),
not public issue or pull-request templates.

## Test ownership

Choose the smallest layer that can fail for the real defect or acceptance
criterion. Escalate only when the behavior crosses a boundary.

| Layer | Owns | Typical entrypoint |
| --- | --- | --- |
| Unit or crate | Parser, state transition, pure logic, schema, component behavior | Owning package test command or crate test |
| Business | Deterministic workflow across modules without public services | `xtask/scripts/run-backend-business-tests.*`, TUI `test:business`, or GUI `test:e2e` |
| OS | Processes, sockets, PATH, shell, ownership, shutdown, or OS policy | `xtask/scripts/run-backend-os-tests.*` or OS wrapper |
| Performance | Latency, throughput, memory, CPU, I/O, token, load, or soak claim | Owning performance runner |
| Release | Built and packaged command behavior | Release runner for the affected surface |
| Live provider | Behavior that cannot be proven with a protocol fixture | Explicit opt-in live runner |
| TUI or GUI end-to-end | User interaction or rendering across the app boundary | App-owned end-to-end command |

A parser fix normally needs a parser or crate regression test. Add GUI coverage
only if GUI behavior or its serialization boundary allowed the defect to escape.
An installer PATH defect belongs in OS/install coverage rather than a provider
suite. This is how regression depth stays consistent with YAGNI.

When stable automation is not possible, document why and provide the strongest
durable substitute. Acceptable substitutes include deterministic fault
injection, a reduced fixture, a bounded stress reproducer, a sanitized trace, a
manual procedure with expected observations, or a follow-up issue describing
the missing harness capability.

## Affected test matrix

Report only dimensions touched by the change. "Not tested" means an affected
dimension was skipped; it does not mean listing every possible model or machine.

| Dimension | Values to consider when affected |
| --- | --- |
| Surface | Backend/runtime, session DB, gateway, CLI, TUI, GUI, Tauri, source installer, npm package |
| OS/package | Linux x64, macOS arm64, macOS x64, Windows x64; record the exact runner or host version |
| Provider protocol | OpenAI/Codex, Anthropic, Google, OpenAI-compatible third party, local endpoint |
| Provider behavior | Streaming, tool calls, parallel calls, auth, usage, reasoning metadata, caching, retry/rate limit, cancellation, fallback |
| Persistent state | Fresh state, current records, legacy records, compaction, interruption/recovery, concurrent sessions |

The repository currently exercises source installation on Ubuntu, macOS,
Windows Server 2022, and Windows Server 2025. Release packages target Linux x64,
macOS arm64/x64, and Windows x64. That describes current project coverage; it is
not a demand that every pull request rerun every cell.

For each affected dimension, record one of: verified, covered by a deterministic
fixture, not run with a reason, or not applicable.

## Performance and efficiency evidence

The full evidence contract applies when a pull request claims that something is
faster, cheaper, more memory-efficient, more scalable, or materially improves a
resource limit. Ordinary bug fixes, documentation changes, and refactors do not
need a benchmark report unless they make such a claim or introduce an obvious
performance risk.

Define the claim before measuring. Prefer an end-to-end user or system metric.
An internal timer may diagnose the cause, but it does not by itself prove user
value.

Every claimed comparison includes:

- baseline and candidate commit IDs;
- exact command, workload, OS, hardware, and relevant provider/model/settings;
- warm-up policy and measured sample count;
- p50 and p95 for latency claims; use p99 only with enough samples and state the
  sample count;
- minimum, maximum, and IQR as the default spread summary;
- failures, timeouts, retries, and correctness/evaluation score;
- relevant CPU, peak memory, disk I/O, network, or token usage;
- a stated pass/fail threshold and machine-readable raw measurements in JSON or
  CSV.

Do not silently remove outliers. If an observation is excluded, retain it in the
raw file and explain the predeclared exclusion rule. For network providers,
record the date/time window, rate-limit or retry behavior, request ordering, and
provider-side variability. Separate controlled fixture results from noisy live
service measurements; a live-provider comparison alone should not become a hard
regression gate.

A change may be valuable because it reduces complexity, bounds peak memory,
removes duplicate work, or improves a worst case without moving average latency.
State that value directly and choose a matching measurement instead of claiming
a general speedup.

## Safe and reproducible evidence

"Raw" means minimally processed measurements sufficient to recompute the
summary. It does not mean publishing secrets or unrestricted request logs.

Before attaching or committing an artifact:

- scan for API keys, OAuth material, cookies, authorization/request headers,
  account identifiers, session IDs, and private filesystem paths;
- remove personal data and private prompt/session content;
- retain only provider response fields that may be redistributed;
- replace restricted inputs with a hash, schema, generator, redacted fixture,
  or access instructions;
- include a manifest with commit, command, environment, settings, and artifact
  hashes;
- state where the artifact is stored and its license or retention limits.

Use the benchmark repository for published benchmark datasets and reports when
that repository owns the claim. Small regression fixtures should remain with
the owning test. Confidential security evidence goes only through the private
security-reporting path.

## Pull-request preparation

Before opening a pull request:

1. Select one primary PR type and use its template.
2. Rebase or merge only according to normal repository policy; do not rewrite
   unrelated contributor work.
3. Confirm the diff contains no generated local state or credentials.
4. Run the smallest owning checks and any crossed boundary checks.
5. Record commands, results, and skipped affected cells.
6. Link the issue when the contribution type requires one.
7. Explain compatibility, migration, and rollback concerns when they exist.

The primary submitter must be human and remains responsible for the contribution
even when coding, analysis, or writing tools were used. Tool or AI assistance
may be disclosed in the pull request or acknowledged through normal repository
commit conventions, but responsibility for correctness, licensing, provenance,
verification, and review statements remains with the human contributors.

By submitting a contribution, you agree that it may be distributed under the
repository's license and confirm that you have the right to submit it. Do not
include third-party code, data, prompts, fixtures, or generated material whose
license or provenance is unclear. Review the
[Code of Conduct](https://github.com/Tura-AI/tura/blob/main/.github/CODE_OF_CONDUCT.md),
the [repository architecture](https://github.com/Tura-AI/tura/blob/main/ARCHITECTURE.md),
and the core-code links in
[CONTRIBUTING.md](https://github.com/Tura-AI/tura/blob/main/.github/CONTRIBUTING.md#core-code-and-architecture)
before changing ownership or system boundaries.
