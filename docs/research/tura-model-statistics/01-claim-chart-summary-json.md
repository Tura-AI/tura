# claim-chart-summary.json

Source: https://raw.githubusercontent.com/Tura-AI/benchmark/main/assets/model-run-statistics/claim-charts/claim-chart-summary.json

{
  "sample": {
    "runs": 278,
    "tasks": 25,
    "source": "assets/model-run-statistics/run-level-data.csv"
  },
  "agent_summary": {
    "tura-balanced": {
      "runs": 68,
      "mean_rounds": 27.808823529411764,
      "median_rounds": 27.0,
      "mean_cost_usd": 3.2805868823529405,
      "success_rate": 0.8207547169811321,
      "checks": 530
    },
    "tura-direct": {
      "runs": 70,
      "mean_rounds": 15.6,
      "median_rounds": 15.0,
      "mean_cost_usd": 1.6775042142857142,
      "success_rate": 0.7368421052631579,
      "checks": 532
    },
    "codex-cli-medium": {
      "runs": 70,
      "mean_rounds": 50.92857142857143,
      "median_rounds": 50.5,
      "mean_cost_usd": 4.297588799999999,
      "success_rate": 0.731203007518797,
      "checks": 532
    },
    "codex-cli-high": {
      "runs": 70,
      "mean_rounds": 97.14285714285714,
      "median_rounds": 83.0,
      "mean_cost_usd": 5.4216215,
      "success_rate": 0.7293233082706767,
      "checks": 532
    }
  },
  "round_command_association": {
    "tura-balanced": {
      "mean_commands_per_round": 5.612374405076679,
      "median_commands_per_round": 5.751105216622458,
      "linear_slope": 4.116492387360794,
      "linear_intercept": 41.59871905148146
    },
    "tura-direct": {
      "mean_commands_per_round": 4.572344322344322,
      "median_commands_per_round": 4.625,
      "linear_slope": 3.9858977436389833,
      "linear_intercept": 9.148566627803296
    },
    "codex-cli-medium": {
      "mean_commands_per_round": 0.8779803646563815,
      "median_commands_per_round": 0.8125,
      "linear_slope": 0.854543895453532,
      "linear_intercept": 1.1935858958308307
    },
    "codex-cli-high": {
      "mean_commands_per_round": 0.9904411764705883,
      "median_commands_per_round": 0.989010989010989,
      "linear_slope": 0.9994933606682934,
      "linear_intercept": -0.8793550363483924
    }
  },
  "round_success_association": {
    "tura-balanced": {
      "formula": "logit(P(success)) = alpha + beta*log(1+rounds)",
      "alpha": -3.7327339650219318,
      "beta": 1.690596574542828,
      "beta_standard_error": 0.3465442382160952,
      "beta_p_value_naive": 1.0692614557219765e-06,
      "q1": 19.75,
      "q3": 32.0,
      "q1_success_percent": 80.12425274882185,
      "q3_success_percent": 89.82975293186243,
      "interquartile_fitted_probability_change_percentage_points": 9.705500183040582,
      "interquartile_fitted_probability_change_95_ci_percentage_points_model_based": [
        6.392514784790496,
        13.01848558129067
      ],
      "observed_min": 1.0,
      "observed_max": 62.0
    },
    "tura-direct": {
      "formula": "logit(P(success)) = alpha + beta*log(1+rounds)",
      "alpha": -3.2011726519126444,
      "beta": 1.6928151983731081,
      "beta_standard_error": 0.30343774925783346,
      "beta_p_value_naive": 2.4219872066761863e-08,
      "q1": 11.0,
      "q3": 19.75,
      "q1_success_percent": 73.21023047683937,
      "q3_success_percent": 87.35112596042649,
      "interquartile_fitted_probability_change_percentage_points": 14.140895483587123,
      "interquartile_fitted_probability_change_95_ci_percentage_points_model_based": [
        10.36143767003695,
        17.920353297137297
      ],
      "observed_min": 4.0,
      "observed_max": 35.0
    },
    "codex-cli-medium": {
      "formula": "logit(P(success)) = alpha + beta*log(1+rounds)",
      "alpha": -2.9903232771471937,
      "beta": 1.0829748766722223,
      "beta_standard_error": 0.23603486772531912,
      "beta_p_value_naive": 4.470869643060121e-06,
      "q1": 38.25,
      "q3": 61.0,
      "q1_success_percent": 72.79292933563941,
      "q3_success_percent": 81.44607177613497,
      "interquartile_fitted_probability_change_percentage_points": 8.653142440495554,
      "interquartile_fitted_probability_change_95_ci_percentage_points_model_based": [
        5.41092903406949,
        11.895355846921618
      ],
      "observed_min": 22.0,
      "observed_max": 94.0
    },
    "codex-cli-high": {
      "formula": "logit(P(success)) = alpha + beta*log(1+rounds)",
      "alpha": 1.2549333655150148,
      "beta": -0.060903292788590864,
      "beta_standard_error": 0.20093280796459165,
      "beta_p_value_naive": 0.7618115352329193,
      "q1": 64.5,
      "q3": 123.75,
      "q1_success_percent": 73.1104335870557,
      "q3_success_percent": 72.33209942491595,
      "interquartile_fitted_probability_change_percentage_points": -0.7783341621397377,
      "interquartile_fitted_probability_change_95_ci_percentage_points_model_based": [
        -5.835369693833135,
        4.2787013695536595
      ],
      "observed_min": 23.0,
      "observed_max": 219.0
    }
  },
  "token_cost_composition": {
    "tura-balanced": {
      "token_share_percent": {
        "uncached_input": 5.611007932835066,
        "cached_input": 93.21861953509524,
        "output": 1.1703725320696918
      },
      "cost_share_percent": {
        "uncached_input": 25.55673458499006,
        "cached_input": 42.458744424441846,
        "output": 31.9845209905681
      }
    },
    "tura-direct": {
      "token_share_percent": {
        "uncached_input": 8.577900069631319,
        "cached_input": 89.65709588840761,
        "output": 1.7650040419610689
      },
      "cost_share_percent": {
        "uncached_input": 30.48984037042445,
        "cached_input": 31.868295498001515,
        "output": 37.64186413157404
      }
    },
    "codex-cli-medium": {
      "token_share_percent": {
        "uncached_input": 4.166650469161616,
        "cached_input": 95.49792222849449,
        "output": 0.33542730234388934
      },
      "cost_share_percent": {
        "uncached_input": 26.49023298167301,
        "cached_input": 60.71452904009801,
        "output": 12.79523797822896
      }
    },
    "codex-cli-high": {
      "token_share_percent": {
        "uncached_input": 2.4192853499618754,
        "cached_input": 97.16632758788477,
        "output": 0.41438706215336074
      },
      "cost_share_percent": {
        "uncached_input": 16.545243890596197,
        "cached_input": 66.45105290785371,
        "output": 17.0037032015501
      }
    }
  },
  "command_success_association": {
    "tura-balanced": {
      "formula": "logit(P(success)) = alpha + beta*log(1+commands)",
      "alpha": -5.558292817096832,
      "beta": 1.4544488810292306,
      "beta_standard_error": 0.3552625114882603,
      "beta_p_value_naive": 4.239728898654677e-05,
      "q1": 121.75,
      "q3": 181.25,
      "q1_success_percent": 80.81174791301329,
      "q3_success_percent": 88.2120461785248,
      "interquartile_fitted_probability_change_percentage_points": 7.400298265511507,
      "interquartile_fitted_probability_change_95_ci_percentage_points_model_based": [
        4.290632983122979,
        10.509963547900036
      ],
      "observed_min": 79.0,
      "observed_max": 349.0
    },
    "tura-direct": {
      "formula": "logit(P(success)) = alpha + beta*log(1+commands)",
      "alpha": -6.178707889773184,
      "beta": 1.7713119479462267,
      "beta_standard_error": 0.2848378795288372,
      "beta_p_value_naive": 5.013958240322011e-10,
      "q1": 51.25,
      "q3": 88.5,
      "q1_success_percent": 69.60742347482277,
      "q3_success_percent": 85.59425010542697,
      "interquartile_fitted_probability_change_percentage_points": 15.986826630604211,
      "interquartile_fitted_probability_change_95_ci_percentage_points_model_based": [
        11.675456082055783,
        20.29819717915264
      ],
      "observed_min": 24.0,
      "observed_max": 161.0
    }
  },
  "command_count_sources": {
    "agent-rounds.commands[]": 40,
    "summary.events.commands": 238
  },
  "command_count_comparability": {
    "included": [
      "tura-balanced",
      "tura-direct"
    ],
    "excluded": [
      "codex-cli-medium",
      "codex-cli-high"
    ],
    "reason": "A Codex shell call may wrap multiple shell commands; its unit is not comparable with normalized Tura command records."
  },
  "scaling": {
    "tura-balanced": {
      "total_tokens": {
        "scale": 20883.416391924617,
        "exponent": 1.4735317565044899
      },
      "cost_usd": {
        "scale": 0.1403292029731604,
        "exponent": 0.943758204624425
      },
      "effective_rate_power_exponent": -0.5297735518800649
    },
    "tura-direct": {
      "total_tokens": {
        "scale": 23768.842953833453,
        "exponent": 1.3965920476747156
      },
      "cost_usd": {
        "scale": 0.14947251799008582,
        "exponent": 0.8764278138665357
      },
      "effective_rate_power_exponent": -0.5201642338081799
    },
    "codex-cli-medium": {
      "total_tokens": {
        "scale": 40927.68297060004,
        "exponent": 1.2395099093158863
      },
      "cost_usd": {
        "scale": 0.10230383772631722,
        "exponent": 0.9489054426663828
      },
      "effective_rate_power_exponent": -0.29060446664950357
    },
    "codex-cli-high": {
      "total_tokens": {
        "scale": 12110.938272740652,
        "exponent": 1.3818475491594147
      },
      "cost_usd": {
        "scale": 0.04262233549619312,
        "exponent": 1.0495674413468474
      },
      "effective_rate_power_exponent": -0.3322801078125672
    }
  },
  "pricing_usd_per_1m_tokens": {
    "uncached_input": 5.0,
    "cached_input": 0.5,
    "output": 30.0
  }
}
