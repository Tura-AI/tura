#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn } from "node:child_process"
import fs from "node:fs/promises"
import http from "node:http"
import { createRequire } from "node:module"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { pathToFileURL } from "node:url"

const repoRoot = process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..")
const nodeBin = process.execPath
const tuiBin = path.join(repoRoot, "apps", "tui", "dist", "index.js")
const runRoot = path.join(repoRoot, "target", "tui-cli-gateway-e2e", String(Date.now()))
const tuiRequire = createRequire(path.join(repoRoot, "apps", "tui", "package.json"))

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
    agents: [],
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
  const task = (id, title, status, offset = 0, directory = runRoot, _trigger = "user_action") => ({
    nonce_id: `${id}:0`,
    step: 0,
    plan_summary: title,
    task_summary: `执行任务：${title}`,
    delivery: "tui session plan e2e",
    sub_session_id: "",
    start_at: new Date(Date.now() + offset).toISOString(),
    poll_interval: { m: 0, d: 0, h: 1, s: 0 },
    status: status,
  })
  const makeSession = (id, title, status, offset = 0, directory = runRoot, trigger = "user_action") => ({
    id,
    name: title,
    directory,
    status: status === "doing" ? "busy" : "idle",
    model: config.model,
    agent: config.active_agent,
    model_variant: config.model_variant,
    model_acceleration_enabled: config.model_acceleration_enabled,
    created_at: Date.now() - offset - 1000,
    updated_at: Date.now() - offset,
    message_count: 0,
    plan_summary: title,
    session_display_name: title,
    task_management: task(id, title, status, offset, directory, trigger),
  })
  let session = {
    id: "sess-e2e",
    name: "TUI e2e",
    directory: runRoot,
    status: "idle",
    model: config.model,
    agent: config.active_agent,
    model_variant: config.model_variant,
    model_acceleration_enabled: config.model_acceleration_enabled,
    created_at: Date.now(),
    updated_at: Date.now(),
    message_count: 0,
    plan_summary: "TUI e2e",
    session_display_name: "TUI e2e",
    task_management: task("sess-e2e", "TUI e2e", "todo"),
  }
  let sessions = [
    session,
    makeSession("plan-doing-002", "实现拖拽状态切换", "doing", 2000),
    makeSession("plan-question-003", "等待用户补充权限", "question", 3000, runRoot, "scheduled_task"),
    makeSession("plan-done-004", "完成 gateway 字段回传", "done", 4000),
    makeSession("plan-archived-005", "隐藏旧会话工单", "archived", 5000),
    makeSession("plan-other-006", "其他目录里的待办", "todo", 6000, path.join(runRoot, "other")),
  ]
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
  const agents = new Map([
    ["coding_agent_fast", {
      summary: {
        id: "coding_agent_fast",
        name: "Coding Agent Fast",
        description: "Fast coding agent",
        source: "static",
        path: "agents/src/coding_agent_fast",
        aliases: [],
        capabilities: ["command_run"],
        provider: "fast",
        hidden: false,
      },
      config: { agent_name: "coding_agent_fast", agent_directory: "agents/src/coding_agent_fast" },
      prompt: "Fast prompt",
    }],
  ])

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
    if (req.method === "GET" && url.pathname === "/provider/auth") {
      return sendJson(res, {
        openai: [
          {
            type: "oauth",
            kind: "OAuthPkce",
            login: "oauth",
            label: "OpenAI OAuth",
            token_env: "OPENAI_API_KEY",
            login_env: "OPENAI_LOGIN",
          },
        ],
      })
    }
    if (req.method === "GET" && url.pathname === "/provider/openai/auth/status") {
      return sendJson(res, { provider_id: "openai", configured: true, authenticated: true, auth_state: "authenticated", runtime_state: "ready" })
    }
    if (req.method === "POST" && url.pathname === "/provider/openai/oauth/authorize") {
      return sendJson(res, {
        url: "https://auth.example.test/openai",
        method: "auto",
        instructions: "OpenAI OAuth test login started.",
      })
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
    if (req.method === "GET" && url.pathname === "/agent") {
      return sendJson(res, Array.from(agents.values()).map((item) => ({
        name: item.summary.id,
        description: item.summary.description,
        mode: "primary",
        native: item.summary.source === "static",
        hidden: false,
        options: { source: item.summary.source, path: item.summary.path, capabilities: item.summary.capabilities },
        permission: { allow: ["*"], deny: [] },
      })))
    }
    if (req.method === "POST" && url.pathname === "/agent") {
      const payload = await readJson(req)
      records.agents.push({ method: "POST", payload })
      const id = payload.id || payload.config?.agent_name
      const item = {
        summary: {
          id,
          name: id,
          description: payload.config?.description || "Custom Tura agent",
          source: "dynamic",
          path: `agents/${id}`,
          aliases: [],
          capabilities: ["command_run"],
          provider: "fast",
          hidden: false,
        },
        config: payload.config || { agent_name: id },
        prompt: payload.prompt || "",
      }
      agents.set(id, item)
      return sendJson(res, item)
    }
    const agentMatch = url.pathname.match(/^\/agent\/([^/]+)$/)
    if (agentMatch) {
      const id = decodeURIComponent(agentMatch[1])
      if (req.method === "GET") return sendJson(res, agents.get(id) || { error: "not found" }, agents.has(id) ? 200 : 404)
      if (req.method === "PATCH" || req.method === "PUT") {
        const payload = await readJson(req)
        records.agents.push({ method: req.method, id, payload })
        const existing = agents.get(id) || { summary: { id, name: id, description: "", source: "dynamic", path: `agents/${id}`, aliases: [], capabilities: [], hidden: false }, config: { agent_name: id }, prompt: "" }
        const updated = { ...existing, config: payload.config || existing.config, prompt: payload.prompt ?? existing.prompt }
        agents.set(id, updated)
        return sendJson(res, updated)
      }
      if (req.method === "DELETE") {
        records.agents.push({ method: "DELETE", id })
        const deleted = agents.delete(id)
        return sendJson(res, deleted)
      }
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
      const id = payload.task_management ? `plan-local-${records.createSessions.length}` : session.id
      const planName = payload.task_management?.plan_summary ?? session.name
      session = {
        ...session,
        id,
        name: planName,
        directory: payload.directory ?? session.directory,
        model: payload.model ?? config.model,
        agent: payload.agent ?? config.active_agent,
        session_type: payload.session_type,
        model_variant: payload.model_variant ?? config.model_variant,
        model_acceleration_enabled: payload.model_acceleration_enabled ?? config.model_acceleration_enabled,
        force_multiple_tasks: payload.force_multiple_tasks,
        plan_summary: planName,
        session_display_name: planName,
        task_management: payload.task_management ?? session.task_management,
      }
      sessions = [session, ...sessions.filter((item) => item.id !== session.id)]
      return sendJson(res, session)
    }
    if (req.method === "GET" && url.pathname === "/session") return sendJson(res, sessions)
    if (req.method === "GET" && url.pathname === "/session/status") {
      return sendJson(res, Object.fromEntries(sessions.map((item) => [item.id, { status: { type: item.status }, task_management: item.task_management, plan_summary: item.plan_summary, session_display_name: item.session_display_name }])))
    }
    const sessionMatch = url.pathname.match(/^\/session\/([^/]+)$/)
    if (req.method === "GET" && sessionMatch) {
      const id = decodeURIComponent(sessionMatch[1])
      return sendJson(res, sessions.find((item) => item.id === id) ?? session)
    }
    if (req.method === "PATCH" && sessionMatch) {
      const id = decodeURIComponent(sessionMatch[1])
      const payload = await readJson(req)
      records.sessionUpdates.push(payload)
      const existing = sessions.find((item) => item.id === id) ?? session
      const taskState = payload.task_management ?? existing.task_management
      const planName = taskState?.plan_summary ?? payload.plan_summary ?? existing.plan_summary
      const updated = {
        ...existing,
        ...payload,
        name: planName ?? existing.name,
        plan_summary: planName,
        session_display_name: planName,
        task_management: { ...existing.task_management, ...taskState },
        updated_at: Date.now(),
      }
      session = updated
      sessions = sessions.map((item) => (item.id === id ? updated : item))
      return sendJson(res, updated)
    }
    if (req.method === "DELETE" && sessionMatch) return sendJson(res, true)
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

function parseCommandLine(input) {
  const args = []
  let current = ""
  let quote = null
  let escaping = false
  for (const char of input) {
    if (escaping) {
      current += char
      escaping = false
      continue
    }
    if (char === "\\") {
      escaping = true
      continue
    }
    if (quote) {
      if (char === quote) quote = null
      else current += char
      continue
    }
    if (char === "\"" || char === "'") {
      quote = char
      continue
    }
    if (/\s/.test(char)) {
      if (current) {
        args.push(current)
        current = ""
      }
      continue
    }
    current += char
  }
  if (escaping) current += "\\"
  if (quote) throw new Error(`unterminated ${quote} quote`)
  if (current) args.push(current)
  return args
}

function terminalPageHtml() {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>TUI CLI bridge</title>
  <style>
    * { box-sizing: border-box; }
    body { margin: 0; font: 14px/1.45 ui-monospace, SFMono-Regular, Consolas, monospace; background: #111417; color: #edf0f2; }
    main { min-height: 100vh; display: grid; grid-template-rows: auto auto auto 1fr; gap: 12px; padding: 18px; }
    header { display: flex; align-items: baseline; justify-content: space-between; gap: 16px; border-bottom: 1px solid #30363d; padding-bottom: 10px; }
    h1 { margin: 0; font: 700 18px/1.2 system-ui, sans-serif; letter-spacing: 0; }
    .meta { color: #9aa5ad; font: 12px/1.2 system-ui, sans-serif; }
    form { display: grid; grid-template-columns: minmax(0, 1fr) 92px; gap: 8px; align-items: stretch; }
    textarea { min-height: 78px; resize: vertical; border: 1px solid #48515a; border-radius: 6px; background: #171c21; color: inherit; padding: 10px; font: inherit; outline: none; }
    textarea:focus { border-color: #73c2fb; box-shadow: 0 0 0 2px rgba(115, 194, 251, .18); }
    button { border: 1px solid #73c2fb; border-radius: 6px; background: #73c2fb; color: #07111a; padding: 0 18px; font-weight: 700; }
    button:disabled { opacity: .55; }
    .status { min-height: 22px; color: #73c2fb; }
    pre { margin: 0; white-space: pre-wrap; overflow: auto; border: 1px solid #30363d; border-radius: 6px; background: #080b0e; padding: 12px; }
  </style>
</head>
<body>
  <main>
    <header>
      <h1>TUI CLI bridge</h1>
      <div class="meta">local command relay · playwright evidence</div>
    </header>
    <form id="terminal-form">
      <textarea id="command" aria-label="Command" spellcheck="false"></textarea>
      <button id="run" type="submit">Run</button>
    </form>
    <div id="status" class="status" role="status">ready</div>
    <pre id="output" aria-label="Terminal output"></pre>
  </main>
  <script>
    const form = document.querySelector("#terminal-form");
    const command = document.querySelector("#command");
    const output = document.querySelector("#output");
    const status = document.querySelector("#status");
    const run = document.querySelector("#run");
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      run.disabled = true;
      status.textContent = "running";
      output.textContent = "";
      try {
        const response = await fetch("/run", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ command: command.value }),
        });
        const result = await response.json();
        output.textContent = [
          "$ tura " + command.value,
          "status=" + result.status + " durationMs=" + result.durationMs,
          result.stdout ? "\\n[stdout]\\n" + result.stdout : "",
          result.stderr ? "\\n[stderr]\\n" + result.stderr : "",
          result.error ? "\\n[error]\\n" + result.error : "",
        ].filter(Boolean).join("\\n");
        status.textContent = result.status === 0 ? "completed" : "failed";
      } catch (error) {
        output.textContent = String(error && error.stack || error);
        status.textContent = "failed";
      } finally {
        run.disabled = false;
      }
    });
  </script>
</body>
</html>`
}

function startTerminalBridge(gateway) {
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url || "/", "http://127.0.0.1")
    if (req.method === "GET" && url.pathname === "/") {
      const body = terminalPageHtml()
      res.writeHead(200, { "content-type": "text/html; charset=utf-8", "content-length": Buffer.byteLength(body) })
      res.end(body)
      return
    }
    if (req.method === "POST" && url.pathname === "/run") {
      try {
        const payload = await readJson(req)
        const args = parseCommandLine(String(payload.command || ""))
        const result = await spawnCli([...baseArgs(gateway), ...args])
        return sendJson(res, result)
      } catch (error) {
        return sendJson(res, { status: 1, stdout: "", stderr: "", durationMs: 0, error: error.stack || error.message || String(error) }, 400)
      }
    }
    sendJson(res, { error: `unhandled ${req.method} ${url.pathname}` }, 404)
  })
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const address = server.address()
      resolve({ server, url: `http://127.0.0.1:${address.port}` })
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

async function runWebTerminalE2e(gateway) {
  const { chromium } = tuiRequire("playwright")
  const bridge = await startTerminalBridge(gateway)
  const browser = await chromium.launch({ headless: true })
  const page = await browser.newPage({ viewport: { width: 1100, height: 780 } })
  const screenshotDir = path.join(runRoot, "screenshots")
  const screenshots = []
  const capture = async (name) => {
    const screenshotPath = path.join(screenshotDir, `${String(screenshots.length).padStart(2, "0")}-${name}.png`)
    await page.screenshot({ path: screenshotPath, fullPage: true })
    screenshots.push({ name, path: screenshotPath })
  }
  const runStep = async (name, command, assertOutput) => {
    await page.getByLabel("Command").fill(command)
    await page.getByRole("button", { name: "Run" }).click()
    await page.waitForFunction(() => document.querySelector("#status")?.textContent !== "running", { timeout: 12_000 })
    const output = await page.getByLabel("Terminal output").innerText()
    assert.match(output, /status=0/, `${name} should exit successfully`)
    await assertOutput(output)
    await capture(name)
    return output
  }
  try {
    await fs.mkdir(screenshotDir, { recursive: true })
    await page.goto(bridge.url, { waitUntil: "domcontentloaded" })
    await capture("ready")

    await runStep("settings-config-get", "--json config get", async (output) => {
      assert.match(output, /coding_agent_fast|coding_agent/)
    })
    await runStep("settings-config-set", "config set agent=coding_agent model_variant=low", async (output) => {
      assert.match(output, /"active_agent":\s*"coding_agent"/)
      assert.match(output, /"model_variant":\s*"low"/)
    })
    await runStep("login-provider-list", "--json provider list", async (output) => {
      assert.match(output, /"id":\s*"openai"/)
    })
    await runStep("login-provider-status", "provider status openai", async (output) => {
      assert.match(output, /authenticated/)
    })
    await runStep("task-plan", "--json session plan --all", async (output) => {
      assert.match(output, /等待用户补充权限|需要审批后继续执行/)
    })
    await runStep("gateway-command", "command run hello web", async (output) => {
      assert.match(output, /ran hello web/)
    })
    await runStep("permission-feedback", "--json permission list", async (output) => {
      assert.match(output, /\[|\]/)
    })
    await runStep("run-feedback", 'run "hello from tui playwright bridge" --json --no-stream --timeout 5', async (text) => {
      assert.match(text, /hello from tui playwright bridge/)
      assert.match(text, /streaming-middle final/)
      assert.match(text, /"status":\s*"completed"/)
    })
    await runStep("settings-restore", "config set agent=coding_agent_fast model_variant=medium", async (output) => {
      assert.match(output, /"active_agent":\s*"coding_agent_fast"/)
      assert.match(output, /"model_variant":\s*"medium"/)
    })
    assert.ok(
      gateway.records.prompts.some((payload) =>
        payload.parts?.some((part) => part.text === "hello from tui playwright bridge"),
      ),
      "web terminal should send the prompt through the TUI CLI",
    )

    await fs.writeFile(path.join(screenshotDir, "manifest.json"), JSON.stringify(screenshots, null, 2))
    console.log(`[tui-cli-e2e] playwright web terminal ok=true screenshots=${screenshotDir}`)
  } finally {
    await browser.close()
    await new Promise((resolve) => bridge.server.close(resolve))
  }
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
      "model_variant=medium",
      "model_acceleration_enabled=true",
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
    const plan = await expectCliJson([...baseArgs(gateway), "--json", "session", "plan", "--all"])
    assert.ok(plan.tickets.some((ticket) => ticket.plan_summary === "等待用户补充权限" && ticket.status === "question"))
    assert.ok(!plan.tickets.some((ticket) => ticket.plan_summary === "隐藏旧会话工单"))
    const archivedPlan = await expectCliJson([...baseArgs(gateway), "--json", "session", "plan", "--all", "--archived", "--status", "archived"])
    assert.equal(archivedPlan.tickets[0].status, "archived")
    const statusUpdate = await expectCliJson([...baseArgs(gateway), "--json", "session", "set-status", "sess-e2e", "done"])
    assert.equal(statusUpdate.session.task_management.status, "done")
    const ticketUpdate = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "session",
      "create-ticket",
      "需要审批后继续执行",
      "--session",
      "sess-e2e",
      "--status",
      "question",
      "--start-condition",
      "scheduled_task",
      "--start-at",
      "2026-05-25T10:30",
      "--poll",
      "m=0,d=1,h=2,s=3",
      "--step",
      "2",
    ])
    assert.equal(ticketUpdate.session.task_management.status, "question")
    assert.equal(ticketUpdate.session.task_management.start_at, "2026-05-25T08:30:00.000Z")
    assert.equal(ticketUpdate.session.task_management.step, 2)
    const humanPlan = await expectCliOk([...baseArgs(gateway), "session", "plan", "--all"])
    assert.match(humanPlan.stdout, /command session plan/)
    assert.match(humanPlan.stdout, /result ok/)
    const sessionDelete = await expectCliJson([...baseArgs(gateway), "--json", "session", "delete", "sess-e2e"])
    assert.equal(sessionDelete.deleted, true)

    const providerList = await expectCliJson([...baseArgs(gateway), "--json", "provider", "list"])
    assert.equal(providerList.all[0].id, "openai")
    const gatewayClientModule = await import(pathToFileURL(path.join(repoRoot, "apps", "tui", "dist", "gateway", "client.js")).href)
    const authMethods = await new gatewayClientModule.GatewayClient({ baseUrl: gateway.url, directory: runRoot }).listProviderAuthMethods()
    assert.equal(authMethods.openai[0].login, "oauth")
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

    const agentList = await expectCliJson([...baseArgs(gateway), "--json", "agent", "list"])
    assert.equal(agentList[0].name, "coding_agent_fast")
    const agentCreate = await expectCliJson([
      ...baseArgs(gateway),
      "--json",
      "agent",
      "create",
      "cli_agent",
      "--prompt",
      "Use command_run.",
    ])
    assert.equal(agentCreate.summary.id, "cli_agent")
    const agentShow = await expectCliJson([...baseArgs(gateway), "--json", "agent", "show", "cli_agent"])
    assert.equal(agentShow.summary.id, "cli_agent")
    const agentDelete = await expectCliJson([...baseArgs(gateway), "--json", "agent", "delete", "cli_agent"])
    assert.equal(agentDelete.deleted, true)

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
      "--model-reasoning-effort",
      "high",
      "--no-model-acceleration",
      "--force-multiple-tasks",
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
    assert.equal(events[0].type, "cli.started")
    assert.equal(events[0].prompt, "hello from tui cli")
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

    await runWebTerminalE2e(gateway)

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
    assert.equal(await client.sessionStatus("sess-e2e"), "idle")
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
    assert.equal((await client.listAgents())[0].name, "coding_agent_fast")
    assert.equal((await client.createAgent({ id: "client_agent" })).summary.id, "client_agent")
    assert.equal((await client.updateAgent("client_agent", { prompt: "updated" })).summary.id, "client_agent")
    assert.equal((await client.getAgent("client_agent")).summary.id, "client_agent")
    assert.equal(await client.deleteAgent("client_agent"), true)
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
