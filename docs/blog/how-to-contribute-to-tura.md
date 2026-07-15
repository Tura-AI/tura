# How to Contribute to Tura

You do not need to understand the entire repository before contributing. Start with one observable problem, identify the component that owns it, and verify the change at that boundary.

## Choose the contribution type

Before editing, search existing [issues](https://github.com/Tura-AI/tura/issues) and [pull requests](https://github.com/Tura-AI/tura/pulls).

| Change | Required evidence |
| --- | --- |
| Bug fix | Reproduction, root cause, and a regression test at the smallest owning layer |
| Behavior change | User problem, observable acceptance criteria, and compatibility impact |
| Performance improvement | Controlled before/after comparison, correctness results, and sanitized raw measurements |
| Provider compatibility | Protocol fixtures and focused live evidence where fixtures are insufficient |
| Documentation | Accurate sources, working links, and a readable rendered result |

Large features, migrations, compatibility breaks, new provider families, state-model changes, and performance claims need an issue first. Small documentation fixes usually do not. Report security problems privately through [SECURITY.md](https://github.com/Tura-AI/tura/blob/main/.github/SECURITY.md).

The full decision table and pull-request templates are in [CONTRIBUTING.md](https://github.com/Tura-AI/tura/blob/main/.github/CONTRIBUTING.md#choose-the-contribution-type).

## Set up the contributor environment

Use the dependency-only installer so development setup does not register a new `tura` command on your user `PATH`.

```powershell
# Windows PowerShell
.\scripts\install.ps1 -EnvironmentOnly
```

```bash
# macOS or Linux
./scripts/install.sh --environment-only
```

The default installer performs a release build and registers Tura. Installed files, side effects, and cleanup commands are documented in [docs/start/install.md](https://github.com/Tura-AI/tura/blob/main/docs/start/install.md).

## Find the owning boundary

Tura assigns major responsibilities to specific components:

- runtime handles model output and agent execution;
- provider performs and normalizes model calls;
- tools own model-visible commands and execution policy;
- router dispatches workers and owns routing lifecycle;
- session DB owns durable session state;
- gateway exposes backend behavior to frontends;
- TUI and GUI own their interaction and rendering behavior.

Read the relevant section of [ARCHITECTURE.md](https://github.com/Tura-AI/tura/blob/main/ARCHITECTURE.md) before changing a system boundary. Major directories also contain local `ARCHITECTURE.md` files.

A fix in the wrong layer can create a second parser, state model, retry policy, or source of truth. Change the existing owner unless the architecture explicitly requires a new boundary.

## Keep the change focused

A focused diff has one primary job. Reproduce the failing behavior, modify the owning code, and add evidence that fails before the change and passes after it.

Do not add abstractions for hypothetical features. Avoid combining a bug fix with unrelated renaming, formatting, cleanup, or redesign. If a change crosses a real boundary, test that boundary.

## Run the test that owns the behavior

Start with the smallest test that can catch the regression:

- pure logic, parsing, schemas, or state transitions: owning unit or crate test;
- deterministic workflows across modules: owning business test;
- processes, sockets, shell behavior, PATH, permissions, or shutdown: OS test;
- visible TUI or GUI behavior: app unit test and affected end-to-end flow;
- latency, memory, tokens, or throughput: performance runner plus correctness check;
- provider behavior that fixtures cannot reproduce: explicit opt-in live test.

Common commands are listed in [CONTRIBUTING.md](https://github.com/Tura-AI/tura/blob/main/.github/CONTRIBUTING.md#choosing-tests). The complete ownership and affected-test matrix is in [docs/contributing-guide.md](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md#test-ownership).

> A pull request does not need to prove everything. It needs to prove the behavior it owns.

## Support performance claims with measurements

An internal timer is diagnostic evidence, not proof of a user-visible improvement. A performance comparison should identify:

- baseline and candidate commits;
- exact command and workload;
- hardware, operating system, and settings;
- warm-up policy and sample count;
- correctness results, failures, and excluded observations;
- the metric and threshold used to decide the result;
- distributions such as p50 and p95 for latency claims.

Do not silently remove outliers or trade correctness for a faster median. If the result is lower complexity, bounded memory, or a better worst case rather than lower latency, report it that way.

The full format is in [docs/contributing-guide.md](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md#performance-and-efficiency-evidence).

## Submit test reports

Reproducible test reports are useful contributions. Tura still needs broader coverage across reasoning levels, models, providers, agent architectures, frontends, packaged desktop behavior, and operating systems.

Include the Tura commit, agent configuration, model and provider version, reasoning level, operating system, hardware, command, workload, expected result, observed result, and sanitized artifacts. Failed and neutral results are useful when another person can reproduce them.

See [We Need More Benchmark Data and Test Reports](https://github.com/Tura-AI/tura/blob/main/docs/blog/we-need-more-benchmark-data-and-test-reports.md) for proposed comparisons and ablations. Open gaps are tracked in [Known Issues](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md).

## Open an accurate pull request

Create a focused branch and commit only related files:

```bash
git switch -c fix/short-description
git add <related-paths>
git commit -m "Fix short description"
git push -u origin fix/short-description
```

In the matching pull-request template, explain:

- the user-visible problem or requirement;
- the root cause for a bug;
- what changed and why that layer owns it;
- exact checks and summarized results;
- affected checks not run and the reason;
- compatibility, migration, or rollback concerns.

Review the diff for credentials, authorization headers, private prompts, session data, personal paths, provider logs, and generated local state. Evidence should be detailed enough to verify without exposing private data.

You must have the right to submit all code, data, prompts, fixtures, and generated material. Tura is licensed under AGPL-3.0-or-later. AI tools may assist, but the human submitter remains responsible for correctness, licensing, provenance, verification, and the pull-request description.

## Start with a bounded contribution

Good first changes include documentation corrections, reproducible parser edge cases, clearer error messages, and removal of proven duplication. Choose one result you can observe and verify.

If a full issue is premature, share the finding by mentioning `@tura-ai-agent` or using `#tura-ai-agent`.

The goal is to make Tura the strongest-performing open-source coding agent.

## Reference documents

- [CONTRIBUTING.md](https://github.com/Tura-AI/tura/blob/main/.github/CONTRIBUTING.md) - contribution types, setup, tests, and pull requests.
- [Contributing guide](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md) - test ownership, performance evidence, and sanitization.
- [ARCHITECTURE.md](https://github.com/Tura-AI/tura/blob/main/ARCHITECTURE.md) - repository boundaries and ownership.
- [Tests README](https://github.com/Tura-AI/tura/blob/main/tests/README.md) - complete testing reference.
- [Installation](https://github.com/Tura-AI/tura/blob/main/docs/start/install.md) - setup, PATH effects, and cleanup.
- [Known Issues](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md) - current evidence gaps.
- [Code of Conduct](https://github.com/Tura-AI/tura/blob/main/.github/CODE_OF_CONDUCT.md) - community expectations.
- [Security policy](https://github.com/Tura-AI/tura/blob/main/.github/SECURITY.md) - private vulnerability reporting.

## Contact

Email: [info@turaai.net](mailto:info@turaai.net)
