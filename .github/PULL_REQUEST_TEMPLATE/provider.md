## Provider or protocol change

Link the approved provider issue.

Closes #

- Provider and protocol family:
- Models used for verification, if any:
- External service/version/date/settings:

## Behavior covered

Mark only affected behavior.

- [ ] Streaming
- [ ] Tool calls
- [ ] Parallel tool calls
- [ ] Authentication
- [ ] Usage or reasoning metadata
- [ ] Prompt caching
- [ ] Retry, rate limit, timeout, or cancellation
- [ ] Fallback routing
- [ ] Error normalization

## Reproducible evidence

- Deterministic protocol fixtures:
- Mock/local endpoint coverage:
- Live evidence that fixtures cannot prove, if any:
- External access, licensing, or redistribution limits:
- Redacted fixture, hash, schema, or generator for restricted material:

```text
command -> result
```

Do not add production branches that recognize benchmark prompts or exact model
text. A new provider does not require testing every model sold by that provider;
state the protocol and model scope actually covered.

## Compatibility and safety

- Existing provider routes affected:
- Affected behavior/OS/surface cells not run and why:
- [ ] No keys, tokens, cookies, authorization headers, private sessions, or unsafe provider logs are included.
- [ ] A human is the primary submitter and accepts responsibility for correctness, licensing, provenance, verification, and the statements in this pull request.
- [ ] I have the right to submit all included material under the repository license and any applicable provider terms.

Meaningful tool or AI assistance, if useful to reviewers (optional):
