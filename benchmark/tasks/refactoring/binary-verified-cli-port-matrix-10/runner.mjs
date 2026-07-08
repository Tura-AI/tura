#!/usr/bin/env node
import path from "node:path"
import { fileURLToPath } from "node:url"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))

process.env.SOURCE_PORT_TASK_CONFIG ||= path.join(scriptDir, "tasks.json")
process.env.COMMAND_RUN_AGENT_SOURCE_PORT_TASK_CONFIG ||= process.env.SOURCE_PORT_TASK_CONFIG
process.env.SOURCE_PORT_TASKS ||= "eza,ripgrep,fzf,yq,prettier,typescript,black,pyflakes,checkstyle,google-java-format"
process.env.COMMAND_RUN_AGENT_SOURCE_PORT_TASKS ||= process.env.SOURCE_PORT_TASKS
process.env.SOURCE_PORT_COMPLEX_TODO_HINT ||= "0"
process.env.COMMAND_RUN_AGENT_SOURCE_PORT_COMPLEX_TODO_HINT ||= "0"

// Reuse the existing shared harness engine while keeping this matrix's public
// naming separate from the older single-target suite.
const sharedHarnessDir = ["source", "port", "python"].join("-")
await import(`../${sharedHarnessDir}/runner.mjs`)
