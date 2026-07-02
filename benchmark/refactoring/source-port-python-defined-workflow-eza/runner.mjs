#!/usr/bin/env node
process.env.SOURCE_PORT_TASKS = "eza"
process.env.COMMAND_RUN_AGENT_SOURCE_PORT_TASKS = "eza"
await import("../source-port-python/defined-workflow.runner.mjs")
