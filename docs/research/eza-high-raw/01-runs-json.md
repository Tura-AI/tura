# runs.json

Source: https://raw.githubusercontent.com/Tura-AI/benchmark/main/blog_data/token-saving-plugin-eza/runs.json

{
  "schema": "tura.token-saving-plugin-eza-runs.v1",
  "task": "source-port-python-default-eza",
  "model": "gpt-5.6-sol",
  "reasoning": "high",
  "codex_cli": "0.144.1",
  "runs": [
    {
      "run": "ponytail-r2",
      "arm": "ponytail",
      "activation_mode": "hook+skill",
      "exclusive_activation": true,
      "codex_exit_code": 0,
      "harness": {
        "passed": 43,
        "failed": 9,
        "total": 52,
        "score": 0.8269230769230769
      },
      "duration_ms": 1287058,
      "usage": {
        "input_tokens": 7879489,
        "cached_input_tokens": 7695872,
        "output_tokens": 43042,
        "reasoning_tokens": 15990,
        "total_tokens": 7922531
      },
      "llm_rounds": 70,
      "commands": { "started": 43, "completed": 37, "failed": 6 }
    },
    {
      "run": "ponytail-r3",
      "arm": "ponytail",
      "activation_mode": "hook+skill",
      "exclusive_activation": true,
      "codex_exit_code": 0,
      "harness": {
        "passed": 41,
        "failed": 11,
        "total": 52,
        "score": 0.7884615384615384
      },
      "duration_ms": 745357,
      "usage": {
        "input_tokens": 4367678,
        "cached_input_tokens": 4216064,
        "output_tokens": 23445,
        "reasoning_tokens": 5752,
        "total_tokens": 4391123
      },
      "llm_rounds": 43,
      "commands": { "started": 33, "completed": 29, "failed": 4 }
    },
    {
      "run": "rtk-r2",
      "arm": "rtk",
      "activation_mode": "global-agents-expanded",
      "exclusive_activation": true,
      "codex_exit_code": 0,
      "harness": {
        "passed": 38,
        "failed": 14,
        "total": 52,
        "score": 0.7307692307692307
      },
      "duration_ms": 1145765,
      "usage": {
        "input_tokens": 6003813,
        "cached_input_tokens": 5853696,
        "output_tokens": 37082,
        "reasoning_tokens": 9447,
        "total_tokens": 6040895
      },
      "llm_rounds": 78,
      "commands": { "started": 71, "completed": 64, "failed": 6 }
    },
    {
      "run": "rtk-r3",
      "arm": "rtk",
      "activation_mode": "global-agents-expanded",
      "exclusive_activation": true,
      "codex_exit_code": 0,
      "harness": {
        "passed": 42,
        "failed": 10,
        "total": 52,
        "score": 0.8076923076923077
      },
      "duration_ms": 1373335,
      "usage": {
        "input_tokens": 8997304,
        "cached_input_tokens": 8813056,
        "output_tokens": 40154,
        "reasoning_tokens": 11928,
        "total_tokens": 9037458
      },
      "llm_rounds": 102,
      "commands": { "started": 123, "completed": 119, "failed": 4 }
    },
    {
      "run": "no-plugin-high-r1",
      "arm": "no-plugin",
      "activation_mode": "none",
      "exclusive_activation": true,
      "codex_exit_code": 0,
      "harness": {
        "passed": 42,
        "failed": 10,
        "total": 52,
        "score": 0.8076923076923077
      },
      "duration_ms": 738521,
      "usage": {
        "input_tokens": 4860395,
        "cached_input_tokens": 4708864,
        "output_tokens": 34252,
        "reasoning_tokens": 9854,
        "total_tokens": 4894647
      },
      "llm_rounds": 50,
      "commands": { "started": 35, "completed": 32, "failed": 3 }
    },
    {
      "run": "no-plugin-high-r2",
      "arm": "no-plugin",
      "activation_mode": "none",
      "exclusive_activation": true,
      "codex_exit_code": 0,
      "harness": {
        "passed": 40,
        "failed": 12,
        "total": 52,
        "score": 0.7692307692307693
      },
      "duration_ms": 1052015,
      "usage": {
        "input_tokens": 8381193,
        "cached_input_tokens": 8183040,
        "output_tokens": 44732,
        "reasoning_tokens": 15946,
        "total_tokens": 8425925
      },
      "llm_rounds": 75,
      "commands": { "started": 53, "completed": 49, "failed": 4 }
    }
  ]
}
