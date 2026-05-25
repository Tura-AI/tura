#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn } from "node:child_process"
import fs from "node:fs/promises"
import http from "node:http"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { pathToFileURL } from "node:url"

const repoRoot = process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..")
const nodeBin = process.execPath
const tuiBin = path.join(repoRoot, "apps", "tui", "dist", "index.js")
const runRoot = path.join(repoRoot, "target", "tui-cli-gateway-e2e", String(Date.now()))

function sendJson(res, value, status = 200) {
  const body = JSON.stringify(value)
  res.writeHead(status, { "content-type": "application/json", "content-length": Buffer.byteLength(body) })
  res.end(body)
}

function readJson(req) {
  return new Promise((resolve) => {
    let body = ""
    req.on("data", (chunk) => {
      body += chunk.toString()
    })
    req.on("end", () => {
      resolve(body.trim() ? JSON.parse(body) : {})
    })
  })
}

function startGateway() {
  const records = {
    requests: [],
    createSessions: [],
    prompts: [],
    configPatches: [],
    globalConfigPatches: [],
    commandRuns: [],
    permissionReplies: [],
    questionReplies: [],
    questionRejects: [],
    providerLogouts: [],
    sessionUpdates: [],
    sentMessages: [],
    aborts: [],
    streamDeltaAt: null,
    completedAt: null,
  }
  const clients = new Set()
  let config = {
    model: "openai/from-config",
    active_agent: "coding_agent",
    model_variant: "low",
    model_acceleration_enabled: true,
  }
  let globalConfig = {
    language: "en",
    theme: "dark",
    model: "openai/from-global",
    agent: "coding_agent",
    skill_folders: [],
  }
  let session = {
    id: "sess-e2e",
    title: "TUI e2e",
    directory: runRoot,
    status: "idle",
    model: config.model,
    agent: config.active_agent,
    model_variant: config.model_variant,
    model_acceleration_enabled: config.model_acceleration_enabled,
    created_at: Date.now(),
    updated_at: Date.now(),
    message_count: 0,
  }
  const providerList = {
    all: [
      {
        id: "openai",
        name: "OpenAI",
        source: "config",
        env: ["OPENAI_API_KEY"],
        options: {},
        models: { "gpt-test": { id: "gpt-test", name: "gpt-test" } },
      },
    ],
    default: { openai: "gpt-test" },
    connected: ["openai"],
  }
  let messages = [
    {
      id: "msg-initial",
      sessionID: session.id,
      role: "assistant",
      parts: [{ id: "part-initial", type: "text", text: "initial assistant message" }],
      created_at: Date.now(),
      updated_at: Date.now(),
    },
  ]
  let permissions = [{ id: "perm-1", session_id: session.id, permission: "shell", args: { command: "echo ok" } }]
  let questions = [{ id: "q-1", session_id: session.id, question: "Proceed?", metadata: {} }]
  const todos = [{ id: "todo-1", content: "Wire gateway", status: "in_progress" }]

  const pushEvent = (event) => {
    const data = `data: ${JSON.stringify(event)}\n\n`
    for (const res of clients) res.write(data)
  }
  const waitForClient = async () => {
    for (let index = 0; index < 50 && clients.size === 0; index += 1) {
      await new Promise((resolve) => setTimeout(resolve, 20))
    }
  }

  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url || "/", "http://127.0.0.1")
    records.requests.push({ method: req.method, path: url.pathname, query: Object.fromEntries(url.searchParams) })
    if (req.method === "GET" && url.pathname === "/global/health") return sendJson(res, { healthy: true, version: "e2e" })
    if (req.method === "GET" && url.pathname === "/project/current") return sendJson(res, { project: { worktree: runRoot } })
    if (req.method === "GET" && url.pathname === "/config") return sendJson(res, globalConfig)
    if (req.method === "PATCH" && url.pathname === "/config") {
      const patch = await readJson(req)
      records.globalConfigPatches.push(patch)
      globalConfig = { ...globalConfig, ...patch }
      return sendJson(res, globalConfig)
    }
    if (req.method === "GET" && url.pathname === "/permission") return sendJson(res, permissions)
    if (req.method === "POST" && url.pathname.startsWith("/permission/") && url.pathname.endsWith("/reply")) {
      const id = decodeURIComponent(url.pathname.split("/")[2])
      const payload = await readJson(req)
      records.permissionReplies.push({ id, payload })
      permissions = permissions.filter((item) => item.id !== id)
      return sendJson(res, { success: true })
    }
    if (req.method === "GET" && url.pathname === "/question") return sendJson(res, questions)
    if (req.method === "POST" && url.pathname.startsWith("/question/") && url.pathname.endsWith("/reply")) {
      const id = decodeURIComponent(url.pathname.split("/")[2])
      const payload = await readJson(req)
      records.questionReplies.push({ id, payload })
      questions = questions.filter((item) => item.id !== id)
      return sendJson(res, { success: true })
    }
    if (req.method === "POST" && url.pathname.startsWith("/question/") && url.pathname.endsWith("/reject")) {
      const id = decodeURIComponent(url.pathname.split("/")[2])
      records.questionRejects.push({ id })
      questions = questions.filter((item) => item.id !== id)
      return sendJson(res, true)
    }
    if (req.method === "GET" && url.pathname === "/provider") return sendJson(res, providerList)
    if (req.method === "GET" && url.pathname === "/provider/openai/auth/status") {
      return sendJson(res, { provider_id: "openai", configured: true, authenticated: true, auth_state: "authenticated", runtime_state: "ready" })
    }
    if (req.method === "POST" && url.pathname === "/provider/openai/auth/logout") {
      records.providerLogouts.push("openai")
      return sendJson(res, { ok: true, provider_id: "openai", message: "logged out" })
    }
    if (req.method === "POST" && url.pathname === "/provider/model/validate") return sendJson(res, { ok: true, message: "ok" })
    if (req.method === "GET" && url.pathname === "/command") return sendJson(res, [{ name: "hello", description: "Say hello", source: "mock", subtask: false, hints: [] }])
    if (req.method === "POST" && url.pathname === "/command") {
      const payload = await readJson(req)
      records.commandRuns.push(payload)
      return sendJson(res, { output: `ran ${payload.command} ${(payload.args || []).join(" ")}`.trim() })
    }
    if (req.method === "GET" && url.pathname === "/vcs") return sendJson(res, { branch: "main", default_branch: "main" })
    if (req.method === "GET" && url.pathname === "/vcs/diff") return sendJson(res, { files: [{ old_file_name: "a.txt", new_file_name: "a.txt", hunks: [{ lines: ["+ok"] }] }] })
    if (req.method === "GET" && url.pathname === "/service/status") return sendJson(res, { mano: { status: "ok" }, router: { status: "ok" }, lsp: [] })
    if (req.method === "GET" && url.pathname === "/skill") return sendJson(res, [{ name: "skill-a", description: "mock", path: "/tmp/skill-a" }])
    if (req.method === "GET" && url.pathname === "/plugin") return sendJson(res, [{ id: "plugin-a", name: "Plugin A", description: "mock", path: "/tmp/plugin-a", enabled: true, skills: [] }])
    if (req.method === "GET" && url.pathname === "/session/config") return sendJson(res, config)
    if (req.method === "PATCH" && url.pathname === "/session/config") {
      const patch = await readJson(req)
      records.configPatches.push(patch)
      config = { ...config, ...patch }
      return sendJson(res, config)
    }
    if (req.method === "POST" && url.pathname === "/session") {
      const payload = await readJson(req)
      records.createSessions.push(payload)
      session = {
        ...session,
        directory: payload.directory ?? session.directory,
        model: payload.model ?? config.model,
        agent: payload.agent ?? config.active_agent,
        session_type: payload.session_type,
        model_variant: payload.model_variant ?? config.model_variant,
        model_acceleration_enabled: payload.model_acceleration_enabled ?? config.model_acceleration_enabled,
        force_multiple_tasks: payload.force_multiple_tasks,
      }
      return sendJson(res, session)
    }
    if (req.method === "GET" && url.pathname === "/session") return sendJson(res, [session])
    if (req.method === "GET" && url.pathname === "/session/status") return sendJson(res, { [session.id]: session.status })
    if (req.method === "GET" && url.pathname === `/session/${session.id}`) return sendJson(res, session)
    if (req.method === "PATCH" && url.pathname === `/session/${session.id}`) {
      const payload = await readJson(req)
      records.sessionUpdates.push(payload)
      session = { ...session, ...payload, updated_at: Date.now() }
      return sendJson(res, session)
    }
    if (req.method === "DELETE" && url.pathname === `/session/${session.id}`) return sendJson(res, true)
    if (req.method === "GET" && url.pathname === `/session/${session.id}/message`) return sendJson(res, messages)
    if (req.method === "POST" && url.pathname === `/session/${session.id}/message`) {
      const payload = await readJson(req)
      records.sentMessages.push(payload)
      const message = {
        id: `msg-manual-${records.sentMessages.length}`,
        sessionID: session.id,
        role: "user",
        parts: [{ id: `part-manual-${records.sentMessages.length}`, type: "text", text: payload.content }],
        created_at: Date.now(),
        updated_at: Date.now(),
      }
      messages.push(message)
      return sendJson(res, message)
    }
    if (req.method === "POST" && url.pathname === `/session/${session.id}/abort`) {
      records.aborts.push(session.id)
      return sendJson(res, { ok: true })
    }
    if (req.method === "GET" && url.pathname === `/session/${session.id}/todo`) return sendJson(res, todos)
    if (req.method === "POST" && url.pathname === `/session/${session.id}/prompt_async`) {
      const payload = await readJson(req)
      records.prompts.push(payload)
      session = { ...session, status: "busy" }
      messages.push({
        id: payload.messageID,
        sessionID: session.id,
        role: "user",
        parts: payload.parts,
        created_at: Date.now(),
        updated_at: Date.now(),
      })
      void (async () => {
        await waitForClient()
        await new Promise((resolve) => setTimeout(resolve, 80))
        records.streamDeltaAt = performance.now()
        pushEvent({
          directory: runRoot,
          payload: {
            type: "message.part.delta",
            properties: {
              sessionID: session.id,
              messageID: "msg-assistant",
              partID: "part-assistant",
              field: "text",
              delta: "streaming-middle",
            },
          },
        })
        await new Promise((resolve) => setTimeout(resolve, 180))
        records.completedAt = performance.now()
        session = { ...session, status: "idle" }
        messages.push({
          id: "msg-assistant",
          sessionID: session.id,
          role: "assistant",
          parts: [{ id: "part-assistant", type: "text", text: "streaming-middle final" }],
          created_at: Date.now(),
          updated_at: Date.now(),
        })
        pushEvent({
          directory: runRoot,
          payload: { type: "session.status", properties: { sessionID: session.id, status: "idle" } },
        })
        pushEvent({
          directory: runRoot,
          payload: { type: "message.updated", properties: { sessionID: session.id, info: messages.at(-1) } },
        })
      })()
      res.writeHead(204)
      return res.end()
    }
    if (req.method === "GET" && url.pathname === "/event") {
      res.writeHead(200, {
        "content-type": "text/event-stream",
        "cache-control": "no-cache",
        connection: "keep-alive",
      })
      clients.add(res)
      res.write(`data: ${JSON.stringify({ directory: "global", payload: { type: "server.connected", properties: {} } })}\n\n`)
      req.on("close", () => clients.delete(res))
      return
    }
    sendJson(res, { error: `unhandled ${req.method} ${url.pathname}` }, 404)
  })

  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const address = server.address()
      resolve({ server, records, url: `http://127.0.0.1:${address.port}` })
    })
  })
}

function spawnCli(args, options = {}) {
  return new Promise((resolve) => {
    const started = performance.now()
    const child = spawn(nodeBin, [tuiBin, ...args], {
      cwd: repoRoot,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, ...(options.env || {}) },
    })
    let stdout = ""
    let stderr = ""
    let firstStreamingLineAt = null
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString()
      if (firstStreamingLineAt === null && stdout.includes("streaming-middle")) {
        firstStreamingLineAt = performance.now()
      }
    })
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString()
    })
    child.on("close", (status) => {
      resolve({ status, stdout, stderr, durationMs: Math.round(performance.now() - started), firstStreamingLineAt })
    })
  })
}

async function expectCliOk(args) {
  const result = await spawnCli(args)
  assert.equal(result.status, 0, result.stderr)
  return result
}

async function expectCliJson(args) {
  const result = await expectCliOk(args)
  return JSON.parse(result.stdout)
}

function baseArgs(gateway) {
  return ["--gateway-url", gateway.url, "--cwd", runRoot]
}

async function main() {
  await fs.mkdir(runRoot, { recursive: true })
  await fs.access(tuiBin)
  const gateway = await startGateway()
  try {
    const configResult = await expectCliOk([
      ...baseArgs(gateway),
      "config",
      "set",
      "agent=coding_agent_fast",
      "model_reasoning_effort=medium",
      "service_tier=priority",
    ])
    assert.deepEqual(gateway.records.configPatches[0], {
      active_agent: "coding_agent_fast",
      model_variant: "medium",
      model_acceleration_enabled: true,
    })
    const configGet = await expectCliJson([...baseArgs(gateway), "config", "get"])
    assert.equal(configGet.active_agent, "coding_agent_fast")

    const sessions = await expectCliJson([...baseArgs(gateway), "--json", "session", "list"])
    assert.equal(sessions[0].id, "sess-e2e")
    const sessionShow = await expectCliJson([...baseArgs(gateway), "--json", "session", "show", "sess-e2e"])
    assert.equal(sessionShow.session.id, "sess-e2e")
    assert.equal(sessionShow.todos[0].id, "todo-1")
    const sessionDelete = await expectCliJson([...baseArgs(gateway), "--json", "session", "delete", "sess-e2e"])
    assert.equal(sessionDelete.deleted, true)

    const providerList = await expectCliJson([...baseArgs(gateway), "--json", "provider", "list"])
    assert.equal(providerList.all[0].id, "openai")
    const providerStatus = await expectCliJson([...baseArgs(gateway), "provider", "status", "openai"])
    assert.equal(providerStatus.authenticated, true)
    const providerStatuses = await expectCliJson([...baseArgs(gateway), "provider", "status"])
    assert.equal(providerStatuses[0].provider_id, "openai")
    const providerLogout = await expectCliJson([...baseArgs(gateway), "provider", "logout", "openai"])
    assert.equal(providerLogout.ok, true)

    const permissionList = await expectCliJson([...baseArgs(gateway), "--json", "permission", "list"])
    assert.equal(permissionList[0].id, "perm-1")
    const permissionReply = await expectCliJson([...baseArgs(gateway), "permission", "reply", "perm-1", "--approve"])
    assert.equal(permissionReply.success, true)
    assert.deepEqual(gateway.records.permissionReplies[0], { id: "perm-1", payload: { approve: true } })

    const commandList = await expectCliJson([...baseArgs(gateway), "--json", "command", "list"])
    assert.equal(commandList[0].name, "hello")
    const commandRun = await expectCliOk([...baseArgs(gateway), "command", "run", "hello", "world"])
    assert.match(commandRun.stdout, /ran hello world/)
    assert.deepEqual(gateway.records.commandRuns[0], { command: "hello", args: ["world"] })

    const status = await expectCliJson([...baseArgs(gateway), "--json", "status"])
    assert.equal(status.health.version, "e2e")
    assert.equal(status.sessions[0].id, "sess-e2e")

    const resumeShow = await expectCliOk([...baseArgs(gateway), "resume", "sess-e2e"])
    assert.match(resumeShow.stdout, /initial assistant message/)

    for (const shell of ["bash", "zsh", "fish"]) {
      const completion = await expectCliOk(["completion", shell])
      assert.match(completion.stdout, /tura|complete|_arguments/)
    }

    const runResult = await expectCliOk([
      ...baseArgs(gateway),
      "run",
      "hello from tui cli",
      "--output",
      "ndjson",
      "--model",
      "openai/gpt-test",
      "--reasoning-effort",
      "high",
      "--no-model-acceleration",
      "-c",
      "force_multiple_tasks=true",
      "--timeout",
      "5",
    ])
    assert.equal(gateway.records.createSessions[0].model, "openai/gpt-test")
    assert.equal(gateway.records.createSessions[0].model_variant, "high")
    assert.equal(gateway.records.createSessions[0].model_acceleration_enabled, false)
    assert.equal(gateway.records.createSessions[0].force_multiple_tasks, true)
    assert.equal(gateway.records.prompts[0].model, "openai/gpt-test")
    assert.equal(gateway.records.prompts[0].variant, "high")
    assert.equal(gateway.records.prompts[0].model_variant, "high")
    assert.equal(gateway.records.prompts[0].model_acceleration_enabled, false)

    const events = runResult.stdout.split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line))
    const deltaIndex = events.findIndex((event) => event.type === "message.part.delta" && event.text === "streaming-middle")
    const completedIndex = events.findIndex((event) => event.type === "cli.completed")
    assert.ok(deltaIndex >= 0, "streaming delta should be emitted")
    assert.ok(completedIndex > deltaIndex, "completion should arrive after the streaming delta")
    assert.ok(runResult.firstStreamingLineAt && gateway.records.completedAt)
    assert.ok(runResult.firstStreamingLineAt < gateway.records.completedAt, "test observed streaming before backend completion")
    console.log(`[tui-cli-e2e] streaming delta observed before completion; duration=${runResult.durationMs}ms`)

    const noStreamResult = await expectCliOk([
      ...baseArgs(gateway),
      "run",
      "poll without event stream",
      "--json",
      "--no-stream",
      "--timeout",
      "5",
    ])
    const noStreamJson = JSON.parse(noStreamResult.stdout)
    assert.equal(noStreamJson.status, "completed")
    assert.equal(noStreamJson.finalText, "streaming-middle final")
    console.log(`[tui-cli-e2e] no-stream polling completed; duration=${noStreamResult.durationMs}ms`)

    const { GatewayClient } = await import(pathToFileURL(path.join(repoRoot, "apps", "tui", "dist", "gateway", "client.js")).href)
    const client = new GatewayClient({ baseUrl: gateway.url, directory: runRoot, timeoutMs: 5000 })
    assert.equal((await client.health()).version, "e2e")
    await client.syncWorkspace()
    assert.equal((await client.getGlobalConfig()).theme, "dark")
    assert.equal((await client.patchGlobalConfig({ theme: "light" })).theme, "light")
    assert.equal((await client.getSessionConfig()).model_variant, "medium")
    assert.equal((await client.patchSessionConfig({ model_variant: "low" })).model_variant, "low")
    assert.equal((await client.listSessions({ all: true, includeChildren: true, limit: 3 }))[0].id, "sess-e2e")
    assert.equal((await client.getSession("sess-e2e")).id, "sess-e2e")
    assert.equal((await client.updateSession("sess-e2e", { agent: "coding_agent" })).agent, "coding_agent")
    assert.equal((await client.sendMessage("sess-e2e", "manual message")).role, "user")
    assert.equal((await client.todos("sess-e2e"))[0].id, "todo-1")
    assert.deepEqual(await client.listPermissions(), [])
    assert.equal((await client.listQuestions())[0].id, "q-1")
    assert.equal((await client.replyQuestion("q-1", "yes")).success, true)
    assert.equal(await client.rejectQuestion("q-3"), true)
    assert.equal((await client.listProviders()).all[0].id, "openai")
    assert.equal((await client.providerAuthStatus("openai")).authenticated, true)
    assert.equal((await client.providerLogout("openai")).ok, true)
    assert.equal((await client.validateModel("openai/gpt-test")).ok, true)
    assert.equal((await client.listCommands())[0].name, "hello")
    assert.match((await client.executeCommand("hello", ["client"])).output, /client/)
    assert.equal((await client.vcs()).branch, "main")
    assert.equal((await client.diff()).files[0].new_file_name, "a.txt")
    assert.equal((await client.serviceStatus()).mano.status, "ok")
    assert.equal((await client.skills())[0].name, "skill-a")
    assert.equal((await client.plugins())[0].id, "plugin-a")
    await client.abort("sess-e2e")
    assert.ok(gateway.records.aborts.includes("sess-e2e"))

    const controller = new AbortController()
    const stream = client.streamEvents(controller.signal)
    const firstEvent = await stream.next()
    controller.abort()
    await stream.return?.()
    assert.equal(firstEvent.value.payload.type, "server.connected")
    console.log("[tui-cli-e2e] direct GatewayClient endpoint sweep ok=true")
    console.log("[tui-cli-e2e] ok=true")
  } finally {
    await new Promise((resolve) => gateway.server.close(resolve))
  }
}

main().catch((error) => {
  console.error(error.stack || error.message)
  process.exitCode = 1
})
