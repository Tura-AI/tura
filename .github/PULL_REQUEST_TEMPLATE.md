## Pull request type

Select one primary type. For more focused prompts, use the matching template in
`.github/PULL_REQUEST_TEMPLATE/`:

- [ ] [Bug fix](https://github.com/Tura-AI/tura/compare?expand=1&template=bug_fix.md)
- [ ] [Feature or behavior change](https://github.com/Tura-AI/tura/compare?expand=1&template=feature.md)
- [ ] [Performance or efficiency claim](https://github.com/Tura-AI/tura/compare?expand=1&template=performance.md)
- [ ] [Provider compatibility](https://github.com/Tura-AI/tura/compare?expand=1&template=provider.md)
- [ ] [Documentation only](https://github.com/Tura-AI/tura/compare?expand=1&template=documentation.md)
- [ ] Maintenance or other

## Outcome

Describe the user-visible or maintainer-visible result and link the issue when
required. Explain why this is the smallest sufficient change.

## Scope and compatibility

- Changed contracts, state, storage, protocol, CLI, GUI, TUI, or installation behavior:
- Compatibility or migration risk:
- Explicitly out of scope:

## Verification

List exact commands and summarized results. State skipped **affected** OS,
surface, provider/protocol, behavior, or state cells and why. Do not enumerate
unaffected combinations.

```text
command -> result
```

## Type-specific evidence

Complete only what applies:

- Bug: reproduction, root cause, and smallest owning regression layer.
- Feature: current user problem and observable acceptance criteria.
- Performance: claim plus the benchmark fields in `docs/contributing-guide.md`.
- Provider: protocol fixture/live boundary and external dependency metadata.
- Documentation: owning sources checked and links/rendering verified.

If stable automation was not possible, explain why and give the durable
substitute evidence.

## Safety and responsibility

- [ ] No credentials, private session data, unsafe provider logs, or generated local state are included.
- [ ] Public evidence was sanitized according to `docs/contributing-guide.md`.
- [ ] Human contributors accept responsibility for correctness, licensing, and provenance.

Meaningful tool or AI assistance, if useful to reviewers (optional):
