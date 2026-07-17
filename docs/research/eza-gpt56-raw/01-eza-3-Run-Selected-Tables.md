# eza 3-Run Selected Tables

Source: https://raw.githubusercontent.com/Tura-AI/benchmark/main/blog_data/eza-replication-gpt56-max-20260717/eza-3run-selected-tables.md

Title: 

URL Source: https://raw.githubusercontent.com/Tura-AI/benchmark/main/blog_data/eza-replication-gpt56-max-20260717/eza-3run-selected-tables.md

Markdown Content:
# eza 3-Run Selected Tables

## Cost

| Agent | Run | Cost | Input cost | Cache cost | Output cost | Cost / passed |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| balanced | r-29a056ef1e | $14.55 | $3.40 | $4.60 | $6.55 | $0.297 |
| direct | r-245ba19733 | $6.26 | $1.69 | $1.13 | $3.44 | $0.130 |
| codex-cli | r-d3252a51d0 | $12.01 | $2.60 | $7.34 | $2.08 | $0.250 |

## Harness And Tokens

| Agent | Run/status | Harness | Rounds | Total tok | Input tok | Cached | Output | Reasoning |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced | r-29a056ef1e | 49 / 3 | 72 | 10,089,898 | 9,871,477 | 9,192,192 | 218,421 | 113,103 |
| direct | r-245ba19733 | 48 / 4 | 18 | 2,708,614 | 2,593,963 | 2,255,616 | 114,651 | 81,938 |
| codex-cli | r-d3252a51d0 | 48 / 4 | 92 | 15,263,447 | 15,194,251 | 14,673,920 | 69,196 | 30,230 |
