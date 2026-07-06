# Tura documentation

Tura is a terminal-native developer tool for turning intent into verified code
changes with disciplined motion, audit trails, and repo-aware control.

This `doc/` tree is the GitBook-oriented documentation entry. It organizes the
existing repository documentation into a stable reading path and links to deeper
source-owned references instead of duplicating long implementation notes.

## Main paths

- [Start navigation](start/navigation.md) - the shortest path through user-facing docs.
- [GitBook summary](SUMMARY.md) - the full table of contents.
- [Development architecture](development/architecture.md) - how the repository is owned and built.

## Documentation policy

- User-facing docs live in `doc/`.
- Existing deep references under `docs/`, crate `README.md`, and crate
  `ARCHITECTURE.md` stay linked from the matching `doc/` page.
- Do not copy large sections when a link to the owner document is clearer.
