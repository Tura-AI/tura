#!/usr/bin/env node
process.env.COMMAND_RUN_AGENT_BENCHMARK_TASK_NAME ||= "deepswe-minimal-official-spread-10"
process.env.COMMAND_RUN_AGENT_DEEPSWE_TASKS ||= [
  "obsidian-linter-auto-table-of-contents",
  "pest-character-class-coalescing",
  "sqlfmt-create-table-ddl-formatting",
  "prometheus-typed-label-sorting",
  "mnamer-daemon-watch-lifecycle",
  "tengo-callable-instance-isolation",
  "textual-richlog-follow-state",
  "anko-default-function-arguments",
  "actionlint-action-pinning-lint",
  "narwhals-rolling-window-suite"
].join(",")
process.env.COMMAND_RUN_AGENT_DEEPSWE_MINIMAL_PROMPTS ||= "benchmark/tasks/debug/deepswe-minimal-official-spread-10/minimal-prompts.json"
await import("../deepswe-anko-default-arguments/runner.mjs")
