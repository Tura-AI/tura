# diagnostics.json

Source: https://raw.githubusercontent.com/Tura-AI/benchmark/main/assets/model-run-statistics/diagnostics.json

{
  "source_audit": {
    "run_count": 280,
    "task_count": 25,
    "token_contracts_checked": 209,
    "cost_contracts_checked": 239,
    "max_token_difference": 0,
    "max_cost_difference_usd": 0.0,
    "usage_available_runs": 279,
    "usage_unavailable_runs": 1,
    "aggregate_only_usage_runs": 70,
    "aggregate_snapshot_usage_rounds": 1,
    "excluded_duplicate_usage_rounds": 5,
    "runs_with_excluded_duplicate_usage": 1,
    "coverage": {
      "tura-balanced": {
        "runs": 70,
        "tasks": 25,
        "models": [
          "gpt-5.6-sol"
        ],
        "rounds": 2246,
        "tokens": 254693404,
        "passed": 437,
        "checks": 532,
        "cost_usd": 256.747125
      },
      "tura-direct": {
        "runs": 70,
        "tasks": 25,
        "models": [
          "gpt-5.6-sol"
        ],
        "rounds": 1092,
        "tokens": 83476806,
        "passed": 392,
        "checks": 532,
        "cost_usd": 117.425295
      },
      "codex-cli-medium": {
        "runs": 70,
        "tasks": 25,
        "models": [
          "gpt-5.6-sol"
        ],
        "rounds": 3565,
        "tokens": 382517759,
        "passed": 389,
        "checks": 532,
        "cost_usd": 300.831216
      },
      "codex-cli-high": {
        "runs": 70,
        "tasks": 25,
        "models": [
          "gpt-5.6-sol"
        ],
        "rounds": 6800,
        "tokens": 519090772,
        "passed": 388,
        "checks": 532,
        "cost_usd": 379.513505
      }
    }
  },
  "analysis_sample": {
    "run_count": 278,
    "task_count": 25,
    "passed": 1604,
    "checks": 2126,
    "total_tokens": 1188299949,
    "excluded_run_count": 2,
    "excluded_runs": [
      {
        "run_id": "dynamodb-toolbox-conditional-attribute-requirements-tura-balanced-run-01",
        "agent_group": "tura-balanced",
        "rounds": 242,
        "total_tokens": 35464917,
        "reason": "tura-balanced-rounds-over-90-long-tail"
      },
      {
        "run_id": "quill-shared-toolbar-focus-tura-balanced-run-01",
        "agent_group": "tura-balanced",
        "rounds": 113,
        "total_tokens": 16013875,
        "reason": "tura-balanced-rounds-over-90-long-tail"
      }
    ],
    "coverage": {
      "tura-balanced": {
        "runs": 68,
        "tasks": 25,
        "min_rounds": 1,
        "max_rounds": 62
      },
      "tura-direct": {
        "runs": 70,
        "tasks": 25,
        "min_rounds": 4,
        "max_rounds": 35
      },
      "codex-cli-medium": {
        "runs": 70,
        "tasks": 25,
        "min_rounds": 22,
        "max_rounds": 94
      },
      "codex-cli-high": {
        "runs": 70,
        "tasks": 25,
        "min_rounds": 23,
        "max_rounds": 219
      }
    }
  },
  "pricing_usd_per_1m_tokens": {
    "uncached_input": 5.0,
    "cached_input": 0.5,
    "output": 30.0
  },
  "token_models": {
    "tura-balanced": {
      "quadratic_context": {
        "formula": "T(n) = nB + c*n*(n+1)/2",
        "B_tokens": 50643.81448240852,
        "c_tokens_per_round": 3376.7501672916537,
        "metrics": {
          "r_squared": 0.9101793678034672,
          "rmsle": 0.15692632840804327,
          "mape": 0.12436258418153953
        },
        "leave_one_task_out_rmsle": 0.16418352689567556,
        "multiplicative_error": 0.1784305689520045
      },
      "power_law": {
        "formula": "T(n) = a*n^p",
        "a_tokens": 20883.416391924617,
        "p": 1.4735317565044899,
        "metrics": {
          "r_squared": 0.9195795967321079,
          "rmsle": 0.15582758019589904,
          "mape": 0.12320935425831187
        },
        "leave_one_task_out_rmsle": 0.16330297785471826,
        "multiplicative_error": 0.17739335976847292
      },
      "selected_model": "quadratic-context",
      "formula_conforms": true
    },
    "tura-direct": {
      "quadratic_context": {
        "formula": "T(n) = nB + c*n*(n+1)/2",
        "B_tokens": 36084.73403791747,
        "c_tokens_per_round": 3973.2452539256915,
        "metrics": {
          "r_squared": 0.8791270431936274,
          "rmsle": 0.18997953304261675,
          "mape": 0.14876801171633586
        },
        "leave_one_task_out_rmsle": 0.1978837650849382,
        "multiplicative_error": 0.21882071606544384
      },
      "power_law": {
        "formula": "T(n) = a*n^p",
        "a_tokens": 23768.842953833453,
        "p": 1.3965920476747156,
        "metrics": {
          "r_squared": 0.8778276447558757,
          "rmsle": 0.19316482856325246,
          "mape": 0.1508692141934116
        },
        "leave_one_task_out_rmsle": 0.20112865727574308,
        "multiplicative_error": 0.2227820815150272
      },
      "selected_model": "quadratic-context",
      "formula_conforms": true
    },
    "codex-cli-medium": {
      "quadratic_context": {
        "formula": "T(n) = nB + c*n*(n+1)/2",
        "B_tokens": 76960.10657090781,
        "c_tokens_per_round": 1036.014524882173,
        "metrics": {
          "r_squared": 0.9665022100787758,
          "rmsle": 0.057287603302805315,
          "mape": 0.043081240241324724
        },
        "leave_one_task_out_rmsle": 0.06197129544281417,
        "multiplicative_error": 0.06393180459838832
      },
      "power_law": {
        "formula": "T(n) = a*n^p",
        "a_tokens": 40927.68297060004,
        "p": 1.2395099093158863,
        "metrics": {
          "r_squared": 0.9573933169599598,
          "rmsle": 0.0622938881127206,
          "mape": 0.04642465083574953
        },
        "leave_one_task_out_rmsle": 0.06753272178552583,
        "multiplicative_error": 0.06986526692695372
      },
      "selected_model": "quadratic-context",
      "formula_conforms": true
    },
    "codex-cli-high": {
      "quadratic_context": {
        "formula": "T(n) = nB + c*n*(n+1)/2",
        "B_tokens": 39987.87878052768,
        "c_tokens_per_round": 566.8378622799074,
        "metrics": {
          "r_squared": 0.7975869802447538,
          "rmsle": 0.2503169048890996,
          "mape": 0.19331269084730698
        },
        "leave_one_task_out_rmsle": 0.265897807985168,
        "multiplicative_error": 0.30460173199503626
      },
      "power_law": {
        "formula": "T(n) = a*n^p",
        "a_tokens": 12110.938272740652,
        "p": 1.3818475491594147,
        "metrics": {
          "r_squared": 0.7926137782263128,
          "rmsle": 0.24914534322336337,
          "mape": 0.19332370582149572
        },
        "leave_one_task_out_rmsle": 0.26424024883680586,
        "multiplicative_error": 0.3024410686675336
      },
      "selected_model": "quadratic-context",
      "formula_conforms": true
    }
  },
  "success_models": {
    "tura-balanced": {
      "formula": "logit(P(success)) = alpha + beta*log(1+n)",
      "alpha": -3.7327362614386606,
      "beta": 1.690597296512052,
      "weighted_success_rate": 0.8207547169811321,
      "checks": 530
    },
    "tura-direct": {
      "formula": "logit(P(success)) = alpha + beta*log(1+n)",
      "alpha": -3.201171927171383,
      "beta": 1.692814928100976,
      "weighted_success_rate": 0.7368421052631579,
      "checks": 532
    },
    "codex-cli-medium": {
      "formula": "logit(P(success)) = alpha + beta*log(1+n)",
      "alpha": -2.990322293830718,
      "beta": 1.0829746132418065,
      "weighted_success_rate": 0.731203007518797,
      "checks": 532
    },
    "codex-cli-high": {
      "formula": "logit(P(success)) = alpha + beta*log(1+n)",
      "alpha": 1.2549340975507066,
      "beta": -0.060903448198622215,
      "weighted_success_rate": 0.7293233082706767,
      "checks": 532
    }
  },
  "cost_models": {
    "tura-balanced": {
      "formula": "C(n) = a*n^p",
      "a_usd": 0.1403292029731604,
      "p": 0.943758204624425,
      "metrics": {
        "r_squared": 0.7745258241175871,
        "rmsle": 0.17780398312788118,
        "mape": 0.11996900730252458
      }
    },
    "tura-direct": {
      "formula": "C(n) = a*n^p",
      "a_usd": 0.14947251799008582,
      "p": 0.8764278138665357,
      "metrics": {
        "r_squared": 0.7502334859213708,
        "rmsle": 0.23451974767908287,
        "mape": 0.16475998939031417
      }
    },
    "codex-cli-medium": {
      "formula": "C(n) = a*n^p",
      "a_usd": 0.10230383772631722,
      "p": 0.9489054426663828,
      "metrics": {
        "r_squared": 0.861149074436241,
        "rmsle": 0.11592106169732674,
        "mape": 0.08751278363222662
      }
    },
    "codex-cli-high": {
      "formula": "C(n) = a*n^p",
      "a_usd": 0.04262233549619312,
      "p": 1.0495674413468474,
      "metrics": {
        "r_squared": 0.7539763760504438,
        "rmsle": 0.23317861940288565,
        "mape": 0.18314358011642853
      }
    }
  }
}
