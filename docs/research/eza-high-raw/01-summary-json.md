# summary.json

Source: https://raw.githubusercontent.com/Tura-AI/benchmark/main/blog_data/token-saving-plugin-eza/summary.json

{
  "schema": "tura.token-saving-plugin-eza-summary.v1",
  "task": "source-port-python-default-eza",
  "model": "gpt-5.6-sol",
  "reasoning": "high",
  "codex_cli": "0.144.1",
  "pricing_usd_per_million_tokens": {
    "uncached_input": 5.0,
    "cached_input": 0.5,
    "output": 30.0
  },
  "aggregates": {
    "ponytail": {
      "n": 2,
      "mean_score": 0.8076923076923077,
      "mean_passed": 42,
      "mean_total_tokens": 6156827,
      "mean_cost_usd": 4.8133665,
      "mean_rounds": 56.5,
      "mean_duration_ms": 1016207.5,
      "cached_share_of_tokens_percent": 96.73762150536307
    },
    "rtk": {
      "n": 2,
      "mean_score": 0.7692307692307692,
      "mean_passed": 40,
      "mean_total_tokens": 7539176.5,
      "mean_cost_usd": 5.6611405,
      "mean_rounds": 90,
      "mean_duration_ms": 1259550,
      "cached_share_of_tokens_percent": 97.27025226163627
    },
    "no-plugin": {
      "n": 2,
      "mean_score": 0.7884615384615385,
      "mean_passed": 41,
      "mean_total_tokens": 6660286,
      "mean_cost_usd": 5.281946,
      "mean_rounds": 62.5,
      "mean_duration_ms": 895268,
      "cached_share_of_tokens_percent": 96.7819099660285
    }
  },
  "deltas_vs_no_plugin_high": {
    "ponytail": {
      "score_percentage_points": 1.9230769230769162,
      "total_tokens_percent": -7.559119833592732,
      "cost_percent": -8.871342115197688,
      "rounds_percent": -9.6,
      "duration_percent": 13.508748218410577
    },
    "rtk": {
      "score_percentage_points": -1.9230769230769384,
      "total_tokens_percent": 13.19598737952094,
      "cost_percent": 7.179068093464049,
      "rounds_percent": 44.0,
      "duration_percent": 40.68971525844775
    }
  },
  "within_arm_variation": {
    "ponytail": {
      "total_tokens": {
        "min": 4391123,
        "max": 7922531,
        "max_to_min_ratio": 1.804215231502283,
        "range_percent_of_mean": 57.357596697129864
      },
      "cost_usd": {
        "min": 3.569452,
        "max": 6.057281,
        "max_to_min_ratio": 1.696977855424306,
        "range_percent_of_mean": 51.68584191542447
      },
      "rounds": {
        "min": 43,
        "max": 70,
        "max_to_min_ratio": 1.627906976744186,
        "range_percent_of_mean": 47.78761061946903
      }
    },
    "rtk": {
      "total_tokens": {
        "min": 6040895,
        "max": 9037458,
        "max_to_min_ratio": 1.4960461984523816,
        "range_percent_of_mean": 39.746555873841125
      },
      "cost_usd": {
        "min": 4.789893,
        "max": 6.532388,
        "max_to_min_ratio": 1.36378578811677,
        "range_percent_of_mean": 30.779928532068755
      },
      "rounds": {
        "min": 78,
        "max": 102,
        "max_to_min_ratio": 1.3076923076923077,
        "range_percent_of_mean": 26.666666666666668
      }
    },
    "no-plugin": {
      "total_tokens": {
        "min": 4894647,
        "max": 8425925,
        "max_to_min_ratio": 1.7214571347024616,
        "range_percent_of_mean": 53.01991536099201
      },
      "cost_usd": {
        "min": 4.139647,
        "max": 6.424245,
        "max_to_min_ratio": 1.551882322333281,
        "range_percent_of_mean": 43.252960177934426
      },
      "rounds": {
        "min": 50,
        "max": 75,
        "max_to_min_ratio": 1.5,
        "range_percent_of_mean": 40.0
      }
    }
  },
  "limitations": [
    "Each plugin arm and the matched baseline have n=2.",
    "Ponytail and RTK use matched replicate indices r2 and r3.",
    "Results are descriptive and task-specific, not statistical significance claims."
  ]
}
