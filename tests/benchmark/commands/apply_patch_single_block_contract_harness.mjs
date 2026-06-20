#!/usr/bin/env node
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { businessRunPaths, normalizeBusinessSummary } from "../lib/business_paths.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..")
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `apply-patch-single-block-${Date.now()}`
const runPaths = businessRunPaths("benchmark-commands-apply-patch-single-block", runId)
const runRoot = runPaths.run_root
const summaryPath = runPaths.summary_path

const trialsPerGroup = Number(process.env.COMMAND_RUN_APPLY_PATCH_TRIALS || 10)
const concurrency = Number(process.env.COMMAND_RUN_APPLY_PATCH_CONCURRENCY || 2)
const variants = parseVariants(process.env.COMMAND_RUN_APPLY_PATCH_VARIANTS || "current,single-block")
const providerName = process.env.COMMAND_RUN_APPLY_PATCH_PROVIDER || "codex"
const model = normalizeModelForProvider(
  process.env.COMMAND_RUN_APPLY_PATCH_MODEL ||
    process.env.COMMAND_RUN_AGENT_TURA_MODEL ||
    "codex/gpt-5.3-codex-spark",
  providerName,
)
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const serviceTier =
  process.env.COMMAND_RUN_AGENT_SERVICE_TIER ||
  process.env.COMMAND_RUN_AGENT_CODEX_SERVICE_TIER ||
  "priority"
const requestTimeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 180_000)

function parseVariants(value) {
  const allowed = new Set(["current", "single-block"])
  const parsed = String(value)
    .split(",")
    .map((item) => item.trim().toLowerCase())
    .filter((item) => allowed.has(item))
  return parsed.length ? parsed : ["current", "single-block"]
}

function normalizeModelForProvider(value, provider) {
  const text = String(value || "").trim()
  if (!text) return "gpt-5.3-codex-spark"
  const slash = text.indexOf("/")
  if (slash < 0) return text
  const prefix = text.slice(0, slash)
  const suffix = text.slice(slash + 1)
  if (prefix === provider || (provider === "codex" && prefix === "openai")) return suffix
  return text
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeFile(file, text) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, text, "utf8")
}

function writeJson(file, value) {
  writeFile(file, JSON.stringify(value, null, 2))
}

function readFile(file) {
  return fs.readFileSync(file, "utf8")
}

function loadDotEnv(file) {
  if (!fs.existsSync(file)) return
  for (const line of readFile(file).split(/\r?\n/)) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith("#")) continue
    const match = trimmed.match(/^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/)
    if (!match || process.env[match[1]]) continue
    let value = match[2].trim()
    if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
      value = value.slice(1, -1)
    }
    process.env[match[1]] = value
  }
}

function copyDir(src, dst) {
  mkdirp(dst)
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const from = path.join(src, entry.name)
    const to = path.join(dst, entry.name)
    if (entry.isDirectory()) copyDir(from, to)
    else fs.copyFileSync(from, to)
  }
}

function providerEndpoint(provider) {
  if (process.env.COMMAND_RUN_APPLY_PATCH_BASE_URL) return process.env.COMMAND_RUN_APPLY_PATCH_BASE_URL
  if (provider === "codex") return "https://chatgpt.com/backend-api/codex/responses"
  if (provider === "qwen") return "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/responses"
  if (provider === "openrouter") return "https://openrouter.ai/api/v1/responses"
  return "https://api.openai.com/v1/responses"
}

function readCodexAuth() {
  const home = process.env.USERPROFILE || process.env.HOME
  if (!home) return { accessToken: null, accountId: null }
  try {
    const value = JSON.parse(readFile(path.join(home, ".codex", "auth.json")))
    const tokens = value.tokens || {}
    return {
      accessToken: tokens.access_token || null,
      accountId: tokens.account_id || value.account_id || null,
    }
  } catch {
    return { accessToken: null, accountId: null }
  }
}

function providerApiKey(provider) {
  if (process.env.COMMAND_RUN_APPLY_PATCH_API_KEY) return process.env.COMMAND_RUN_APPLY_PATCH_API_KEY
  if (provider === "qwen") return process.env.QWEN_API_KEY
  if (provider === "openrouter") return process.env.OPENROUTER_API_KEY
  if (provider === "codex") return process.env.OPENAI_API_KEY || readCodexAuth().accessToken
  return process.env.OPENAI_API_KEY
}

function providerAccountId(provider) {
  if (process.env.OPENAI_ACCOUNT_ID) return process.env.OPENAI_ACCOUNT_ID
  return provider === "codex" ? readCodexAuth().accountId : null
}

function createFixtureTemplate(template) {
  fs.rmSync(template, { recursive: true, force: true })
  mkdirp(path.join(template, "src", "components"))
  mkdirp(path.join(template, "src", "games"))
  writeFile(path.join(template, "src", "App.jsx"), heavyApp())
  writeFile(path.join(template, "src", "styles.css"), heavyStyles())
  writeFile(path.join(template, "src", "games", "catalog.js"), heavyCatalog())
  writeFile(path.join(template, "src", "components", "GameCard.jsx"), heavyGameCard())
  writeFile(path.join(template, "src", "components", "StatusStrip.jsx"), heavyStatusStrip())
  writeFile(path.join(template, "README.md"), "# Apply Patch Single Block Fixture\n")
  return template
}

function heavyApp() {
  return [
    "import React, { useMemo, useState } from 'react'",
    "import { games, tuningNotes, missingGames } from './games/catalog.js'",
    "import { GameCard } from './components/GameCard.jsx'",
    "import { StatusStrip } from './components/StatusStrip.jsx'",
    "",
    "const filters = ['All', 'Classic', 'Prototype', 'Needs polish']",
    "",
    "export default function App() {",
    "  const [active, setActive] = useState('Snake')",
    "  const [filter, setFilter] = useState('All')",
    "  const visibleGames = useMemo(() => {",
    "    if (filter === 'All') return games",
    "    if (filter === 'Needs polish') return games.filter((game) => game.todo.length > 0)",
    "    return games.filter((game) => game.group === filter)",
    "  }, [filter])",
    "  const selected = games.find((game) => game.title === active) || games[0]",
    "",
    "  return (",
    "    <main className=\"arcade-shell\">",
    "      <header className=\"hero\">",
    "        <p className=\"eyebrow\">prototype control room</p>",
    "        <h1>Arcade Draft Board</h1>",
    "        <p className=\"lede\">Five games exist, but the platform entry is scattered and the next two games are only notes.</p>",
    "      </header>",
    "      <StatusStrip games={games} active={active} missing={missingGames} notes={tuningNotes} />",
    "      <nav className=\"filters\">",
    "        {filters.map((item) => (",
    "          <button key={item} className={filter === item ? 'active' : ''} onClick={() => setFilter(item)}>{item}</button>",
    "        ))}",
    "      </nav>",
    "      <section className=\"workspace\">",
    "        <aside className=\"game-list\">",
    "          {visibleGames.map((game) => (",
    "            <GameCard key={game.title} game={game} active={game.title === active} onSelect={() => setActive(game.title)} />",
    "          ))}",
    "        </aside>",
    "        <section className=\"detail-panel\">",
    "          <p className=\"eyebrow\">selected game</p>",
    "          <h2>{selected.title}</h2>",
    "          <p>{selected.summary}</p>",
    "          <div className=\"preview-grid\">",
    "            {selected.cells.map((cell, index) => <span key={index} className={`cell ${cell}`} />)}",
    "          </div>",
    "          <ul>",
    "            {selected.todo.map((item) => <li key={item}>{item}</li>)}",
    "          </ul>",
    "        </section>",
    "      </section>",
    "    </main>",
    "  )",
    "}",
    "",
  ].join("\n")
}

function heavyStyles() {
  return [
    ":root { font-family: Inter, ui-sans-serif, system-ui, sans-serif; background: #eef1f4; color: #172026; }",
    "* { box-sizing: border-box; }",
    "body { margin: 0; min-width: 320px; }",
    "button { font: inherit; }",
    ".arcade-shell { min-height: 100vh; padding: 28px; display: grid; gap: 18px; }",
    ".hero { background: white; border: 1px solid #d8dde5; padding: 20px; }",
    ".eyebrow { margin: 0 0 6px; text-transform: uppercase; letter-spacing: .16em; font-size: 12px; color: #64748b; }",
    "h1, h2 { margin: 0 0 8px; }",
    ".lede { max-width: 760px; }",
    ".status-strip { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 10px; }",
    ".status-tile { background: #fff; border: 1px solid #d8dde5; padding: 14px; }",
    ".filters { display: flex; flex-wrap: wrap; gap: 8px; }",
    ".filters button { border: 1px solid #9aa5b1; background: white; padding: 8px 12px; }",
    ".filters button.active { background: #172026; color: white; }",
    ".workspace { display: grid; grid-template-columns: minmax(280px, 420px) 1fr; gap: 18px; align-items: start; }",
    ".game-list { display: grid; gap: 10px; }",
    ".game-card { text-align: left; border: 1px solid #d8dde5; background: white; padding: 14px; }",
    ".game-card.active { border-color: #172026; box-shadow: 0 0 0 2px #172026 inset; }",
    ".game-card strong { display: block; }",
    ".game-card small { display: block; color: #64748b; }",
    ".detail-panel { background: white; border: 1px solid #d8dde5; padding: 18px; min-height: 520px; }",
    ".preview-grid { width: min(440px, 100%); display: grid; grid-template-columns: repeat(8, 1fr); gap: 4px; margin: 16px 0; }",
    ".cell { aspect-ratio: 1; background: #e2e8f0; }",
    ".cell.hero { background: #172026; }",
    ".cell.target { background: #eab308; }",
    ".cell.danger { background: #dc2626; }",
    "@media (max-width: 760px) { .workspace { grid-template-columns: 1fr; } .status-strip { grid-template-columns: 1fr 1fr; } }",
    "",
  ].join("\n")
}

function heavyCatalog() {
  return [
    "const baseCells = [",
    "  ['hero','hero','target','blank','blank','danger','blank','blank'],",
    "  ['blank','target','blank','blank','hero','blank','danger','blank'],",
    "  ['blank','blank','hero','blank','target','blank','blank','danger'],",
    "].flat()",
    "",
    "export const games = [",
    "  { title: 'Snake', group: 'Classic', summary: 'Snake has movement but lacks food feedback and restart polish.', cells: baseCells, todo: ['polish food pulse', 'upgrade game over copy'] },",
    "  { title: 'Pong', group: 'Classic', summary: 'Pong preview exists but paddles feel static.', cells: [...baseCells].reverse(), todo: ['tune paddle rebound', 'polish score strip'] },",
    "  { title: 'Memory', group: 'Prototype', summary: 'Memory cards flip, but matched states are unclear.', cells: baseCells.map((item, index) => index % 3 === 0 ? 'target' : item), todo: ['add matched badge', 'upgrade moves label'] },",
    "  { title: 'Asteroids', group: 'Prototype', summary: 'Asteroids has rocks and a ship but no danger rhythm.', cells: baseCells.map((item, index) => index % 4 === 0 ? 'danger' : item), todo: ['polish rock glow', 'tune thrust state'] },",
    "  { title: 'Runner', group: 'Prototype', summary: 'Runner has lanes but needs pace cues.', cells: baseCells.map((item, index) => index % 5 === 0 ? 'hero' : item), todo: ['upgrade lane speed', 'polish obstacle warning'] },",
    "]",
    "",
    "export const missingGames = ['Tetris', 'Breakout']",
    "export const tuningNotes = ['Arcade entry is incomplete', 'Responsive layout needs polish', 'Two games are still missing']",
    "",
  ].join("\n")
}

function heavyGameCard() {
  return [
    "export function GameCard({ game, active, onSelect }) {",
    "  return (",
    "    <button className={active ? 'game-card active' : 'game-card'} onClick={onSelect}>",
    "      <strong>{game.title}</strong>",
    "      <small>{game.group}</small>",
    "      <span>{game.todo.length} polish tasks</span>",
    "    </button>",
    "  )",
    "}",
    "",
  ].join("\n")
}

function heavyStatusStrip() {
  return [
    "export function StatusStrip({ games, active, missing, notes }) {",
    "  return (",
    "    <section className=\"status-strip\">",
    "      <div className=\"status-tile\"><strong>{games.length}</strong><span> playable drafts</span></div>",
    "      <div className=\"status-tile\"><strong>{active}</strong><span> selected</span></div>",
    "      <div className=\"status-tile\"><strong>{missing.join(', ')}</strong><span> missing</span></div>",
    "      <div className=\"status-tile\"><strong>{notes.length}</strong><span> notes</span></div>",
    "    </section>",
    "  )",
    "}",
    "",
  ].join("\n")
}

function fixtureExcerpts(workspace) {
  const files = [
    ["Current file: src/App.jsx", path.join(workspace, "src", "App.jsx"), "jsx"],
    ["Current file: src/styles.css", path.join(workspace, "src", "styles.css"), "css"],
    ["Current file: src/games/catalog.js", path.join(workspace, "src", "games", "catalog.js"), "js"],
    ["Current file: src/components/GameCard.jsx", path.join(workspace, "src", "components", "GameCard.jsx"), "jsx"],
    ["Current file: src/components/StatusStrip.jsx", path.join(workspace, "src", "components", "StatusStrip.jsx"), "jsx"],
  ]
  return files.flatMap(([label, file, lang]) => [label, `\`\`\`${lang}`, readFile(file), "```"]).join("\n")
}

function commandRunTool(variant) {
  const commandLine = {
    type: "string",
    maxLength: 10000,
    pattern:
      variant === "single-block"
        ? "^(?![\\s\\S]*\\n\\*\\*\\* (?:Update|Add|Delete) File: [^\\n]+[\\s\\S]*\\n\\*\\*\\* (?:Update|Add|Delete) File: )(?![\\s\\S]*\\n@@[\\s\\S]*\\n@@)\\*\\*\\* Begin Patch[\\s\\S]*\\*\\*\\* End Patch\\s*$"
        : "^\\*\\*\\* Begin Patch[\\s\\S]*\\*\\*\\* End Patch\\s*$",
    description:
      variant === "single-block"
        ? "Raw apply_patch body. It must contain exactly one file operation; update patches must contain exactly one @@ hunk/code block. Use another apply_patch command for each additional code block."
        : "Raw apply_patch body beginning with *** Begin Patch and ending with *** End Patch.",
  }
  return {
    type: "function",
    name: "command_run",
    description:
      variant === "single-block"
        ? "Run a batch of commands. command_run may contain many apply_patch commands, but each apply_patch command_line must edit exactly one file operation and one code block."
        : "Run a batch of commands. Current apply_patch guidance: command_line is a normal raw patch body.",
    parameters: {
      type: "object",
      required: ["commands"],
      additionalProperties: false,
      properties: {
        commands: {
          type: "array",
          minItems: 1,
          maxItems: 15,
          items: {
            type: "object",
            required: ["command_type", "command_line"],
            additionalProperties: false,
            properties: {
              command_type: { type: "string", enum: ["apply_patch"] },
              command_line: commandLine,
              step: { type: "integer", minimum: 1 },
            },
          },
        },
      },
    },
    strict: false,
  }
}

function runtimeMessages(variant, marker, workspace) {
  return [
    {
      role: "system",
      content: [
        "You are Codex, a coding agent. You must use the command_run tool.",
        "Use apply_patch for file edits. Do not use shell commands, prose-only answers, or markdown code fences.",
      ].join("\n"),
    },
    {
      role: "developer",
      content:
        "Filesystem sandboxing is disabled. Network is enabled. Approval policy is never. Do not provide sandbox_permissions.",
    },
    { role: "user", content: `<environment_context>\n  <cwd>${workspace}</cwd>\n  <shell>powershell</shell>\n</environment_context>` },
    { role: "user", content: userPrompt(variant, marker, workspace) },
  ]
}

function userPrompt(variant, marker, workspace) {
  const variantRules =
    variant === "single-block"
      ? [
          "Single-block apply_patch contract for this run:",
          "- command_run may contain multiple commands in its commands array.",
          "- Every command must be command_type apply_patch.",
          "- Each individual apply_patch command_line must contain exactly one file operation.",
          "- If the command_line uses *** Update File, it must contain exactly one @@ hunk/code block.",
          "- If you need to edit two code blocks or two files, emit two separate apply_patch commands in the same command_run batch.",
        ]
      : [
          "Current apply_patch contract for this run:",
          "- command_run may contain multiple commands in its commands array.",
          "- Every command must be command_type apply_patch.",
          "- Each command_line must be a raw patch beginning with *** Begin Patch and ending with *** End Patch.",
        ]
  return [
    `Benchmark marker: ${marker}`,
    "You are in a disposable heavy React arcade workspace with several files and existing technical debt.",
    "Make one realistic implementation move now, using only apply_patch.",
    "The changed source must contain the literal benchmark marker and the word polish.",
    "Improve the arcade platform toward this goal:",
    "- Keep the existing five games: Snake, Pong, Memory, Asteroids, and Runner.",
    "- Add visible references for missing games Tetris and Breakout.",
    "- Improve status copy, selected-game detail copy, or responsive UI hierarchy.",
    "- It is acceptable to complete only a focused slice in this first move.",
    "",
    ...variantRules,
    "",
    "Important source excerpts are included so the first response can patch directly:",
    fixtureExcerpts(workspace),
  ].join("\n")
}

function buildRequest(messages, variant) {
  const request = {
    model,
    instructions: "Follow the user request and answer concisely.",
    input: messages.map((message) => ({
      role: ["system", "developer", "assistant"].includes(message.role) ? message.role : "user",
      content: [{ type: message.role === "assistant" ? "output_text" : "input_text", text: message.content }],
    })),
    stream: true,
    tools: [commandRunTool(variant)],
    tool_choice: { type: "function", name: "command_run" },
    parallel_tool_calls: false,
    store: false,
  }
  if (reasoning && reasoning !== "default") {
    request.reasoning = { effort: reasoning }
    if (["codex", "openai", "chatgpt"].includes(providerName)) request.include = ["reasoning.encrypted_content"]
  }
  if (serviceTier && serviceTier !== "default" && ["codex", "openai", "chatgpt"].includes(providerName)) {
    request.service_tier = serviceTier
  }
  return request
}

async function callProvider(request) {
  const endpoint = providerEndpoint(providerName)
  const apiKey = providerApiKey(providerName)
  if (!apiKey) throw new Error(`missing API key for provider ${providerName}`)
  const headers = { Authorization: `Bearer ${apiKey}`, "Content-Type": "application/json" }
  if (providerName === "codex") {
    headers.originator = "codex_cli_rs"
    headers["User-Agent"] = "codex_cli_rs/0.0.0 (Windows 10.0; x86_64)"
    headers.session_id = "tura-apply-patch-single-block"
    const accountId = providerAccountId(providerName)
    if (accountId) headers["ChatGPT-Account-Id"] = accountId
  }
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), requestTimeoutMs)
  const started = performance.now()
  try {
    const response = await fetch(endpoint, {
      method: "POST",
      headers,
      body: JSON.stringify(request),
      signal: controller.signal,
    })
    const text = await response.text()
    return {
      ok: response.ok,
      status: response.status,
      status_text: response.statusText,
      endpoint,
      duration_ms: Math.round(performance.now() - started),
      body: parseProviderBody(text, response.headers.get("content-type") || ""),
    }
  } catch (error) {
    return {
      ok: false,
      status: null,
      status_text: null,
      endpoint,
      duration_ms: Math.round(performance.now() - started),
      error: String(error.stack || error.message || error),
      body: null,
    }
  } finally {
    clearTimeout(timer)
  }
}

function parseProviderBody(text, contentType) {
  if (contentType.includes("text/event-stream") || text.includes("\ndata:")) return parseSseResponse(text)
  try {
    return JSON.parse(text)
  } catch {
    return { raw_text: text }
  }
}

function parseSseResponse(text) {
  let completed = null
  const events = []
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trimStart()
    if (!line.startsWith("data:")) continue
    const data = line.slice("data:".length).trim()
    if (!data || data === "[DONE]") continue
    try {
      const event = JSON.parse(data)
      if (event.response) completed = event.response
      events.push(event)
    } catch {
      events.push({ raw_data: data })
    }
  }
  return { ...(completed || {}), events }
}

function parseMaybeJson(value) {
  if (!value) return null
  if (typeof value === "object") return value
  if (typeof value !== "string") return null
  try {
    return JSON.parse(value)
  } catch {
    return null
  }
}

function collectCommandRunCalls(value, calls = [], seen = new Set()) {
  if (!value || typeof value !== "object") return calls
  if (Array.isArray(value)) {
    for (const item of value) collectCommandRunCalls(item, calls, seen)
    return calls
  }
  const name = value.name || value.tool_name || value.function?.name || (value.type === "function_call" ? value.name : null)
  const args = value.arguments ?? value.input ?? value.function?.arguments ?? value.args ?? null
  const hasUsableArgs =
    args !== null &&
    !(typeof args === "string" && args.trim() === "") &&
    !(typeof args === "object" && Object.keys(args).length === 0)
  if (name === "command_run" && hasUsableArgs) {
    const key = `${value.call_id || value.id || ""}\u0000${JSON.stringify(args)}`
    if (!seen.has(key)) {
      seen.add(key)
      calls.push({ name, arguments: args })
    }
  }
  for (const child of Object.values(value)) collectCommandRunCalls(child, calls, seen)
  return calls
}

function commandsFromCall(call) {
  const args = parseMaybeJson(call?.arguments)
  if (!args || typeof args !== "object") return { args, commands: [] }
  return { args, commands: Array.isArray(args.commands) ? args.commands.filter((item) => item && typeof item === "object") : [] }
}

function parsePatch(text) {
  const lines = String(text || "").split(/\r?\n/)
  const changes = []
  let current = null
  let hunk = null
  let started = false
  let ended = false
  function finish() {
    if (hunk && current) current.hunks.push(hunk)
    hunk = null
    if (current) changes.push(current)
    current = null
  }
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]
    if (!started) {
      if (line.trim() === "") continue
      if (line === "*** Begin Patch") {
        started = true
        continue
      }
      return { ok: false, error: `expected Begin Patch at line ${index + 1}` }
    }
    if (line.startsWith("*** Update File: ")) {
      finish()
      current = { kind: "update", path: line.slice(17), hunks: [], lines: [] }
    } else if (line.startsWith("*** Add File: ")) {
      finish()
      current = { kind: "add", path: line.slice(14), hunks: [], lines: [] }
    } else if (line.startsWith("*** Delete File: ")) {
      finish()
      current = { kind: "delete", path: line.slice(17), hunks: [], lines: [] }
    } else if (line.startsWith("@@")) {
      if (!current || current.kind !== "update") return { ok: false, error: "hunk without update file" }
      if (hunk) current.hunks.push(hunk)
      hunk = []
    } else if (line.startsWith("*** End Patch")) {
      finish()
      ended = true
      break
    } else if (current?.kind === "add" && line.startsWith("+")) {
      current.lines.push(line.slice(1))
    } else if (hunk && /^[ +\-]/.test(line)) {
      hunk.push(line)
    } else if (line.trim() !== "" && line !== "*** End of File") {
      return { ok: false, error: `invalid patch line ${index + 1}` }
    }
  }
  if (!started) return { ok: false, error: "missing Begin Patch" }
  if (!ended) return { ok: false, error: "missing End Patch" }
  if (!changes.length) return { ok: false, error: "no changes" }
  return { ok: true, changes }
}

function validatePatchCommand(command) {
  const raw = String(command.command_line || "")
  const parsed = parsePatch(raw.trim())
  const changeCount = parsed.ok ? parsed.changes.length : 0
  const hunkCount = parsed.ok ? parsed.changes.reduce((total, change) => total + change.hunks.length, 0) : 0
  return {
    command_type: command.command_type || command.command || null,
    command_line_chars: raw.length,
    under_10000_chars: raw.length <= 10000,
    parse_ok: parsed.ok,
    error: parsed.ok ? null : parsed.error,
    change_count: changeCount,
    hunk_count: hunkCount,
    one_file_or_block: parsed.ok && changeCount === 1 && hunkCount <= 1,
    paths: parsed.ok ? parsed.changes.map((change) => change.path) : [],
    excerpt: raw.slice(0, 500),
  }
}

function summarizeValidity(calls, variant) {
  const first = calls[0] || null
  const { args, commands } = commandsFromCall(first)
  const validations = commands.map(validatePatchCommand)
  const shapeOk = Boolean(args && Array.isArray(args.commands) && commands.length >= 1)
  const runtimeOk =
    calls.length === 1 &&
    shapeOk &&
    commands.length > 0 &&
    validations.every((item) => item.command_type === "apply_patch" && item.parse_ok && item.under_10000_chars)
  const singleBlockOk = runtimeOk && validations.every((item) => item.one_file_or_block)
  return {
    command_run_call_count: calls.length,
    command_count: commands.length,
    shape_ok: shapeOk,
    runtime_ok: runtimeOk,
    single_block_ok: singleBlockOk,
    expected_contract_ok: variant === "single-block" ? singleBlockOk : runtimeOk,
    validations,
  }
}

function failedTrialSummary(variant, index, error) {
  const trialName = `${variant}-${String(index + 1).padStart(2, "0")}`
  const trialRoot = path.join(runRoot, variant, trialName)
  mkdirp(trialRoot)
  const summary = {
    id: trialName,
    variant,
    index: index + 1,
    provider: providerName,
    model,
    http_ok: false,
    provider_error: String(error.stack || error.message || error),
    validity: { expected_contract_ok: false, runtime_ok: false, single_block_ok: false, validations: [] },
    ok: false,
  }
  writeJson(path.join(trialRoot, "trial-summary.json"), summary)
  return summary
}

async function runTrial(variant, template, index) {
  const trialName = `${variant}-${String(index + 1).padStart(2, "0")}`
  const trialRoot = path.join(runRoot, variant, trialName)
  const workspace = path.join(trialRoot, "workspace")
  const marker = `APPLY_PATCH_${variant.toUpperCase().replace("-", "_")}_${index + 1}_${Date.now()}`
  fs.rmSync(trialRoot, { recursive: true, force: true })
  copyDir(template, workspace)
  const messages = runtimeMessages(variant, marker, workspace)
  const request = buildRequest(messages, variant)
  writeJson(path.join(trialRoot, "request.json"), request)
  const started = performance.now()
  const provider = await callProvider(request)
  writeJson(path.join(trialRoot, "response.json"), provider)
  const calls = collectCommandRunCalls(provider.body)
  const validity = summarizeValidity(calls, variant)
  writeJson(path.join(trialRoot, "validity.json"), validity)
  const summary = {
    id: trialName,
    variant,
    index: index + 1,
    marker,
    workspace,
    duration_ms: Math.round(performance.now() - started),
    provider: providerName,
    model,
    endpoint: provider.endpoint,
    http_ok: provider.ok,
    http_status: provider.status,
    http_status_text: provider.status_text,
    provider_error: provider.error || null,
    validity,
    ok: provider.ok && validity.expected_contract_ok,
  }
  writeJson(path.join(trialRoot, "trial-summary.json"), summary)
  return summary
}

async function runWithLimit(tasks, limit) {
  const results = []
  let next = 0
  async function worker() {
    for (;;) {
      const index = next++
      if (index >= tasks.length) return
      try {
        results[index] = await tasks[index].run()
      } catch (error) {
        results[index] = failedTrialSummary(tasks[index].variant, tasks[index].index, error)
      }
    }
  }
  await Promise.all(Array.from({ length: Math.max(1, Math.min(limit, tasks.length)) }, () => worker()))
  return results
}

function groupSummary(results, variant) {
  const group = results.filter((item) => item.variant === variant)
  const patches = group.flatMap((item) => item.validity.validations || [])
  return {
    variant,
    trials: group.length,
    ok_trials: group.filter((item) => item.ok).length,
    http_successes: group.filter((item) => item.http_ok).length,
    runtime_ok_trials: group.filter((item) => item.validity.runtime_ok).length,
    single_block_trials: group.filter((item) => item.validity.single_block_ok).length,
    expected_contract_rate: group.length ? group.filter((item) => item.ok).length / group.length : 0,
    single_block_rate: group.length ? group.filter((item) => item.validity.single_block_ok).length / group.length : 0,
    total_commands: group.reduce((total, item) => total + (item.validity.command_count || 0), 0),
    patch_count: patches.length,
    one_file_or_block_patches: patches.filter((item) => item.one_file_or_block).length,
    multi_hunk_patches: patches.filter((item) => item.hunk_count > 1).length,
    multi_file_patches: patches.filter((item) => item.change_count > 1).length,
    max_command_line_chars: patches.length ? Math.max(...patches.map((item) => item.command_line_chars)) : 0,
  }
}

async function main() {
  loadDotEnv(path.join(repoRoot, ".env"))
  fs.rmSync(runRoot, { recursive: true, force: true })
  mkdirp(runRoot)
  const template = createFixtureTemplate(path.join(runRoot, "template"))
  const tasks = []
  for (let index = 0; index < trialsPerGroup; index += 1) {
    for (const variant of variants) tasks.push({ variant, index, run: () => runTrial(variant, template, index) })
  }
  const started = performance.now()
  const results = await runWithLimit(tasks, concurrency)
  const groups = variants.map((variant) => groupSummary(results, variant))
  const summary = normalizeBusinessSummary({
    ok: groups.every((group) => group.ok_trials === group.trials),
    run_id: runId,
    run_root: runRoot,
    provider: providerName,
    model,
    endpoint: providerEndpoint(providerName),
    reasoning,
    timeout_ms: requestTimeoutMs,
    trials_per_group: trialsPerGroup,
    concurrency,
    variants,
    duration_ms: Math.round(performance.now() - started),
    groups,
    results,
    notes: [
      "Compares current apply_patch guidance against a single-block JSON/prompt contract.",
      "command_run is always a batch with commands[]. Only the per-command apply_patch contract changes.",
      "The benchmark validates generated JSON/tool shape and patch syntax; it does not execute patches.",
    ],
  }, runPaths)
  writeJson(summaryPath, summary)
  console.log(JSON.stringify(summary, null, 2))
  const failOnInvalid = process.env.COMMAND_RUN_APPLY_PATCH_FAIL_ON_INVALID === "1"
  process.exit(failOnInvalid && !summary.ok ? 1 : 0)
}

main().catch((error) => {
  mkdirp(runRoot)
  const summary = normalizeBusinessSummary({
    ok: false,
    run_id: runId,
    run_root: runRoot,
    error: String(error.stack || error.message || error),
  }, runPaths)
  writeJson(summaryPath, summary)
  console.error(error.stack || error.message || error)
  process.exit(1)
})
