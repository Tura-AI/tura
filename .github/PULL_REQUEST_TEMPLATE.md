## Pull request type

Choose one primary type. If the change needs more focused questions, use the matching template in
[PULL_REQUEST_TEMPLATE/](https://github.com/Tura-AI/tura/tree/main/.github/PULL_REQUEST_TEMPLATE):

- [ ] [Bug fix](https://github.com/Tura-AI/tura/compare?expand=1&template=bug_fix.md)
- [ ] [Feature or behavior change](https://github.com/Tura-AI/tura/compare?expand=1&template=feature.md)
- [ ] [Performance or efficiency claim](https://github.com/Tura-AI/tura/compare?expand=1&template=performance.md)
- [ ] [Provider compatibility](https://github.com/Tura-AI/tura/compare?expand=1&template=provider.md)
- [ ] [Documentation only](https://github.com/Tura-AI/tura/compare?expand=1&template=documentation.md)
- [ ] Maintenance or other

## Outcome

Start with the result a user or maintainer can observe, and link the issue when
required. Then explain why the change is no larger than it needs to be.

## Scope and compatibility

- Changed contracts, state, storage, protocol, CLI, GUI, TUI, or installation behavior:
- Compatibility or migration risk:
- Explicitly out of scope:

## Verification

List the exact commands and summarized results. Name any skipped **affected** OS,
surface, provider/protocol, behavior, or state cells and why. Do not enumerate
unaffected combinations.

```text
command -> result
```

## Type-specific evidence

Complete only what applies:

- Bug: reproduction, root cause, and smallest owning regression layer.
- Feature: current user problem and observable acceptance criteria.
- Performance: claim plus the benchmark fields in the
  [contribution guide](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md#performance-and-efficiency-evidence).
- Provider: protocol fixture/live boundary and external dependency metadata.
- Documentation: owning sources checked and links/rendering verified.

If stable automation was not possible, explain why and give the durable
substitute evidence.

## Safety and responsibility

- [ ] No credentials, private session data, unsafe provider logs, or generated local state are included.
- [ ] Public evidence was sanitized according to the
      [contribution guide](https://github.com/Tura-AI/tura/blob/main/docs/contributing-guide.md#safe-and-reproducible-evidence).
- [ ] A human is the primary submitter and accepts responsibility for correctness, licensing, provenance, verification, and the statements in this pull request.
- [ ] I have the right to submit all included code, data, prompts, fixtures, and generated material under the repository license.

Meaningful tool or AI assistance, if useful to reviewers (optional):
