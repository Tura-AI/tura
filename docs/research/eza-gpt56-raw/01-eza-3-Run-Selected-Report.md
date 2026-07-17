# eza 3-Run Selected Report

Source: https://raw.githubusercontent.com/Tura-AI/benchmark/main/blog_data/eza-replication-gpt56-max-20260717/eza-3run-selected-report.md

# eza 3-Run Selected Report

Date: 2026-07-17

Task: `source-port-python-default-eza`

Model: `gpt-5.6-sol`

Reasoning effort: `max`

Output root: `blog_data/eza-replication-gpt56-max-20260717`

This report intentionally keeps only the three runs selected by the user:

- `balanced` run `r-29a056ef1e`
- `direct` run `r-245ba19733`
- `codex-cli` run `r-d3252a51d0`

## Results

| Agent | Run | Harness | Rounds | Total tokens | Input tokens | Cached input | Output tokens | Reasoning tokens |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced | r-29a056ef1e | 49 / 3 | 72 | 10,089,898 | 9,871,477 | 9,192,192 | 218,421 | 113,103 |
| direct | r-245ba19733 | 48 / 4 | 18 | 2,708,614 | 2,593,963 | 2,255,616 | 114,651 | 81,938 |
| codex-cli | r-d3252a51d0 | 48 / 4 | 92 | 15,263,447 | 15,194,251 | 14,673,920 | 69,196 | 30,230 |

## Aggregate

| Metric | Value |
| --- | ---: |
| Runs | 3 |
| Harness passed | 145 |
| Harness failed | 11 |
| Harness pass rate | 92.95% |
| Rounds | 182 |
| Total tokens | 28,061,959 |
| Input tokens | 27,659,691 |
| Cached input tokens | 26,121,728 |
| Output tokens | 402,268 |
| Reasoning tokens | 225,271 |

## Notes

- These are the selected comparable runs only; other completed runs and failed direct attempts are excluded from this report.
- The Tura top-level publish assertion seen earlier was a reporting/round-counting bug: Tura rounds were present in provider logs and agent summaries, but the publisher checked `events.llm_rounds`, which was not populated for Tura.
- The selected `direct` run here is `r-245ba19733`, which completed as an agent run and produced a harness result of `48 / 4`.
