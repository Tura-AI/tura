# Contributing to Tura

You do not need to understand all of Tura before improving one part of it. Small,
reviewable changes are welcome. A pull request does not have to prove the whole
system; it does have to prove the behavior it owns. That is a much more useful
standard than making every contributor carry the entire repository uphill.

## Start here

1. Search existing issues and pull requests.
2. Choose the contribution type below. Use its issue and pull-request template.
3. Set up dependencies without changing your user PATH.
4. Make the smallest change that satisfies an observable requirement.
5. Run the smallest test layer that owns the behavior, then any affected
   boundary flow.
6. Open a pull request that states what was and was not verified.

Read the [contribution guide](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md) for the test
ownership model, benchmark format, evidence-sanitization rules, and affected
test matrix. Participation in this project is governed by the
[Code of Conduct](https://github.com/Tura-AI/tura/blob/main/.github/CODE_OF_CONDUCT.md).

## Core code and architecture

Read the [repository architecture](https://github.com/Tura-AI/tura/blob/main/ARCHITECTURE.md) before changing a system
boundary. The main implementation owners are:

- [runtime](https://github.com/Tura-AI/tura/tree/main/crates/runtime/src) and its
  [architecture](https://github.com/Tura-AI/tura/blob/main/crates/runtime/ARCHITECTURE.md);
- [session DB](https://github.com/Tura-AI/tura/tree/main/crates/session_log/src) and its
  [architecture](https://github.com/Tura-AI/tura/blob/main/crates/session_log/ARCHITECTURE.md);
- [gateway](https://github.com/Tura-AI/tura/tree/main/crates/gateway/src) and its
  [architecture](https://github.com/Tura-AI/tura/blob/main/crates/gateway/ARCHITECTURE.md);
- [tools](https://github.com/Tura-AI/tura/tree/main/crates/tools/src) and their
  [architecture](https://github.com/Tura-AI/tura/blob/main/crates/tools/ARCHITECTURE.md);
- [TUI](https://github.com/Tura-AI/tura/tree/main/apps/tui/src) and its
  [architecture](https://github.com/Tura-AI/tura/blob/main/apps/tui/ARCHITECTURE.md);
- [GUI](https://github.com/Tura-AI/tura/tree/main/apps/gui/app/src) and its
  [architecture](https://github.com/Tura-AI/tura/blob/main/apps/gui/ARCHITECTURE.md).

Follow existing ownership and contract boundaries rather than adding a parallel
state model, parser, protocol, or test hierarchy.

## Choose the contribution type

| Type | Open an issue first? | Primary evidence | Template |
| --- | --- | --- | --- |
| Bug fix | Recommended; required for security-sensitive or broad fixes | Reproduction, root cause, smallest owning regression test | [Bug fix](https://github.com/Tura-AI/tura/compare?expand=1&template=bug_fix.md) |
| Feature or behavior change | Required for large features, state changes, migrations, or compatibility breaks | User problem and observable acceptance criteria | [Feature](https://github.com/Tura-AI/tura/compare?expand=1&template=feature.md) |
| Performance or efficiency claim | Required | Before/after end-to-end benchmark and correctness comparison | [Performance](https://github.com/Tura-AI/tura/compare?expand=1&template=performance.md) |
| Provider compatibility | Required for a new provider or protocol family | Protocol fixtures; scoped live evidence when needed | [Provider](https://github.com/Tura-AI/tura/compare?expand=1&template=provider.md) |
| Documentation only | Not normally | Source accuracy, links, and rendered/readable output | [Documentation](https://github.com/Tura-AI/tura/compare?expand=1&template=documentation.md) |

Use the repository's **New issue** chooser for matching issue forms. Report
security vulnerabilities privately through
[SECURITY.md](https://github.com/Tura-AI/tura/blob/main/.github/SECURITY.md).

## Open harness principle

We believe the best agent harness should be open source. Logic controlled by
this project that is necessary to reproduce a public claim must be inspectable:
prompts, tool contracts, runner behavior, benchmark methodology, scoring rules,
and failure classification. Do not add hidden benchmark branches, private
scoring logic, or prompt-specific production behavior.

Reproducibility does not mean every dependency must be open source or runnable
offline. For commercial providers, judge models, licensed datasets, and other
external systems, record the interface, provider/model or dataset version,
settings, date, known limitations, and access requirements. When material cannot
be published for privacy, security, license, or provider-policy reasons, provide
a safe substitute such as a redacted fixture, content hash, generator, schema,
or private disclosure path. Never publish an exploit payload that belongs in a
private security report.

## Scope and evidence

Apply YAGNI (You Aren't Gonna Need It): do not add speculative state,
compatibility layers, provider abstractions, or generalized behavior without a
demonstrated requirement. Explain why each new abstraction is needed now.

Only pull requests that make a performance or efficiency claim must provide the
full benchmark evidence defined in the
[contribution guide](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md#performance-and-efficiency-evidence).
Changes whose primary value is simpler code, a lower resource ceiling, or a
better worst case may use that as their acceptance criterion; do not relabel
them as an average-speed improvement without evidence.

## Contribution license and provenance

By submitting a contribution, you agree that it may be distributed under the
repository's license and confirm that you have the right to submit it. Do not
include third-party code, data, prompts, fixtures, or generated material whose
license or provenance is unclear.

See [LICENSE](https://github.com/Tura-AI/tura/blob/main/LICENSE) for the
repository license. Identify any compatible
third-party material and its source in the pull request when it is necessary to
review provenance or redistribution rights.

## Bug fixes and regression coverage

By default, a bug fix includes a regression test that fails without the fix and
passes with it. Add coverage in the **smallest test layer that owns the affected
behavior**. Add a higher-level test only when the failure crossed a process,
storage, protocol, OS, release, TUI, or GUI boundary.

Some failures cannot be retained as stable automated tests, including rare
hardware races, temporary upstream outages, confidential security inputs, and
inputs that cannot be redistributed. In that case, explain the limitation and
provide the strongest durable substitute: a deterministic model or fixture, a
stress test, a sanitized trace, a fault-injection case, a manual reproduction,
or a follow-up issue. An exception is evidence to review, not permission to omit
verification silently.

If an unrelated assertion appears outdated, do not rewrite it merely to get a
green result. Describe the mismatch in the pull request.

## Development setup

For contribution work, start with dependency-only setup. It does not build a
release or register Tura on your user PATH:

```powershell
.\scripts\install.ps1 -EnvironmentOnly
```

```bash
./scripts/install.sh --environment-only
```

The default installer is the full end-user flow. It builds into
`target/release` and registers that directory on the user PATH:

```powershell
.\scripts\install.ps1
```

```bash
./scripts/install.sh
```

On Windows, registration updates the user PATH. On Linux and macOS, it adds a
marked block to applicable user shell profiles. It does not overwrite unrelated
PATH entries, but an existing `tura` command may resolve differently after the
new entry is added. PATH registration itself is user-scoped; dependency package
managers may separately request elevation. Undo registration with:

```powershell
.\scripts\unregister-cli.ps1
```

```bash
./scripts/unregister-cli.sh
```

See [Install](https://github.com/Tura-AI/tura/blob/main/docs/start/install.md)
for exact files, effects, and cleanup.

## Choosing tests

Use the [test ownership table](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md#test-ownership)
and the full [testing reference](https://github.com/Tura-AI/tura/blob/main/tests/README.md).
Common entrypoints include:

| Surface | Quality | Deterministic tests | Broader affected flow |
| --- | --- | --- | --- |
| Backend | `scripts/check-backend-quality.*` | owning crate or `run-backend-business-tests.*` | `run-backend-os-tests.*`, performance, release, or live runner only when affected |
| TUI | `npm --prefix apps/tui run check` | `npm --prefix apps/tui run test:unit` | `test:e2e`, `test:business`, or the affected performance/live command |
| GUI | `bun run --cwd apps/gui check` | `bun run --cwd apps/gui test:unit` | `test:e2e`, `test:performance`, or the affected live command |

Live-provider tests are opt-in, may cost money, and must not use contributor
credentials in an untrusted pull request.

## Opening a pull request

Create a focused branch and commit only related files:

```bash
git switch -c fix/short-description
git add <related-paths>
git commit -m "Fix short description"
git push -u origin fix/short-description
```

Open the matching template from the contribution-type table, select the correct
base and compare branches, and complete only that template's requirements. If
you open a pull request without a type-specific template, the generic template
will ask you to select a type and provide the matching evidence.

- Keep one primary contribution type per pull request. Split unrelated fixes.
- Link the issue when one is required and describe the user-visible result.
- Explain the root cause or requirement, not only the edited files.
- List exact commands and summarized results. Say plainly what was not run.
- Report only affected OS, surface, provider/protocol, behavior, and state
  dimensions; there is no requirement to enumerate an infinite Cartesian
  product.
- Update documentation when setup, behavior, architecture, compatibility, or
  a public claim changes.
- Update the [roadmap](https://github.com/Tura-AI/tura/blob/main/ROADMAP.md) or
  [known issues](https://github.com/Tura-AI/tura/blob/main/docs/KNOWN_ISSUES.md)
  only when evidence changes their
  status.
- Keep credentials, private session data, unsafe provider logs, and generated
  local state out of commits and pull requests.

Maintainers may decline a correct change when its complexity is not justified by
the demonstrated requirement, when a public claim is not reproducible, or when
the affected behavior lacks reasonable regression coverage.

## Authorship and tool assistance

A human must be the primary submitter of every contribution. The primary human
submitter is responsible for correctness, licensing, provenance, verification,
and every statement made in the pull request. Meaningful tool or AI assistance
may be disclosed in the pull request or acknowledged through the repository's
normal commit conventions. Such acknowledgement does not make the tool the
responsible submitter and does not transfer responsibility away from the human
contributors.

Use clear imperative commit subjects and follow the repository's existing
history for commit structure.

## Contact

- Primary maintainer: Yohji Sakamoto (`yohji.sakamoto@gmail.com`)
- Project contact: `info@turaai.net`
