# Agent-group round, token, success, and cost analysis

Source: https://raw.githubusercontent.com/Tura-AI/benchmark/main/assets/model-run-statistics/analysis.md

# Agent-group round, token, success, and cost analysis

## Scope and grain

- Source: run contracts under `results/debug` and `results/rewrite`.
- Source grain: 280 runs across 25 tasks.
- Analysis grain after the documented Tura Balanced long-tail exclusion: 278 runs across 25 tasks.
- Exclusions: 2 configured runs: dynamodb-toolbox-conditional-attribute-requirements-tura-balanced-run-01 (242 rounds), quill-shared-toolbar-focus-tura-balanced-run-01 (113 rounds). They remain in source results and aggregate score tables but are omitted from every statistical figure and fitted relationship.
- Grouping: Tura Balanced High, Tura Direct High, Codex CLI Medium, and Codex CLI High remain separate configurations.
- Rounds: reconstructed from each run's contiguous `agent-rounds.jsonl` indexes.
- Usage: read from the run-level aggregate contract and, where the historical schema populated usage, independently checked against summed provider-round usage.
- Source usage-complete runs: 279; usage-unavailable runs: 1.
- Aggregate-only historical usage: 70 runs; their round contracts contain null usage fields.
- Success: `sum(passed) / sum(checks)` for weighted summaries; points retain run-level ratios.
- Cost: `(uncached input*5 + cached input*0.5 + output*30) / 1,000,000` USD.

## Formula test

The supplied formula is interpreted as `T(n) = nB + c*n*(n+1)/2`. Both candidate models have two parameters and are compared with leave-one-task-out RMSLE. The quadratic-context form is retained when its RMSLE is within 5% of the power-law model; otherwise `T(n) = a*n^p` is selected.

| Agent group | Quadratic CV RMSLE | Power CV RMSLE | Selected | Power-law estimate |
|---|---:|---:|---|---|
| Tura Balanced | 0.164 | 0.163 | quadratic-context | T(n) = 20883 n^1.474 |
| Tura Direct | 0.198 | 0.201 | quadratic-context | T(n) = 23769 n^1.397 |
| Codex CLI Medium | 0.062 | 0.068 | quadratic-context | T(n) = 40928 n^1.240 |
| Codex CLI High | 0.266 | 0.264 | quadratic-context | T(n) = 12111 n^1.382 |

**Conclusion:** Quadratic-context form retained for: Tura Balanced, Tura Direct, Codex CLI Medium, Codex CLI High. Power-law alternative preferred for: none.

The result is an empirical cross-task relationship, not a claim that extra rounds cause success or token growth identically for every task. Task difficulty and model configuration remain visible as run-level scatter.

## Contract audit

- Token totals cross-checked against all 209 round contracts; maximum difference: 0 tokens.
- Costs cross-checked against 239 populated task contracts; maximum difference: $0.00000000.
- Excluded duplicate aggregate-usage snapshots: 5 rounds in 1 run; these rounds remain in the round count.
- Excluded exact run-aggregate usage snapshots: 1 round; it remains in the round count.
- The remaining historical cost fields were absent, not zero; they were recomputed from their recorded token components with the same benchmark pricing rule.
