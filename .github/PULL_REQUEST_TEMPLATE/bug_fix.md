## Bug fixed

Link the issue and describe the observed failure.

Fixes #

## Root cause and change

Explain the root cause, why the previous flow missed it, and why this is the
smallest sufficient fix.

## Regression evidence

- Smallest owning test layer:
- Failure before the fix:
- Result after the fix:
- Higher-level boundary coverage, if the failure crossed one:

```text
command -> result
```

If stable automation is not possible, explain the limitation and provide the
durable substitute evidence described in `docs/contributing-guide.md`.

## Affected matrix

List only affected OS, surface, provider/protocol, behavior, or persistent-state
cells. Mark each verified, fixture-covered, not run with a reason, or not
applicable.

## Compatibility and safety

- User-visible behavior changed:
- Compatibility or migration risk:
- [ ] No credentials, private session data, unsafe provider logs, or generated local state are included.
- [ ] Human contributors accept responsibility for correctness, licensing, and provenance.

Meaningful tool or AI assistance, if useful to reviewers (optional):
