#!/usr/bin/env node
process.env.SOURCE_PORT_TASKS = "nushell"
process.env.COMMAND_RUN_AGENT_SOURCE_PORT_TASKS = "nushell"
await import("../source-port-python/composite.runner.mjs")
