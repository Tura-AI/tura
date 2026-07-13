# Contributing to Tura

Thank you for improving Tura. The project values small changes, reproducible
evidence, and durable tests over broad rewrites.

## Open harness principle

We believe the best agent harness should be open source. Prompts, tool
contracts, runner behavior, benchmark methodology, evaluation criteria, and
failure evidence should be inspectable and reproducible. Contributions must not
introduce hidden benchmark-only behavior, opaque scoring paths, or private logic
required to reproduce a public claim.

## Before starting

- Search existing issues and pull requests.
- Open an issue before a large feature, state-model change, data migration,
  provider expansion, or user-facing compatibility break.
- Never include API keys, OAuth tokens, session databases, provider logs, or
  private benchmark inputs.
- For security reports, follow [SECURITY.md](SECURITY.md) instead of opening a
  public issue.

## Evidence-first rule

Apply YAGNI (You Aren't Gonna Need It): do not implement speculative behavior,
state, compatibility, or abstraction without a demonstrated requirement. A
performance or efficiency change that cannot prove an improvement through a
relevant benchmark or evaluation should not exist in the codebase.

Performance evidence must include:

- baseline and candidate commit IDs;
- exact command, workload, provider/model/settings, OS, and hardware;
- warm-up policy, sample count, raw results, and variance;
- correctness results and resource trade-offs;
- before/after p50 and tail latency where latency is claimed.

Do not optimize only one internal timer if end-to-end behavior is unchanged.
Do not accept lower correctness, reliability, or recovery quality as a hidden
performance trade.

## Bug-fix contract

Every bug fix must include:

- a minimal reproduction or failing test;
- a regression test that fails before the fix and passes after it;
- coverage in the flow that previously missed the bug: unit, business, OS,
  performance, release, live, TUI, or GUI end-to-end;
- documentation updates when behavior, setup, architecture, or compatibility
  changes.

If a test assertion unrelated to your change appears outdated, do not rewrite it
merely to get green CI. Explain the mismatch in the pull request.

## Development setup

The default installer performs environment setup, release build, and user PATH
registration:

```powershell
.\scripts\install.ps1
```

```bash
./scripts/install.sh
```

For dependency setup without a release build or PATH registration, explicitly
use environment-only mode:

```powershell
.\scripts\install.ps1 -EnvironmentOnly
```

```bash
./scripts/install.sh --environment-only
```

See [Install](../docs/start/install.md) and
[Testing](../tests/README.md) for targeted commands.

## Choosing tests

Run the smallest owning suite while developing, then the complete affected flow
before requesting review.

```powershell
.\scripts\check-backend-quality.ps1
.\xtask\scripts\run-backend-business-tests.ps1
.\xtask\scripts\run-backend-os-tests.ps1
.\xtask\scripts\run-backend-performance-tests.ps1
```

```bash
sh scripts/check-backend-quality.sh
sh xtask/scripts/run-backend-business-tests.sh
sh xtask/scripts/run-backend-os-tests.sh
sh xtask/scripts/run-backend-performance-tests.sh
```

Use the app-owned commands for TUI and GUI suites. Live-provider tests are
opt-in, may cost money, and must never run with contributor credentials in an
untrusted pull request.

## Pull requests

- Keep the scope narrow and explain the root cause.
- Link the issue and identify user-visible behavior.
- List changed contracts and compatibility risks.
- Include exact test commands and summarized results.
- Attach raw benchmark/evaluation artifacts for performance claims.
- State which OS and provider/model cells were and were not tested.
- Keep generated files, local state, logs, and secrets out of the commit.
- Update `ROADMAP.md` or `docs/KNOWN_ISSUES.md` only when evidence changes their
  status.

Maintainers may decline a correct change if its complexity is not justified by
measured value or if it lacks regression coverage.

## Commit authorship

Use clear imperative commit subjects. If Tura AI materially contributes to a
commit, append this trailer after a blank line:

```text
Co-authored-by: Tura AI <info@turaai.net>
```

## Contact

- Primary maintainer: Yohji Sakamoto (`yohji.sakamoto@gmail.com`)
- Project contact: `info@turaai.net`
