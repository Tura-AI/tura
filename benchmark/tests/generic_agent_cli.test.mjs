import assert from "node:assert/strict"
import fs from "node:fs"
import os from "node:os"
import path from "node:path"
import process from "node:process"
import test from "node:test"

import { buildCodexTokenFixtureStdout, codexHomeForAgent, eventsForAgent, eventsWithUsageRounds, parseGenericAgents, prepareCodexCliHomeForAgent, runCodexCliSetupCommands, runLive, seedCodexPluginCache, usageForAgent } from "../lib/generic_agent_cli.mjs"

function tempAgentDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "tura-generic-agent-cli-"))
}

test("generic agent aliases include codex, ponytail, pi, and opencode", () => {
  assert.deepEqual(parseGenericAgents("codex,ponytail,codex-ponytail,pi,pi-agent,opencode,open-code"), [
    "codex-main",
    "codex-main-ponytail",
    "codex-main-ponytail",
    "pi-agent",
    "pi-agent",
    "opencode",
    "opencode",
  ])
})

test("codex home prep is isolated and plugin cache seeding is explicit", () => {
  const sourceHome = tempAgentDir()
  const targetHome = tempAgentDir()
  fs.writeFileSync(path.join(sourceHome, "auth.json"), "{}\n", "utf8")
  const pluginSource = path.join(sourceHome, "plugin-source", "github")
  fs.mkdirSync(path.join(pluginSource, ".codex-plugin"), { recursive: true })
  fs.writeFileSync(path.join(pluginSource, ".codex-plugin", "plugin.json"), JSON.stringify({
    name: "github",
    description: "GitHub",
  }), "utf8")

  const previousSourceHome = process.env.COMMAND_RUN_AGENT_CODEX_SOURCE_HOME
  process.env.COMMAND_RUN_AGENT_CODEX_SOURCE_HOME = sourceHome
  try {
    const preparedHome = prepareCodexCliHomeForAgent("codex-main-ponytail", targetHome, {
      model: "gpt-5.5",
      reasoning: "medium",
      serviceTier: "default",
    })
    assert.equal(preparedHome, path.resolve(targetHome))
    assert.equal(fs.readFileSync(path.join(targetHome, "auth.json"), "utf8"), "{}\n")
    assert.equal(fs.existsSync(path.join(targetHome, "config.toml")), false)

    const installed = seedCodexPluginCache(targetHome, {
      marketplaceName: "ponytail-github",
      pluginName: "github",
      version: "local",
      sourcePluginDir: pluginSource,
    })
    assert.equal(installed, path.join(path.resolve(targetHome), "plugins", "cache", "ponytail-github", "github", "local"))
    assert.ok(fs.existsSync(path.join(installed, ".codex-plugin", "plugin.json")))
  } finally {
    if (previousSourceHome === undefined) delete process.env.COMMAND_RUN_AGENT_CODEX_SOURCE_HOME
    else process.env.COMMAND_RUN_AGENT_CODEX_SOURCE_HOME = previousSourceHome
  }
})

test("codex agents default to a per-run isolated CODEX_HOME", () => {
  const sourceHome = tempAgentDir()
  const agentDir = tempAgentDir()
  fs.writeFileSync(path.join(sourceHome, "auth.json"), "{\"token\":true}\n", "utf8")

  const names = [
    "COMMAND_RUN_AGENT_CODEX_SOURCE_HOME",
    "COMMAND_RUN_AGENT_CODEX_HOME",
    "COMMAND_RUN_AGENT_CODEX_MAIN_HOME",
    "COMMAND_RUN_AGENT_CODEX_CLEAN_HOME",
    "COMMAND_RUN_AGENT_CODEX_PREPARE_HOME",
    "COMMAND_RUN_AGENT_CODEX_HOME_CODEX_MAIN",
  ]
  const previous = new Map(names.map((name) => [name, process.env[name]]))
  process.env.COMMAND_RUN_AGENT_CODEX_SOURCE_HOME = sourceHome
  for (const name of names.slice(1)) delete process.env[name]
  try {
    const codexHome = codexHomeForAgent("codex-main", { agentDir })
    assert.equal(codexHome, path.join(agentDir, "codex-home"))
    assert.equal(fs.readFileSync(path.join(codexHome, "auth.json"), "utf8"), "{\"token\":true}\n")
    assert.equal(fs.existsSync(path.join(codexHome, "state_5.sqlite")), false)
  } finally {
    for (const [name, value] of previous) {
      if (value === undefined) delete process.env[name]
      else process.env[name] = value
    }
  }
})

test("runLive settles idle processes instead of leaving them running forever", async () => {
  const agentDir = tempAgentDir()
  const statusPath = path.join(agentDir, "status.json")
  const result = await runLive(process.execPath, ["-e", "setInterval(() => {}, 1000)"], {
    cwd: agentDir,
    timeoutMs: 5000,
    idleTimeoutMs: 200,
    stdoutPath: path.join(agentDir, "stdout.jsonl"),
    stderrPath: path.join(agentDir, "stderr.log"),
    statusPath,
  })

  assert.equal(result.status, 1)
  assert.equal(result.timed_out, true)
  assert.match(result.error, /no stdout\/stderr progress/)
  const status = JSON.parse(fs.readFileSync(statusPath, "utf8"))
  assert.equal(status.status, "error")
  assert.match(status.result.error, /no stdout\/stderr progress/)
})

test("codex CLI setup commands use the generic isolated setup hook", () => {
  const agentDir = tempAgentDir()
  const codexHome = tempAgentDir()
  const setupScript = [
    "const fs=require('node:fs')",
    "const path=require('node:path')",
    "fs.writeFileSync(path.join(process.env.CODEX_HOME,'setup.txt'),process.argv.slice(1).join('|'))",
  ].join(";")

  const setupLog = runCodexCliSetupCommands("codex-main-ponytail", process.execPath, [
    ["-e", setupScript, "alpha", "beta"],
  ], {
    agentDir,
    codexHome,
    cwd: agentDir,
    timeoutMs: 30_000,
  })

  assert.equal(setupLog, path.join(agentDir, "codex-cli-setup.jsonl"))
  assert.equal(fs.readFileSync(path.join(codexHome, "setup.txt"), "utf8"), "alpha|beta")
  const records = fs.readFileSync(setupLog, "utf8").trim().split(/\r?\n/).map((line) => JSON.parse(line))
  assert.equal(records.length, 1)
  assert.equal(records[0].type, "codex_cli_setup.completed")
  assert.equal(records[0].agent_id, "codex-main-ponytail")
  assert.deepEqual(records[0].args.slice(0, 2), ["-e", setupScript])
  assert.equal(records[0].codex_home, path.resolve(codexHome))
  assert.equal(fs.existsSync(path.join(codexHome, "config.toml")), false)
})

test("codex token usage updates are counted as per-round increments", () => {
  const stdout = [
    { type: "turn.started" },
    {
      type: "thread.token_usage.updated",
      usage: { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1 },
      total_usage: { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1 },
    },
    {
      type: "thread.token_usage.updated",
      usage: { input_tokens: 11, cached_input_tokens: 5, output_tokens: 3, reasoning_output_tokens: 2 },
      total_usage: { input_tokens: 21, cached_input_tokens: 9, output_tokens: 5, reasoning_output_tokens: 3 },
    },
    { type: "turn.completed", usage: { input_tokens: 21, cached_input_tokens: 9, output_tokens: 5, reasoning_output_tokens: 3 } },
  ].map((event) => JSON.stringify(event)).join("\n")

  const info = usageForAgent(tempAgentDir(), stdout, "codex-main")
  assert.equal(info.usage_source, "stdout-jsonl")
  assert.deepEqual(info.usage, {
    usage_events: 2,
    input_tokens: 21,
    output_tokens: 5,
    reasoning_tokens: 3,
    cached_input_tokens: 9,
    cache_write_tokens: 0,
    total_tokens: 26,
    latency_ms: 0,
  })
  assert.deepEqual(eventsForAgent(stdout, "codex-main"), {
    events: 4,
    thread_started: 0,
    turn_started: 1,
    turn_completed: 1,
    round_completed: 0,
    token_usage_updates: 2,
    agent_messages: 0,
    command_executions: 0,
    commands_completed: 0,
    commands_failed: 0,
    file_changes: 0,
    llm_rounds: 2,
    callback_ok: true,
  })
})

test("pi and opencode round callbacks use the same usage parser", () => {
  const stdout = [
    {
      type: "pi.round.completed",
      agent_id: "pi-agent",
      usage: { input_tokens: 7, cached_input_tokens: 2, output_tokens: 3, reasoning_tokens: 1, total_tokens: 10 },
    },
    {
      type: "opencode.round.completed",
      agent_id: "opencode",
      metrics: { inputTokens: 13, cacheInputTokens: 8, outputTokens: 5, reasoningTokens: 2, totalTokens: 18, durationMs: 1234 },
    },
  ].map((event) => JSON.stringify(event)).join("\n")

  const pi = usageForAgent(tempAgentDir(), stdout.split(/\r?\n/)[0], "pi-agent")
  assert.equal(pi.usage_source, "stdout-jsonl")
  assert.equal(pi.usage.usage_events, 1)
  assert.equal(pi.usage.input_tokens, 7)
  assert.equal(pi.usage.cached_input_tokens, 2)
  assert.equal(pi.usage.output_tokens, 3)
  assert.equal(pi.usage.reasoning_tokens, 1)
  assert.equal(pi.usage.total_tokens, 10)

  const opencode = usageForAgent(tempAgentDir(), stdout.split(/\r?\n/)[1], "opencode")
  assert.equal(opencode.usage_source, "stdout-jsonl")
  assert.equal(opencode.usage.usage_events, 1)
  assert.equal(opencode.usage.input_tokens, 13)
  assert.equal(opencode.usage.cached_input_tokens, 8)
  assert.equal(opencode.usage.output_tokens, 5)
  assert.equal(opencode.usage.reasoning_tokens, 2)
  assert.equal(opencode.usage.total_tokens, 18)
  assert.equal(opencode.usage.latency_ms, 1234)

  assert.equal(eventsForAgent(stdout, "pi-agent").round_completed, 2)
  assert.equal(eventsForAgent(stdout, "opencode").llm_rounds, 2)
})

test("pi and opencode native cache split counts as total input", () => {
  const piStdout = JSON.stringify({
    type: "turn_end",
    message: {
      usage: { input: 432, output: 28, cacheRead: 5632, cacheWrite: 0, totalTokens: 6092 },
    },
  })
  const opencodeStdout = JSON.stringify({
    type: "step_finish",
    part: {
      tokens: { input: 189, output: 84, reasoning: 19, total: 16676, cache: { write: 0, read: 16384 } },
    },
  })

  const pi = usageForAgent(tempAgentDir(), piStdout, "pi-agent")
  assert.equal(pi.usage.input_tokens, 6064)
  assert.equal(pi.usage.cached_input_tokens, 5632)
  assert.equal(pi.usage.output_tokens, 28)
  assert.equal(pi.usage.total_tokens, 6092)

  const opencode = usageForAgent(tempAgentDir(), opencodeStdout, "opencode")
  assert.equal(opencode.usage.input_tokens, 16573)
  assert.equal(opencode.usage.cached_input_tokens, 16384)
  assert.equal(opencode.usage.output_tokens, 84)
  assert.equal(opencode.usage.reasoning_tokens, 19)
  assert.equal(opencode.usage.total_tokens, 16676)
})

test("standalone compact usage is added alongside native turn usage", () => {
  const stdout = [
    {
      type: "turn_end",
      message: {
        usage: { input: 100, output: 20, cacheRead: 400, cacheWrite: 0, totalTokens: 520 },
      },
    },
    {
      type: "context.compact",
      usage: { input: 30, output: 10, cacheRead: 70, cacheWrite: 0, totalTokens: 110 },
    },
  ].map((event) => JSON.stringify(event)).join("\n")

  const usage = usageForAgent(tempAgentDir(), stdout, "pi-agent")
  assert.equal(usage.usage.usage_events, 2)
  assert.equal(usage.usage.input_tokens, 600)
  assert.equal(usage.usage.cached_input_tokens, 470)
  assert.equal(usage.usage.output_tokens, 30)
  assert.equal(usage.usage.total_tokens, 630)
})

test("codex rollout archive usage is counted per unique token_count", () => {
  const agentDir = tempAgentDir()
  const archiveDir = path.join(agentDir, "context-and-calls")
  fs.mkdirSync(archiveDir, { recursive: true })
  const rolloutPath = path.join(archiveDir, "codex-rollout.jsonl")
  const records = [
    {
      timestamp: "2026-01-01T00:00:01.000Z",
      type: "event_msg",
      payload: {
        type: "token_count",
        info: {
          last_token_usage: { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1, total_tokens: 12 },
          total_token_usage: { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1, total_tokens: 12 },
        },
      },
    },
    {
      timestamp: "2026-01-01T00:00:02.000Z",
      type: "event_msg",
      payload: {
        type: "token_count",
        info: {
          last_token_usage: { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1, total_tokens: 12 },
          total_token_usage: { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1, total_tokens: 12 },
        },
      },
    },
    {
      timestamp: "2026-01-01T00:00:03.000Z",
      type: "event_msg",
      payload: {
        type: "token_count",
        info: {
          last_token_usage: { input_tokens: 11, cached_input_tokens: 5, output_tokens: 3, reasoning_output_tokens: 2, total_tokens: 14 },
          total_token_usage: { input_tokens: 21, cached_input_tokens: 9, output_tokens: 5, reasoning_output_tokens: 3, total_tokens: 26 },
        },
      },
    },
  ]
  fs.writeFileSync(rolloutPath, records.map((record) => JSON.stringify(record)).join("\n") + "\n", "utf8")
  fs.writeFileSync(path.join(archiveDir, "codex-rollout-paths.json"), JSON.stringify([rolloutPath]), "utf8")

  const info = usageForAgent(agentDir, "", "codex-main")
  assert.equal(info.usage_source, "codex-rollout")
  assert.deepEqual(info.usage, {
    usage_events: 2,
    input_tokens: 21,
    output_tokens: 5,
    reasoning_tokens: 3,
    cached_input_tokens: 9,
    cache_write_tokens: 0,
    total_tokens: 26,
    latency_ms: 0,
  })
})

test("event round count is reconciled with provider usage events", () => {
  const stdout = [
    { type: "turn.started" },
    { type: "turn.completed" },
  ].map((event) => JSON.stringify(event)).join("\n")

  const events = eventsForAgent(stdout, "tura-balanced")
  assert.equal(events.llm_rounds, 1)

  const reconciled = eventsWithUsageRounds(events, { usage_events: 14 })
  assert.equal(reconciled.llm_rounds, 14)
  assert.equal(reconciled.callback_ok, true)
})

test("codex token fixture stdout produces agent-specific round callbacks", () => {
  const source = [
    {
      type: "thread.token_usage.updated",
      usage: { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1 },
      total_usage: { input_tokens: 10, cached_input_tokens: 4, output_tokens: 2, reasoning_output_tokens: 1 },
    },
    {
      type: "thread.token_usage.updated",
      usage: { input_tokens: 11, cached_input_tokens: 5, output_tokens: 3, reasoning_output_tokens: 2 },
      total_usage: { input_tokens: 21, cached_input_tokens: 9, output_tokens: 5, reasoning_output_tokens: 3 },
    },
    { type: "turn.completed", usage: { input_tokens: 21, cached_input_tokens: 9, output_tokens: 5, reasoning_output_tokens: 3 } },
  ].map((event) => JSON.stringify(event)).join("\n")

  const piStdout = buildCodexTokenFixtureStdout(source, "pi-agent", {
    fixtureSourcePath: "codex.stdout.jsonl",
    model: "gpt-5.5",
    reasoning: "medium",
    serviceTier: "default",
  })
  const records = piStdout.trim().split(/\r?\n/).map((line) => JSON.parse(line))

  assert.equal(records.length, 2)
  assert.equal(records[0].type, "pi.round.completed")
  assert.equal(records[0].agent_id, "pi-agent")
  assert.equal(records[0].fixture_backend, "codex")
  assert.equal(records[0].round_source, "codex-token-fixture")

  const usage = usageForAgent(tempAgentDir(), piStdout, "pi-agent")
  assert.equal(usage.usage_source, "stdout-jsonl")
  assert.equal(usage.usage.usage_events, 2)
  assert.equal(usage.usage.input_tokens, 21)
  assert.equal(usage.usage.cached_input_tokens, 9)
  assert.equal(usage.usage.output_tokens, 5)
  assert.equal(usage.usage.reasoning_tokens, 3)
  assert.equal(usage.usage.total_tokens, 26)
  assert.equal(eventsForAgent(piStdout, "pi-agent").llm_rounds, 2)

  const opencodeStdout = buildCodexTokenFixtureStdout(source, "opencode")
  assert.equal(JSON.parse(opencodeStdout.trim().split(/\r?\n/)[0]).type, "opencode.round.completed")
})
