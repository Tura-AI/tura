#!/usr/bin/env node
process.env.SOURCE_PORT_TASKS = "zip-password-finder"
process.env.COMMAND_RUN_AGENT_SOURCE_PORT_TASKS = "zip-password-finder"
await import("../source-port-python/composite.runner.mjs")
