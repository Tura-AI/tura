#!/usr/bin/env node
process.env.SOURCE_PORT_TASKS = "xsv"
process.env.COMMAND_RUN_AGENT_SOURCE_PORT_TASKS = "xsv"
await import("../source-port-python/defined-workflow.runner.mjs")
