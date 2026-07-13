## Performance or efficiency claim

Link the approved performance issue and state one measurable end-to-end claim.

Closes #

## Why this change

Identify the measured bottleneck or resource ceiling. Explain why the change is
the smallest one that addresses it.

## Benchmark contract

- Baseline and candidate commits:
- Exact command and workload:
- OS and hardware:
- Provider/model/settings, if relevant:
- Warm-up and measured sample count:
- Pass/fail threshold:

## Results

| Metric | Baseline | Candidate | Difference |
| --- | ---: | ---: | ---: |
| p50 | | | |
| p95 | | | |
| IQR | | | |
| failures/timeouts | | | |
| correctness score | | | |
| relevant CPU/memory/I/O/token metric | | | |

- Raw sanitized JSON/CSV artifact:
- Exclusions and predeclared rule:
- Live-provider variability, retries, or rate limits:

Do not claim a general speedup from an internal timer alone. If the value is a
lower peak, bounded worst case, or simpler implementation, state and measure
that criterion directly.

## Regression and matrix coverage

- Correctness commands and results:
- Affected OS/surface/provider/state cells not run and why:

## Evidence safety

- [ ] Artifacts were scanned and sanitized as required by `docs/contributing-guide.md`.
- [ ] No credentials, authorization headers, private prompts/sessions, restricted inputs, or unsafe provider logs are included.
- [ ] Human contributors accept responsibility for the claim, correctness, licensing, and provenance.

Meaningful tool or AI assistance, if useful to reviewers (optional):
