#!/usr/bin/env node
process.env.SOURCE_PORT_TASKS ||= "xsv"
await import("../source-port-rewrite-benchmarks/source_port_rewrite_suite.mjs")
