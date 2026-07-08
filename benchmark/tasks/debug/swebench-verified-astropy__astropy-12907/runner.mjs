#!/usr/bin/env node
process.env.COMMAND_RUN_AGENT_TASKS ||= JSON.stringify(["astropy__astropy-12907"])
process.env.COMMAND_RUN_AGENT_BENCHMARK_TASK_NAME ||= "swebench-verified-astropy__astropy-12907"
process.env.COMMAND_RUN_AGENT_SWEBENCH_DATASET ||= "SWE-bench/SWE-bench_Verified"
process.env.COMMAND_RUN_AGENT_SWEBENCH_PREFIX ||= "swebench-verified"
process.env.COMMAND_RUN_AGENT_SWEBENCH_REPOS ||= "astropy/astropy"
await import("../swebench-verified-issue-patch/runner.mjs")
