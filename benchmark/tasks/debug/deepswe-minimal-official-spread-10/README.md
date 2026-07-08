# DeepSWE Minimal Official Spread 10

This task set samples 10 DeepSWE v1.1 tasks across the official included-trial pass-rate distribution.

Selection method:
- Use `deep-swe-site-artifacts/trials-v1.1.json`.
- Keep rows with `included_in_score=true`.
- Group by `task_name` and average `score_value`.
- Sort by pass rate and select 10 approximately even rank quantiles.

Selected tasks:

| Bucket | Task | Official pass rate |
| --- | --- | ---: |
| 1 | `obsidian-linter-auto-table-of-contents` | 1.0% |
| 2 | `pest-character-class-coalescing` | 19.2% |
| 3 | `sqlfmt-create-table-ddl-formatting` | 27.2% |
| 4 | `prometheus-typed-label-sorting` | 34.6% |
| 5 | `mnamer-daemon-watch-lifecycle` | 44.7% |
| 6 | `tengo-callable-instance-isolation` | 54.8% |
| 7 | `textual-richlog-follow-state` | 61.5% |
| 8 | `anko-default-function-arguments` | 68.3% |
| 9 | `actionlint-action-pinning-lint` | 78.8% |
| 10 | `narwhals-rolling-window-suite` | 93.3% |

The wrapper sets `COMMAND_RUN_AGENT_DEEPSWE_MINIMAL_PROMPTS` so the shared DeepSWE runner uses the compact prompt in `minimal-prompts.json` instead of the original long task instructions.
