# Tura documentation

Tura is a terminal-native developer tool that turns intent into verified code
changes. The emphasis is on reading the repository first, keeping an audit
trail, and proving the result before calling the work done.

This `docs/` tree is the GitBook-oriented starting point. It gives readers a
stable route through the project, then links to the source-owned references
when the implementation details matter. Copying those details here would only
give us two documents to keep honest.

## Main paths

- [Start navigation](start/navigation.md) - the shortest path through user-facing docs.
- [GitBook summary](SUMMARY.md) - the full table of contents.
- [Benchmark methodology](https://github.com/Tura-AI/benchmark/blob/main/doc/benchmark-methodology.md) - scope, selection, scoring, and limitations.
- [Current test-set evidence record](https://github.com/Tura-AI/benchmark/blob/main/doc/current-test-set-record.md) - acquisition, provenance, recomputed claims, anomalies, design observations, and missing ablations.
- [Benchmark repository](https://github.com/Tura-AI/benchmark) - tasks, harnesses, and published results.
- [Roadmap](../ROADMAP.md) - 0.1.x stabilization and 0.2 planning priorities.
- [Known issues](KNOWN_ISSUES.md) - architectural risks and missing evidence.
- [Contributing](../.github/CONTRIBUTING.md) - regression and benchmark requirements.
- [Contribution guide](contributing-guide.md) - contribution types, test ownership, evidence format, and affected matrices.
- [Development architecture](../ARCHITECTURE.md) - how the repository is owned and built.

## Documentation policy

- User-facing docs live in `docs/`.
- Existing deep references, crate `README.md`, and crate `ARCHITECTURE.md` stay
  linked from the matching `docs/` page.
- Do not copy large sections when a link to the owner document is clearer.
