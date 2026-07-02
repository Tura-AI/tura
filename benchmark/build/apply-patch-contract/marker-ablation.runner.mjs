#!/usr/bin/env node
import assert from "node:assert/strict"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { businessRunPaths, normalizeBusinessSummary } from "../../../lib/business_paths.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..")
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `apply-patch-marker-ablation-${Date.now()}`
const runPaths = businessRunPaths("benchmark-commands-apply-patch-marker-ablation", runId)
const runRoot = runPaths.run_root
const summaryPath = runPaths.summary_path

const trialsPerGroup = Number(process.env.COMMAND_RUN_APPLY_PATCH_TRIALS || 10)
const concurrency = Number(process.env.COMMAND_RUN_APPLY_PATCH_CONCURRENCY || 2)
const variants = parseVariants(process.env.COMMAND_RUN_APPLY_PATCH_VARIANTS || "current,markerless")
const providerName = process.env.COMMAND_RUN_APPLY_PATCH_PROVIDER || "codex"
const model = normalizeModelForProvider(
  process.env.COMMAND_RUN_APPLY_PATCH_MODEL || process.env.COMMAND_RUN_AGENT_TURA_MODEL || "codex/gpt-5.5",
  providerName,
)
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "low"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || process.env.COMMAND_RUN_AGENT_CODEX_SERVICE_TIER || "priority"
const requestTimeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 180_000)

function parseVariants(value) {
  const allowed = new Set(["current", "markerless"])
  const parsed = String(value)
    .split(",")
    .map((item) => item.trim().toLowerCase())
    .filter((item) => allowed.has(item))
  return parsed.length > 0 ? parsed : ["current", "markerless"]
}

function normalizeModelForProvider(value, provider) {
  const text = String(value || "").trim()
  if (!text) return provider === "codex" ? "gpt-5.5" : "gpt-5.5"
  const slash = text.indexOf("/")
  if (slash === -1) return text
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

function copyDir(src, dst) {
  mkdirp(dst)
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const from = path.join(src, entry.name)
    const to = path.join(dst, entry.name)
    if (entry.isDirectory()) copyDir(from, to)
    else fs.copyFileSync(from, to)
  }
}

function loadDotEnv(file) {
  if (!fs.existsSync(file)) return
  for (const line of fs.readFileSync(file, "utf8").split(/\r?\n/)) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith("#")) continue
    const match = trimmed.match(/^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/)
    if (!match || process.env[match[1]]) continue
    let value = match[2].trim()
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1)
    }
    process.env[match[1]] = value
  }
}

function providerEndpoint(provider) {
  const explicit = process.env.COMMAND_RUN_APPLY_PATCH_BASE_URL
  if (explicit) return explicit
  if (provider === "codex") return "https://chatgpt.com/backend-api/codex/responses"
  if (provider === "qwen") return "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/responses"
  if (provider === "openrouter") return "https://openrouter.ai/api/v1/responses"
  return "https://api.openai.com/v1/responses"
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
  if (provider === "codex") return readCodexAuth().accountId
  return null
}

function readCodexAuth() {
  const home = process.env.USERPROFILE || process.env.HOME
  if (!home) return { accessToken: null, accountId: null }
  const authPath = path.join(home, ".codex", "auth.json")
  try {
    const value = JSON.parse(fs.readFileSync(authPath, "utf8"))
    const tokens = value.tokens || {}
    return {
      accessToken: typeof tokens.access_token === "string" && tokens.access_token.trim()
        ? tokens.access_token
        : null,
      accountId: typeof tokens.account_id === "string" && tokens.account_id.trim()
        ? tokens.account_id
        : typeof value.account_id === "string" && value.account_id.trim()
          ? value.account_id
          : null,
    }
  } catch {
    return { accessToken: null, accountId: null }
  }
}

function createFixtureTemplate(template) {
  fs.rmSync(template, { recursive: true, force: true })
  mkdirp(template)
  writeFile(path.join(template, "package.json"), JSON.stringify({
    scripts: {
      dev: "vite --host 127.0.0.1",
      lint: "node tools/check-static.mjs",
    },
    dependencies: {
      "@vitejs/plugin-react": "latest",
      vite: "latest",
      react: "latest",
      "react-dom": "latest",
    },
    devDependencies: {},
  }, null, 2))
  writeFile(path.join(template, "index.html"), `<div id="root"></div><script type="module" src="/src/main.jsx"></script>\n`)
  writeFile(path.join(template, "README.md"), [
    "# Messy Arcade Fixture",
    "",
    "This disposable workspace starts as a half-finished multi-game arcade.",
    "The benchmark asks the model to emit command_run/apply_patch commands only.",
    "",
  ].join("\n"))
  writeFile(path.join(template, "src", "main.jsx"), [
    "import React from 'react'",
    "import { createRoot } from 'react-dom/client'",
    "import './styles.css'",
    "import App from './App.jsx'",
    "",
    "createRoot(document.getElementById('root')).render(<App />)",
    "",
  ].join("\n"))
  writeFile(path.join(template, "src", "App.jsx"), messyApp())
  writeFile(path.join(template, "src", "styles.css"), messyStyles())
  writeFile(path.join(template, "src", "games", "snakeState.js"), messySnakeState())
  writeFile(path.join(template, "src", "games", "arcadeDrafts.js"), messyArcadeDrafts())
  writeFile(path.join(template, "src", "components", "ScorePanel.jsx"), [
    "export function ScorePanel({ score = 0, best = 0 }) {",
    "  return <aside className=\"score-panel\"><b>{score}</b><span>best {best}</span></aside>",
    "}",
    "",
  ].join("\n"))
  writeFile(path.join(template, "src", "components", "DebugPanel.jsx"), messyDebugPanel())
  writeFile(path.join(template, "legacy", "old-snake.html"), [
    "<!doctype html>",
    "<title>Old Snake</title>",
    "<canvas id=\"snake\" width=\"240\" height=\"240\"></canvas>",
    "<script>var note = 'legacy copy kept only to make the fixture messy';</script>",
    "",
  ].join("\n"))
  writeFile(path.join(template, "docs", "notes.md"), [
    "# Backlog",
    "",
    "- The current app has five games with duplicated state, placeholder details, and inconsistent controls.",
    "- Need an arcade home page/entry shell.",
    "- Need two additional games besides the existing five.",
    "- Existing games that need detail work: Snake, Pong, Memory, Asteroids, Runner.",
    "- New games requested: Tetris and Breakout.",
    "- Avoid global CSS leaks when fixing the layout.",
    "",
  ].join("\n"))
  writeFile(path.join(template, "tools", "check-static.mjs"), [
    "import fs from 'node:fs'",
    "const app = fs.readFileSync('src/App.jsx', 'utf8')",
    "const css = fs.readFileSync('src/styles.css', 'utf8')",
    "const required = ['Arcade', 'Snake', 'Pong', 'Memory', 'Asteroids', 'Runner', 'Tetris', 'Breakout', 'APPLY_PATCH_ARCADE_SENTINEL']",
    "const missing = required.filter((item) => !app.includes(item) && !css.includes(item))",
    "if (missing.length) { console.error(`missing ${missing.join(', ')}`); process.exit(1) }",
    "if (!/detail|tune|polish|variant|upgrade/i.test(app + css)) { console.error('missing existing-game detail work marker'); process.exit(1) }",
    "console.log('static arcade markers present')",
    "",
  ].join("\n"))
  return template
}

function messyApp() {
  return [
    "import React, { useEffect, useMemo, useRef, useState } from 'react'",
    "import { nextSnake, makeSnakeBoard } from './games/snakeState.js'",
    "import { arcadeDrafts, arcadeTuningMatrix } from './games/arcadeDrafts.js'",
    "import { ScorePanel } from './components/ScorePanel.jsx'",
    "import { DebugPanel } from './components/DebugPanel.jsx'",
    "",
    "const SIZE = 12",
    "const initialSnake = { snake: [[4, 4], [3, 4], [2, 4]], food: [7, 7], dir: [1, 0], running: false, score: 0, dead: false }",
    "const initialPong = { ball: [5, 5], velocity: [1, 1], left: 4, right: 4, score: [0, 0], running: false }",
    "const initialMemory = { picked: [], matched: [], moves: 0, deck: ['A', 'B', 'C', 'A', 'B', 'C', 'D', 'D'], message: 'find pairs' }",
    "const initialAsteroids = { ship: [6, 6], angle: 0, rocks: [[2, 2], [9, 3], [7, 9]], shots: [], score: 0, running: false }",
    "const initialRunner = { lane: 1, obstacles: [[0, 6], [2, 10], [1, 14]], distance: 0, running: false, hit: false }",
    "const existingGames = ['Snake', 'Pong', 'Memory', 'Asteroids', 'Runner']",
    "const placeholderStats = [",
    "  { label: 'Snake detail', value: 'food never sparkles' },",
    "  { label: 'Pong detail', value: 'paddles feel flat' },",
    "  { label: 'Memory detail', value: 'matched cards need feedback' },",
    "  { label: 'Asteroids detail', value: 'rocks need danger states' },",
    "  { label: 'Runner detail', value: 'lanes need pace cues' },",
    "]",
    "",
    "function BadCell({ value, index }) {",
    "  const cls = value === 2 ? 'food cell' : value === 1 ? 'snake cell' : 'cell'",
    "  return <div key={index} className={cls}>{value === 2 ? '*' : ''}</div>",
    "}",
    "",
    "function MiniGrid({ title, children, tone = 'plain' }) {",
    "  return <section className={`mini-game ${tone}`}><h2>{title}</h2>{children}</section>",
    "}",
    "",
    "function PongPreview({ state, onNudge }) {",
    "  const rows = Array.from({ length: 10 }, (_, y) => y)",
    "  const cols = Array.from({ length: 12 }, (_, x) => x)",
    "  return (",
    "    <MiniGrid title=\"Pong\" tone=\"pong\">",
    "      <div className=\"micro-board wide\">",
    "        {rows.flatMap((y) => cols.map((x) => {",
    "          const paddle = (x === 0 && Math.abs(y - state.left) < 2) || (x === 11 && Math.abs(y - state.right) < 2)",
    "          const ball = x === state.ball[0] && y === state.ball[1]",
    "          return <span key={`${x}-${y}`} className={ball ? 'dot ball' : paddle ? 'dot paddle' : 'dot'} />",
    "        }))}",
    "      </div>",
    "      <p>Score {state.score[0]}:{state.score[1]} - paddles still need tuned rebound detail.</p>",
    "      <button onClick={onNudge}>Nudge</button>",
    "    </MiniGrid>",
    "  )",
    "}",
    "",
    "function MemoryPreview({ state, onPick }) {",
    "  return (",
    "    <MiniGrid title=\"Memory\" tone=\"memory\">",
    "      <div className=\"memory-grid\">",
    "        {state.deck.map((card, index) => {",
    "          const open = state.picked.includes(index) || state.matched.includes(index)",
    "          return <button key={index} onClick={() => onPick(index)}>{open ? card : '?'}</button>",
    "        })}",
    "      </div>",
    "      <p>{state.message} - {state.moves} moves, matched cards need polish.</p>",
    "    </MiniGrid>",
    "  )",
    "}",
    "",
    "function AsteroidsPreview({ state, onFire }) {",
    "  const cells = Array.from({ length: 64 }, (_, index) => [index % 8, Math.floor(index / 8)])",
    "  return (",
    "    <MiniGrid title=\"Asteroids\" tone=\"asteroids\">",
    "      <div className=\"micro-board asteroids-board\">",
    "        {cells.map(([x, y]) => {",
    "          const ship = x === state.ship[0] && y === state.ship[1]",
    "          const rock = state.rocks.some(([rx, ry]) => rx === x && ry === y)",
    "          const shot = state.shots.some(([sx, sy]) => sx === x && sy === y)",
    "          return <span key={`${x}-${y}`} className={ship ? 'dot ship' : rock ? 'dot rock' : shot ? 'dot shot' : 'dot'} />",
    "        })}",
    "      </div>",
    "      <p>Rocks {state.rocks.length}; danger states are still placeholder.</p>",
    "      <button onClick={onFire}>Fire</button>",
    "    </MiniGrid>",
    "  )",
    "}",
    "",
    "function RunnerPreview({ state, onStep }) {",
    "  return (",
    "    <MiniGrid title=\"Runner\" tone=\"runner\">",
    "      <div className=\"runner-lanes\">",
    "        {[0, 1, 2].map((lane) => <div key={lane} className={lane === state.lane ? 'lane active' : 'lane'}>{state.obstacles.filter(([x]) => x === lane).map(([, y]) => <span key={y} style={{ top: `${Math.min(90, y * 7)}%` }} />)}</div>)}",
    "      </div>",
    "      <p>Distance {state.distance}; pace cues and collision detail are unfinished.</p>",
    "      <button onClick={onStep}>Dash</button>",
    "    </MiniGrid>",
    "  )",
    "}",
    "",
    "export default function App() {",
    "  const [game, setGame] = useState(initialSnake)",
    "  const [pong, setPong] = useState(initialPong)",
    "  const [memory, setMemory] = useState(initialMemory)",
    "  const [asteroids, setAsteroids] = useState(initialAsteroids)",
    "  const [runner, setRunner] = useState(initialRunner)",
    "  const [speed, setSpeed] = useState(125)",
    "  const [selected, setSelected] = useState('Snake')",
    "  const tickRef = useRef(null)",
    "  const board = useMemo(() => makeSnakeBoard(SIZE, game.snake, game.food), [game])",
    "  const gameCountLabel = useMemo(() => `${existingGames.length} prototypes`, [])",
    "",
    "  useEffect(() => {",
    "    function onKey(event) {",
    "      const dirs = { ArrowUp: [0, -1], ArrowDown: [0, 1], ArrowLeft: [-1, 0], ArrowRight: [1, 0] }",
    "      if (selected === 'Snake' && dirs[event.key]) setGame((g) => ({ ...g, dir: dirs[event.key], running: true }))",
    "      if (selected === 'Runner' && event.key === 'ArrowLeft') setRunner((g) => ({ ...g, lane: Math.max(0, g.lane - 1), running: true }))",
    "      if (selected === 'Runner' && event.key === 'ArrowRight') setRunner((g) => ({ ...g, lane: Math.min(2, g.lane + 1), running: true }))",
    "    }",
    "    window.addEventListener('keydown', onKey)",
    "    return () => window.removeEventListener('keydown', onKey)",
    "  }, [selected])",
    "",
    "  useEffect(() => {",
    "    clearInterval(tickRef.current)",
    "    if (game.running && !game.dead) tickRef.current = setInterval(() => setGame((g) => nextSnake(g, SIZE)), speed)",
    "    return () => clearInterval(tickRef.current)",
    "  }, [game.running, game.dead, speed])",
    "",
    "  function nudgePong() {",
    "    setPong((g) => ({ ...g, ball: [(g.ball[0] + g.velocity[0] + 12) % 12, (g.ball[1] + g.velocity[1] + 10) % 10], running: true }))",
    "  }",
    "",
    "  function pickMemory(index) {",
    "    setMemory((g) => ({ ...g, picked: g.picked.includes(index) ? g.picked : [...g.picked.slice(-1), index], moves: g.moves + 1, message: 'needs matched-card feedback' }))",
    "  }",
    "",
    "  function fireAsteroids() {",
    "    setAsteroids((g) => ({ ...g, shots: [[g.ship[0], Math.max(0, g.ship[1] - 1)], ...g.shots].slice(0, 4), running: true }))",
    "  }",
    "",
    "  function stepRunner() {",
    "    setRunner((g) => ({ ...g, distance: g.distance + 12, obstacles: g.obstacles.map(([x, y]) => [x, y - 1]).filter(([, y]) => y > -1).concat([[Math.floor(Math.random() * 3), 14]]), running: true }))",
    "  }",
    "",
    "  return (",
    "    <main className=\"page old-theme\">",
    "      <section className=\"hero-card\">",
    "        <p className=\"tiny\">prototype</p>",
    "        <h1>Messy Arcade Lab</h1>",
    "        <p>The layout is still a pile of {gameCountLabel}; it needs a real Arcade entry and two new games.</p>",
    "        <nav className=\"game-tabs\">",
    "          {existingGames.map((name) => <button key={name} className={selected === name ? 'active' : ''} onClick={() => setSelected(name)}>{name}</button>)}",
    "        </nav>",
    "        <ul className=\"detail-list\">",
    "          {placeholderStats.map((item) => <li key={item.label}><b>{item.label}</b><span>{item.value}</span></li>)}",
    "        </ul>",
    "        <DebugPanel items={arcadeDrafts} matrix={arcadeTuningMatrix} />",
    "      </section>",
    "      <section className=\"game-area\">",
    "        {selected === 'Snake' ? <>",
    "          <ScorePanel score={game.score} best={17} />",
    "          <div className=\"board\" style={{ '--size': SIZE }}>",
    "            {board.flatMap((row) => row).map((value, index) => <BadCell key={index} value={value} index={index} />)}",
    "          </div>",
    "          <div className=\"toolbar\">",
    "            <button onClick={() => setGame((g) => ({ ...g, running: !g.running }))}>Start</button>",
    "            <button onClick={() => setGame(initialSnake)}>Reset</button>",
    "            <label>Speed <input value={speed} onChange={(event) => setSpeed(Number(event.target.value) || 125)} /></label>",
    "          </div>",
    "          {game.dead ? <strong className=\"dead\">Game over</strong> : null}",
    "        </> : null}",
    "        {selected === 'Pong' ? <PongPreview state={pong} onNudge={nudgePong} /> : null}",
    "        {selected === 'Memory' ? <MemoryPreview state={memory} onPick={pickMemory} /> : null}",
    "        {selected === 'Asteroids' ? <AsteroidsPreview state={asteroids} onFire={fireAsteroids} /> : null}",
    "        {selected === 'Runner' ? <RunnerPreview state={runner} onStep={stepRunner} /> : null}",
    "      </section>",
    "    </main>",
    "  )",
    "}",
    "",
  ].join("\n")
}

function messyStyles() {
  return [
    ":root { font-family: Inter, ui-sans-serif, system-ui, sans-serif; color: #172018; background: #edf5e6; }",
    "* { box-sizing: border-box; }",
    "body { margin: 0; min-width: 320px; }",
    "button, input { font: inherit; }",
    ".page { min-height: 100vh; padding: 28px; display: grid; grid-template-columns: 300px 1fr; gap: 20px; }",
    ".hero-card { background: #ffffff; border: 2px solid #324b2d; border-radius: 18px; padding: 24px; box-shadow: 9px 9px 0 #b3ca9e; }",
    ".tiny { text-transform: uppercase; font-size: 12px; letter-spacing: .18em; color: #5b7157; }",
    "h1 { margin: 0 0 12px; font-size: clamp(36px, 7vw, 78px); letter-spacing: -0.06em; }",
    ".game-area { background: #fdfefb; border: 2px solid #324b2d; padding: 18px; min-width: 0; }",
    ".score-panel { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 12px; }",
    ".score-panel b { font-size: 40px; }",
    ".board { width: min(70vmin, 620px); display: grid; grid-template-columns: repeat(var(--size), 1fr); border: 5px solid #243923; background: #dce8cf; }",
    ".cell { aspect-ratio: 1; border: 1px solid rgba(28, 49, 24, .14); display: grid; place-items: center; }",
    ".snake { background: #315f34; }",
    ".food { background: #e05a47; color: white; }",
    ".toolbar { margin-top: 14px; display: flex; gap: 9px; align-items: center; flex-wrap: wrap; }",
    ".toolbar button { border: 2px solid #263820; background: #ffffff; padding: 9px 13px; }",
    ".toolbar input { width: 70px; }",
    ".dead { color: #b42318; display: block; margin-top: 12px; }",
    ".game-tabs { display: flex; gap: 8px; flex-wrap: wrap; margin: 18px 0; }",
    ".game-tabs button { border: 2px solid #263820; background: #fdfefb; padding: 8px 10px; }",
    ".game-tabs .active { background: #315f34; color: white; }",
    ".detail-list { list-style: none; margin: 18px 0 0; padding: 0; display: grid; gap: 8px; }",
    ".detail-list li { border-top: 1px solid #c4d4bb; padding-top: 8px; display: grid; gap: 2px; }",
    ".detail-list b { font-size: 13px; }",
    ".detail-list span { color: #5b7157; font-size: 12px; }",
    ".mini-game { display: grid; gap: 14px; align-content: start; min-height: 460px; }",
    ".mini-game h2 { margin: 0; font-size: 42px; }",
    ".mini-game p { margin: 0; max-width: 56ch; color: #4c6048; }",
    ".mini-game button { justify-self: start; border: 2px solid #263820; background: #ffffff; padding: 9px 13px; }",
    ".micro-board { width: min(62vmin, 520px); display: grid; grid-template-columns: repeat(8, 1fr); border: 5px solid #243923; background: #dce8cf; }",
    ".micro-board.wide { grid-template-columns: repeat(12, 1fr); }",
    ".dot { aspect-ratio: 1; border: 1px solid rgba(28, 49, 24, .12); display: block; }",
    ".ball { background: #e05a47; border-radius: 999px; }",
    ".paddle { background: #315f34; }",
    ".ship { background: #243923; clip-path: polygon(50% 0, 100% 100%, 50% 80%, 0 100%); }",
    ".rock { background: #8a8f72; border-radius: 30% 45% 35% 50%; }",
    ".shot { background: #e05a47; border-radius: 999px; }",
    ".memory-grid { display: grid; grid-template-columns: repeat(4, minmax(56px, 90px)); gap: 10px; }",
    ".memory-grid button { aspect-ratio: 1; background: #f8fbf2; border: 2px solid #263820; font-size: 28px; }",
    ".runner-lanes { width: min(62vmin, 480px); height: 460px; display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }",
    ".lane { position: relative; border: 2px solid #263820; background: #e6efd8; overflow: hidden; }",
    ".lane.active { background: #fdfefb; box-shadow: inset 0 0 0 4px #b3ca9e; }",
    ".lane span { position: absolute; left: 20%; right: 20%; height: 34px; border-radius: 6px; background: #e05a47; }",
    ".pong .micro-board { background: repeating-linear-gradient(90deg, #dce8cf 0 24px, #eef5e5 24px 48px); }",
    ".memory .memory-grid button:nth-child(odd) { transform: rotate(-1deg); }",
    ".asteroids .micro-board { background: #172018; }",
    ".asteroids .dot { border-color: rgba(255,255,255,.08); }",
    ".runner .lane:nth-child(2) { transform: translateY(5px); }",
    ".debug-panel { margin-top: 18px; border: 1px dashed #8fa384; padding: 12px; background: #f8fbf2; display: grid; gap: 10px; }",
    ".debug-title { margin: 0; font-weight: 800; text-transform: uppercase; font-size: 11px; letter-spacing: .12em; color: #5b7157; }",
    ".debug-list { display: grid; gap: 8px; }",
    ".debug-row { display: grid; gap: 3px; padding: 8px; background: white; border: 1px solid #d4e2c9; }",
    ".debug-row strong { font-size: 13px; }",
    ".debug-row span { font-size: 12px; color: #4c6048; }",
    ".debug-row small { font-size: 11px; color: #6f8069; }",
    ".debug-matrix { display: none; }",
    ".audit-badge { display: inline-flex; padding: 3px 7px; border: 1px solid #263820; font-size: 11px; }",
    ".placeholder-meter { display: inline-block; width: 80px; height: 8px; background: #dce8cf; border: 1px solid #263820; }",
    ".placeholder-meter i { display: block; height: 100%; background: #315f34; }",
    "@media (max-width: 760px) { .page { grid-template-columns: 1fr; padding: 14px; } .board { width: 100%; } h1 { letter-spacing: -0.03em; } }",
    "",
  ].join("\n")
}

function messySnakeState() {
  return [
    "export const arcadeTuningNotes = {",
    "  snake: 'food spawn may overlap the tail and needs a detail pass',",
    "  pong: 'ball rebound is flat and should gain angle detail',",
    "  memory: 'matched cards need lock and celebration detail',",
    "  asteroids: 'rocks do not drift and danger detail is missing',",
    "  runner: 'obstacle rhythm is too uniform and needs speed detail',",
    "}",
    "",
    "export function makeSnakeBoard(size, snake, food) {",
    "  const board = Array.from({ length: size }, () => Array.from({ length: size }, () => 0))",
    "  for (const [x, y] of snake) if (board[y] && board[y][x] !== undefined) board[y][x] = 1",
    "  board[food[1]][food[0]] = 2",
    "  return board",
    "}",
    "",
    "export function nextSnake(game, size) {",
    "  if (game.dead) return game",
    "  const head = game.snake[0]",
    "  const next = [head[0] + game.dir[0], head[1] + game.dir[1]]",
    "  const wall = next[0] < 0 || next[1] < 0 || next[0] >= size || next[1] >= size",
    "  const self = game.snake.some(([x, y]) => x === next[0] && y === next[1])",
    "  if (wall || self) return { ...game, dead: true, running: false }",
    "  const ate = next[0] === game.food[0] && next[1] === game.food[1]",
    "  const snake = [next, ...game.snake]",
    "  if (!ate) snake.pop()",
    "  const food = ate ? [Math.floor(Math.random() * size), Math.floor(Math.random() * size)] : game.food",
    "  return { ...game, snake, food, score: game.score + (ate ? 10 : 0) }",
    "}",
    "",
    "export function tickPong(game) {",
    "  const next = [game.ball[0] + game.velocity[0], game.ball[1] + game.velocity[1]]",
    "  const velocity = [...game.velocity]",
    "  if (next[1] <= 0 || next[1] >= 9) velocity[1] *= -1",
    "  if (next[0] <= 0 || next[0] >= 11) velocity[0] *= -1",
    "  return { ...game, ball: [(next[0] + 12) % 12, (next[1] + 10) % 10], velocity }",
    "}",
    "",
    "export function pickMemoryCard(game, index) {",
    "  if (game.matched.includes(index)) return game",
    "  const picked = game.picked.includes(index) ? game.picked : [...game.picked.slice(-1), index]",
    "  const pair = picked.length === 2 && game.deck[picked[0]] === game.deck[picked[1]]",
    "  return {",
    "    ...game,",
    "    picked: pair ? [] : picked,",
    "    matched: pair ? [...game.matched, ...picked] : game.matched,",
    "    moves: game.moves + 1,",
    "    message: pair ? 'pair found but celebration still plain' : 'keep looking',",
    "  }",
    "}",
    "",
    "export function tickAsteroids(game) {",
    "  const rocks = game.rocks.map(([x, y], index) => [(x + (index % 2 ? 1 : 0)) % 8, (y + 1) % 8])",
    "  const shots = game.shots.map(([x, y]) => [x, y - 1]).filter(([, y]) => y >= 0)",
    "  return { ...game, rocks, shots }",
    "}",
    "",
    "export function tickRunner(game) {",
    "  const obstacles = game.obstacles",
    "    .map(([lane, y]) => [lane, y - 1])",
    "    .filter(([, y]) => y >= 0)",
    "  const refill = obstacles.length < 4 ? [[Math.floor(Math.random() * 3), 14]] : []",
    "  const hit = obstacles.some(([lane, y]) => lane === game.lane && y <= 1)",
    "  return { ...game, obstacles: obstacles.concat(refill), distance: game.distance + 1, hit }",
    "}",
    "",
    "export function arcadeChecklist() {",
    "  return ['Snake detail', 'Pong detail', 'Memory detail', 'Asteroids detail', 'Runner detail', 'Arcade entry', 'Tetris', 'Breakout']",
    "}",
    "",
  ].join("\n")
}

function messyArcadeDrafts() {
  return [
    "export const arcadeDrafts = [",
    "  {",
    "    id: 'snake',",
    "    title: 'Snake',",
    "    status: 'playable but plain',",
    "    detail: 'food has no sparkle and the death state is abrupt',",
    "    controls: ['Arrow keys', 'Start', 'Reset'],",
    "    metrics: { score: 0, best: 17, pace: '125ms', risk: 'wall collision' },",
    "    todos: ['polish food', 'show turn buffer', 'add variant badge', 'tune restart copy'],",
    "  },",
    "  {",
    "    id: 'pong',",
    "    title: 'Pong',",
    "    status: 'prototype preview',",
    "    detail: 'paddles need rebound angles and a visible serve state',",
    "    controls: ['Nudge', 'W/S planned', 'Up/Down planned'],",
    "    metrics: { score: '0:0', best: 5, pace: 'manual', risk: 'flat bounce' },",
    "    todos: ['tune paddle feedback', 'add center line', 'add serve label', 'polish ball trail'],",
    "  },",
    "  {",
    "    id: 'memory',",
    "    title: 'Memory',",
    "    status: 'cards flip',",
    "    detail: 'matched cards disappear into the same state as selected cards',",
    "    controls: ['Click cards', 'Reset planned'],",
    "    metrics: { moves: 0, pairs: 4, pace: 'turn based', risk: 'unclear match' },",
    "    todos: ['upgrade match feedback', 'show pair count', 'tune card states', 'polish win copy'],",
    "  },",
    "  {",
    "    id: 'asteroids',",
    "    title: 'Asteroids',",
    "    status: 'static field',",
    "    detail: 'rocks have no drift variation or danger glow',",
    "    controls: ['Fire', 'Rotate planned', 'Thrust planned'],",
    "    metrics: { rocks: 3, shots: 0, pace: 'manual', risk: 'no collision cue' },",
    "    todos: ['tune rock drift', 'add shot trail', 'polish danger state', 'add score burst'],",
    "  },",
    "  {",
    "    id: 'runner',",
    "    title: 'Runner',",
    "    status: 'lane sketch',",
    "    detail: 'lane speed and obstacle rhythm are not readable',",
    "    controls: ['ArrowLeft', 'ArrowRight', 'Dash'],",
    "    metrics: { lanes: 3, distance: 0, pace: 'manual', risk: 'flat pacing' },",
    "    todos: ['tune speed bands', 'add near-miss feedback', 'polish lane markers', 'show distance goal'],",
    "  },",
    "]",
    "",
    "export const arcadeTuningMatrix = [",
    "  ['Snake', 'food sparkle', 'tail safety', 'restart rhythm', 'variant label'],",
    "  ['Pong', 'serve state', 'paddle rebound', 'center stripe', 'ball trail'],",
    "  ['Memory', 'matched lock', 'turn counter', 'pair meter', 'win feedback'],",
    "  ['Asteroids', 'rock drift', 'shot trail', 'danger glow', 'score burst'],",
    "  ['Runner', 'lane markers', 'pace meter', 'near miss', 'distance goal'],",
    "]",
    "",
    "export const messyBalancingNotes = {",
    "  shared: {",
    "    entry: 'No Arcade entry yet; game buttons live inside the hero and do not scale to seven games.',",
    "    layout: 'The game-area swaps previews but does not expose a full catalogue or difficulty cues.',",
    "    accessibility: 'Buttons lack consistent aria labels and status text is scattered.',",
    "    responsive: 'Several boards use vmin widths without stable card rhythm on narrow screens.',",
    "  },",
    "  snake: {",
    "    easy: ['short snake', 'slow tick', 'safe opening'],",
    "    medium: ['normal tick', 'food sparkle', 'score pulse'],",
    "    hard: ['faster tick', 'wall warning', 'turn buffer'],",
    "  },",
    "  pong: {",
    "    easy: ['wide paddles', 'slow ball', 'serve hint'],",
    "    medium: ['normal paddles', 'angled rebound', 'center stripe'],",
    "    hard: ['small paddles', 'fast ball', 'trail cue'],",
    "  },",
    "  memory: {",
    "    easy: ['four pairs', 'long reveal', 'match lock'],",
    "    medium: ['six pairs', 'move goal', 'pair meter'],",
    "    hard: ['eight pairs', 'short reveal', 'combo feedback'],",
    "  },",
    "  asteroids: {",
    "    easy: ['three rocks', 'slow drift', 'large ship'],",
    "    medium: ['five rocks', 'danger glow', 'shot trail'],",
    "    hard: ['seven rocks', 'split rocks', 'score burst'],",
    "  },",
    "  runner: {",
    "    easy: ['slow lanes', 'few obstacles', 'distance goal'],",
    "    medium: ['pace meter', 'near miss', 'lane markers'],",
    "    hard: ['fast lanes', 'dense obstacles', 'combo distance'],",
    "  },",
    "}",
    "",
    "export function draftByTitle(title) {",
    "  return arcadeDrafts.find((draft) => draft.title === title) || arcadeDrafts[0]",
    "}",
    "",
    "export function allExistingGameDetails() {",
    "  return arcadeDrafts.map((draft) => `${draft.title}: ${draft.detail}`)",
    "}",
    "",
    "export function missingArcadeTargets(source) {",
    "  return ['Arcade', 'Snake', 'Pong', 'Memory', 'Asteroids', 'Runner', 'Tetris', 'Breakout']",
    "    .filter((target) => !source.includes(target))",
    "}",
    "",
    "export function describeTuning(title) {",
    "  const row = arcadeTuningMatrix.find((item) => item[0] === title)",
    "  if (!row) return 'Needs arcade tuning detail.'",
    "  return `${row[0]} needs ${row.slice(1).join(', ')}.`",
    "}",
    "",
    "export const fillerAuditRows = [",
    "  { area: 'entry', issue: 'No seven-game Arcade shell', severity: 'high' },",
    "  { area: 'snake', issue: 'Food and game-over state are plain', severity: 'medium' },",
    "  { area: 'pong', issue: 'Serve and rebound detail missing', severity: 'medium' },",
    "  { area: 'memory', issue: 'Pair feedback is not distinct', severity: 'medium' },",
    "  { area: 'asteroids', issue: 'Danger and motion detail missing', severity: 'medium' },",
    "  { area: 'runner', issue: 'Pace and lane cues are weak', severity: 'medium' },",
    "  { area: 'tetris', issue: 'Missing new game', severity: 'high' },",
    "  { area: 'breakout', issue: 'Missing new game', severity: 'high' },",
    "]",
    "",
    "export const perGameCopyDeck = [",
    "  { game: 'Snake', slot: 'headline', text: 'Thread the garden without clipping the wall.' },",
    "  { game: 'Snake', slot: 'status', text: 'Food sparkle and restart rhythm are not tuned.' },",
    "  { game: 'Snake', slot: 'control', text: 'Arrow keys should feel buffered and forgiving.' },",
    "  { game: 'Snake', slot: 'badge', text: 'Classic coil variant needs a clearer label.' },",
    "  { game: 'Snake', slot: 'score', text: 'Score pulses should show growth after food.' },",
    "  { game: 'Snake', slot: 'danger', text: 'Wall warning should appear before collision.' },",
    "  { game: 'Pong', slot: 'headline', text: 'Serve, rally, and angle the return.' },",
    "  { game: 'Pong', slot: 'status', text: 'Rebound detail is still too flat.' },",
    "  { game: 'Pong', slot: 'control', text: 'Nudge is temporary until paddle controls are polished.' },",
    "  { game: 'Pong', slot: 'badge', text: 'Duel variant needs a serve badge.' },",
    "  { game: 'Pong', slot: 'score', text: 'Rally score should feel like a scoreboard.' },",
    "  { game: 'Pong', slot: 'danger', text: 'Ball trail should help fast reads.' },",
    "  { game: 'Memory', slot: 'headline', text: 'Flip pairs and lock matches.' },",
    "  { game: 'Memory', slot: 'status', text: 'Matched detail is not distinct enough.' },",
    "  { game: 'Memory', slot: 'control', text: 'Cards need reset and clear pair count.' },",
    "  { game: 'Memory', slot: 'badge', text: 'Focus variant should show a calm badge.' },",
    "  { game: 'Memory', slot: 'score', text: 'Moves need a goal and win copy.' },",
    "  { game: 'Memory', slot: 'danger', text: 'Mismatch feedback should be gentle.' },",
    "  { game: 'Asteroids', slot: 'headline', text: 'Drift through rocks and fire carefully.' },",
    "  { game: 'Asteroids', slot: 'status', text: 'Danger glow and rock drift are placeholder.' },",
    "  { game: 'Asteroids', slot: 'control', text: 'Fire works, rotate and thrust are planned.' },",
    "  { game: 'Asteroids', slot: 'badge', text: 'Orbit variant needs a danger badge.' },",
    "  { game: 'Asteroids', slot: 'score', text: 'Score bursts should follow rock hits.' },",
    "  { game: 'Asteroids', slot: 'danger', text: 'Close rocks need urgent contrast.' },",
    "  { game: 'Runner', slot: 'headline', text: 'Shift lanes and chase distance.' },",
    "  { game: 'Runner', slot: 'status', text: 'Pace cues and obstacle rhythm are weak.' },",
    "  { game: 'Runner', slot: 'control', text: 'Dash and lane shifts need consistent feedback.' },",
    "  { game: 'Runner', slot: 'badge', text: 'Sprint variant needs a speed badge.' },",
    "  { game: 'Runner', slot: 'score', text: 'Distance goal should read at a glance.' },",
    "  { game: 'Runner', slot: 'danger', text: 'Near misses should feel rewarding.' },",
    "  { game: 'Tetris', slot: 'headline', text: 'New falling-block game missing from build.' },",
    "  { game: 'Tetris', slot: 'status', text: 'Needs grid, piece, score, and restart.' },",
    "  { game: 'Tetris', slot: 'control', text: 'Move, drop, and rotate controls expected.' },",
    "  { game: 'Tetris', slot: 'badge', text: 'Stack variant should match arcade shell.' },",
    "  { game: 'Tetris', slot: 'score', text: 'Line clear score should be visible.' },",
    "  { game: 'Tetris', slot: 'danger', text: 'Top-out warning should be readable.' },",
    "  { game: 'Breakout', slot: 'headline', text: 'New paddle-and-bricks game missing from build.' },",
    "  { game: 'Breakout', slot: 'status', text: 'Needs paddle, ball, bricks, and restart.' },",
    "  { game: 'Breakout', slot: 'control', text: 'Move paddle and launch ball controls expected.' },",
    "  { game: 'Breakout', slot: 'badge', text: 'Brick variant should match arcade shell.' },",
    "  { game: 'Breakout', slot: 'score', text: 'Brick clear score should be visible.' },",
    "  { game: 'Breakout', slot: 'danger', text: 'Ball loss warning should be readable.' },",
    "]",
    "",
    "export function copyForGame(game) {",
    "  return perGameCopyDeck.filter((item) => item.game === game)",
    "}",
    "",
    "export function auditCopyCompleteness() {",
    "  const games = ['Snake', 'Pong', 'Memory', 'Asteroids', 'Runner', 'Tetris', 'Breakout']",
    "  return games.map((game) => ({ game, count: copyForGame(game).length }))",
    "}",
    "",
    "export const controlHelpDeck = [",
    "  { game: 'Snake', primary: 'Arrow keys', secondary: 'Start / Reset', detail: 'buffer turns and show food tune feedback' },",
    "  { game: 'Snake', primary: 'Space planned', secondary: 'Pause planned', detail: 'upgrade pause state copy' },",
    "  { game: 'Pong', primary: 'Nudge', secondary: 'W/S planned', detail: 'tune paddle rebound copy' },",
    "  { game: 'Pong', primary: 'Arrow keys planned', secondary: 'Serve planned', detail: 'polish rally status' },",
    "  { game: 'Memory', primary: 'Click card', secondary: 'Reset planned', detail: 'upgrade pair reveal copy' },",
    "  { game: 'Memory', primary: 'Keyboard focus planned', secondary: 'Hint planned', detail: 'tune matched state label' },",
    "  { game: 'Asteroids', primary: 'Fire', secondary: 'Rotate planned', detail: 'polish shot trail label' },",
    "  { game: 'Asteroids', primary: 'Thrust planned', secondary: 'Shield planned', detail: 'upgrade danger glow copy' },",
    "  { game: 'Runner', primary: 'ArrowLeft / ArrowRight', secondary: 'Dash', detail: 'tune lane marker copy' },",
    "  { game: 'Runner', primary: 'Jump planned', secondary: 'Slide planned', detail: 'polish distance goal label' },",
    "  { game: 'Tetris', primary: 'Move planned', secondary: 'Rotate / Drop planned', detail: 'new game control shell required' },",
    "  { game: 'Breakout', primary: 'Paddle planned', secondary: 'Launch planned', detail: 'new game control shell required' },",
    "]",
    "",
    "export function controlHelpFor(game) {",
    "  return controlHelpDeck.filter((item) => item.game === game)",
    "}",
    "",
    "export function fullAuditText() {",
    "  return arcadeDrafts",
    "    .map((draft) => `${draft.title}: ${draft.status}; ${draft.detail}; ${controlHelpFor(draft.title).map((item) => item.detail).join('; ')}`)",
    "    .join('\\n')",
    "}",
    "",
    "export const visualDebtDeck = [",
    "  { game: 'Snake', item: 'board border', need: 'polish with active variant cue' },",
    "  { game: 'Snake', item: 'food cell', need: 'detail sparkle or pulse' },",
    "  { game: 'Snake', item: 'game over', need: 'upgrade restart state' },",
    "  { game: 'Pong', item: 'center line', need: 'add arcade court detail' },",
    "  { game: 'Pong', item: 'ball', need: 'tune motion trail' },",
    "  { game: 'Pong', item: 'paddle', need: 'polish rebound label' },",
    "  { game: 'Memory', item: 'card back', need: 'variant symbol detail' },",
    "  { game: 'Memory', item: 'matched card', need: 'upgrade lock state' },",
    "  { game: 'Memory', item: 'moves', need: 'tune win condition copy' },",
    "  { game: 'Asteroids', item: 'rock', need: 'danger glow detail' },",
    "  { game: 'Asteroids', item: 'ship', need: 'polish thrust shape' },",
    "  { game: 'Asteroids', item: 'shot', need: 'upgrade trail state' },",
    "  { game: 'Runner', item: 'lane', need: 'tune speed stripes' },",
    "  { game: 'Runner', item: 'obstacle', need: 'polish near-miss state' },",
    "  { game: 'Runner', item: 'distance', need: 'upgrade goal meter' },",
    "  { game: 'Tetris', item: 'well', need: 'create new board' },",
    "  { game: 'Tetris', item: 'piece', need: 'create active piece detail' },",
    "  { game: 'Tetris', item: 'lines', need: 'create line clear score' },",
    "  { game: 'Breakout', item: 'bricks', need: 'create brick field' },",
    "  { game: 'Breakout', item: 'paddle', need: 'create paddle detail' },",
    "  { game: 'Breakout', item: 'ball', need: 'create launch state' },",
    "]",
    "",
    "export function visualDebtFor(game) {",
    "  return visualDebtDeck.filter((item) => item.game === game)",
    "}",
    "",
    "export function totalDebtCount() {",
    "  return visualDebtDeck.length + fillerAuditRows.length + controlHelpDeck.length",
    "}",
    "",
    "export function arcadeReady(source) {",
    "  return missingArcadeTargets(source).length === 0 && source.includes('APPLY_PATCH_ARCADE_SENTINEL')",
    "}",
    "",
  ].join("\n")
}

function messyDebugPanel() {
  return [
    "export function DebugPanel({ items = [], matrix = [] }) {",
    "  const rows = items.length ? items : []",
    "  return (",
    "    <aside className=\"debug-panel\">",
    "      <p className=\"debug-title\">Prototype audit</p>",
    "      <div className=\"debug-list\">",
    "        {rows.map((item) => (",
    "          <article key={item.id} className=\"debug-row\">",
    "            <strong>{item.title}</strong>",
    "            <span>{item.detail}</span>",
    "            <small>{item.todos?.slice(0, 2).join(' / ')}</small>",
    "          </article>",
    "        ))}",
    "      </div>",
    "      <div className=\"debug-matrix\">",
    "        {matrix.map((row) => (",
    "          <p key={row[0]}><b>{row[0]}</b>{row.slice(1).join(' - ')}</p>",
    "        ))}",
    "      </div>",
    "    </aside>",
    "  )",
    "}",
    "",
    "export function AuditBadge({ label, tone = 'plain' }) {",
    "  return <span className={`audit-badge ${tone}`}>{label}</span>",
    "}",
    "",
    "export function PlaceholderMeter({ value = 0, label = 'detail' }) {",
    "  const safe = Math.max(0, Math.min(100, value))",
    "  return (",
    "    <span className=\"placeholder-meter\" aria-label={`${label} ${safe}%`}>",
    "      <i style={{ width: `${safe}%` }} />",
    "    </span>",
    "  )",
    "}",
    "",
  ].join("\n")
}

function fixtureExcerpts(workspace) {
  return [
    "Current file: src/App.jsx",
    "```jsx",
    readFile(path.join(workspace, "src", "App.jsx")),
    "```",
    "Current file: src/styles.css",
    "```css",
    readFile(path.join(workspace, "src", "styles.css")),
    "```",
    "Current file: src/games/snakeState.js",
    "```js",
    readFile(path.join(workspace, "src", "games", "snakeState.js")),
    "```",
    "Current file: src/games/arcadeDrafts.js",
    "```js",
    readFile(path.join(workspace, "src", "games", "arcadeDrafts.js")),
    "```",
    "Current file: src/components/DebugPanel.jsx",
    "```jsx",
    readFile(path.join(workspace, "src", "components", "DebugPanel.jsx")),
    "```",
  ].join("\n")
}

function commandRunTool(variant) {
  const commandLineDescription = variant === "markerless"
    ? "String payload for the target command. For apply_patch, pass the raw markerless patch body starting directly with *** Update File:, *** Add File:, or *** Delete File:. Do not include *** Begin Patch or *** End Patch."
    : "String payload for the target command. For apply_patch, pass the raw patch body beginning with *** Begin Patch and ending with *** End Patch."
  const commandLinePattern = variant === "markerless"
    ? "^(?![\\s\\S]*\\*\\*\\* Begin Patch)(?![\\s\\S]*\\*\\*\\* End Patch)\\*\\*\\* (?:Update|Add|Delete) File: [\\s\\S]*$"
    : "^\\*\\*\\* Begin Patch[\\s\\S]*\\*\\*\\* End Patch\\s*$"
  return {
    type: "function",
    name: "command_run",
    description: commandRunDescription(variant),
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
              command_type: {
                type: "string",
                enum: ["apply_patch"],
                description: "Target internal command name from the available commands list. For this benchmark, use apply_patch.",
              },
              command_line: {
                type: "string",
                description: commandLineDescription,
                pattern: commandLinePattern,
              },
              step: {
                type: "integer",
                minimum: 1,
              },
            },
          },
        },
      },
    },
    strict: false,
  }
}

function commandRunDescription(variant) {
  const patchFormat = variant === "markerless"
    ? [
        "- apply_patch: Raw freeform body with no wrapper markers.",
        "- Begin directly with one or more file operations.",
        "- Valid first lines are `*** Update File: path`, `*** Add File: path`, or `*** Delete File: path`.",
        "- Do not include `*** Begin Patch` or `*** End Patch` anywhere.",
        "- Grammar: start: hunk+; hunk: add_hunk | delete_hunk | update_hunk.",
      ]
    : [
        "- apply_patch: Raw freeform body.",
        "- The body must start with `*** Begin Patch`.",
        "- The body must end with `*** End Patch`.",
        "- Grammar: start: begin_patch hunk+ end_patch.",
      ]
  return [
    "Run tools as a pure batch+step command runner. Use assistant content only for concise reasoning, progress, and conclusions. Available commands: apply_patch.",
    "Command run patterns:",
    "- Example repair batch: step 1 `apply_patch` across related files, step 2 run the known build command, step 3 run multiple known test commands in the same step.",
    "Command line formats:",
    ...patchFormat,
  ].join("\n")
}

function runtimeBaseInstructions() {
  const agentPromptPath = process.env.COMMAND_RUN_APPLY_PATCH_AGENT_PROMPT ||
    path.join(repoRoot, "agents", "src", "thinking", "prompt.md")
  const agentPrompt = fs.existsSync(agentPromptPath) ? readFile(agentPromptPath).trim() : ""
  return [
    "You are Codex, a coding agent based on GPT-5.",
    "",
    agentPrompt,
  ].join("\n")
}

function permissionsContext() {
  return [
    "<permissions instructions>",
    "Network access is enabled. Approval policy is currently never.",
    "</permissions instructions>",
  ].join("\n")
}

function workspaceSnapshot(workspace) {
  const rows = []
  collectSnapshotRows(workspace, workspace, rows)
  rows.sort((a, b) => a.path.localeCompare(b.path))
  return [
    "<WORKSPACE_SNAPSHOT>",
    "columns: modified_utc | lines | suffix | path",
    ...rows.map((row) => `${row.modified_utc} | ${row.lines} | ${row.suffix} | ${row.path}`),
    "</WORKSPACE_SNAPSHOT>",
  ].join("\n")
}

function collectSnapshotRows(root, dir, rows) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === "node_modules" || entry.name === ".git") continue
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) {
      collectSnapshotRows(root, full, rows)
      continue
    }
    if (!entry.isFile()) continue
    const stat = fs.statSync(full)
    const rel = path.relative(root, full).replace(/\\/g, "/")
    rows.push({
      modified_utc: stat.mtime.toISOString(),
      lines: lineCount(full),
      suffix: path.extname(entry.name).replace(/^\./, "") || "-",
      path: rel,
    })
  }
}

function lineCount(file) {
  const text = readFile(file)
  if (!text) return 0
  return text.split(/\r?\n/).length
}

function environmentContext(workspace) {
  return [
    "<environment_context>",
    `  <cwd>${workspace}</cwd>`,
    "  <shell>powershell</shell>",
    "  <current_date>2026-06-18</current_date>",
    "  <timezone>Europe/Paris</timezone>",
    "  <filesystem><workspace_roots>",
    `    <root>${workspace}</root>`,
    "  </workspace_roots><permission_profile type=\"disabled\"><file_system type=\"unrestricted\" /></permission_profile></filesystem>",
    "</environment_context>",
  ].join("\n")
}

function runtimeMessages(variant, marker, workspace) {
  return [
    { role: "system", content: runtimeBaseInstructions() },
    { role: "developer", content: permissionsContext() },
    { role: "user", content: workspaceSnapshot(workspace) },
    { role: "user", content: environmentContext(workspace) },
    { role: "user", content: userPrompt(variant, marker, workspace) },
  ]
}

function userPrompt(variant, marker, workspace) {
  const variantRules = variant === "markerless"
    ? [
        "Experimental apply_patch format for this run:",
        "- command_type must be apply_patch.",
        "- command_line must start directly with *** Update File:, *** Add File:, or *** Delete File:.",
        "- Do not include *** Begin Patch or *** End Patch anywhere in command_line.",
      ]
    : [
        "Current apply_patch format for this run:",
        "- command_type must be apply_patch.",
        "- command_line must begin with *** Begin Patch and end with *** End Patch.",
      ]
  return [
    `Benchmark marker: ${marker}`,
    "You are in a disposable messy React multi-game arcade workspace.",
    "Make one implementation move now.",
    "Use the command_run tool with apply_patch commands only.",
    "",
    ...variantRules,
    "",
    "Task:",
    "- Preserve and improve all five existing games: Snake, Pong, Memory, Asteroids, and Runner.",
    "- Each existing game must receive at least one visible detail/polish change, such as tuned status copy, special cells, feedback states, pacing indicators, badges, or controls.",
    "- Add an arcade platform entry/shell that lets the user choose among all seven games.",
    "- Add exactly two additional arcade games: Tetris and Breakout.",
    "- Provide visible game selection controls, score/status areas, restart controls, keyboard or button controls, and responsive layout.",
    "- Editing src/App.jsx, src/styles.css, and src/games/snakeState.js is enough, but adding small helper files is allowed.",
    "- Include the literal strings Arcade, Snake, Pong, Memory, Asteroids, Runner, Tetris, Breakout, and APPLY_PATCH_ARCADE_SENTINEL in the changed source.",
    "- Include a source marker or visible label containing the word detail, tune, polish, variant, or upgrade so static checks can detect that existing games were modified.",
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
      role: ["assistant", "system", "developer"].includes(message.role) ? message.role : "user",
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
    if (["codex", "openai", "chatgpt"].includes(providerName)) {
      request.include = ["reasoning.encrypted_content"]
    }
  }
  if (serviceTier && serviceTier !== "default" && ["codex", "openai", "chatgpt"].includes(providerName)) {
    request.service_tier = serviceTier
  }
  return request
}

async function callProvider(request) {
  const endpoint = providerEndpoint(providerName)
  const apiKey = providerApiKey(providerName)
  assert(apiKey, `missing API key for provider ${providerName}`)
  const headers = {
    Authorization: `Bearer ${apiKey}`,
    "Content-Type": "application/json",
  }
  if (providerName === "codex") {
    headers.originator = "codex_cli_rs"
    headers["User-Agent"] = "codex_cli_rs/0.0.0 (Windows 10.0; x86_64)"
    headers.session_id = "tura-apply-patch-marker-ablation"
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
    const body = parseProviderBody(text, response.headers.get("content-type") || "")
    return {
      ok: response.ok,
      status: response.status,
      status_text: response.statusText,
      endpoint,
      duration_ms: Math.round(performance.now() - started),
      body,
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
  if (contentType.includes("text/event-stream") || text.includes("\ndata:")) {
    return parseSseResponse(text)
  }
  try {
    return JSON.parse(text)
  } catch {
    return { raw_text: text }
  }
}

function parseSseResponse(text) {
  let outputText = ""
  let completed = null
  const events = []
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trimStart()
    if (!line.startsWith("data:")) continue
    const data = line.slice("data:".length).trim()
    if (!data || data === "[DONE]") continue
    let event
    try {
      event = JSON.parse(data)
    } catch {
      events.push({ raw_data: data })
      continue
    }
    appendCodexStreamText(event, (delta) => {
      outputText += delta
    })
    if (event.response) completed = event.response
    events.push(event)
  }
  const root = completed && typeof completed === "object" ? { ...completed } : { output: [] }
  if (outputText) root.output_text = outputText
  root.events = events
  return root
}

function appendCodexStreamText(event, append) {
  const type = typeof event.type === "string" ? event.type : ""
  if (
    type === "response.function_call_arguments.delta" ||
    type === "response.function_call_arguments.done" ||
    type === "response.output_item.added" ||
    type === "response.output_item.done"
  ) {
    return
  }
  if (type.endsWith(".delta") && typeof event.delta === "string") {
    append(event.delta)
    return
  }
  if (typeof event.delta === "string") append(event.delta)
  else if (typeof event.response?.delta === "string") append(event.response.delta)
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
  const name =
    value.name ||
    value.tool_name ||
    value.function?.name ||
    (value.type === "function_call" ? value.name : null)
  const args =
    value.arguments ??
    value.input ??
    value.function?.arguments ??
    value.args ??
    null
  if (name === "command_run" && args !== null) {
    const key = JSON.stringify(args)
    if (!seen.has(key)) {
      seen.add(key)
      calls.push({ name, arguments: args })
    }
  }
  for (const child of Object.values(value)) collectCommandRunCalls(child, calls, seen)
  return calls
}

function commandsFromCommandRunCalls(calls) {
  const commands = []
  for (const call of calls) {
    const args = parseMaybeJson(call.arguments)
    if (!args) continue
    const group = Array.isArray(args.commands) ? args.commands : []
    for (const command of group) {
      if (command && typeof command === "object") commands.push(command)
    }
  }
  return dedupeCommands(commands)
}

function dedupeCommands(commands) {
  const result = []
  const seen = new Set()
  for (const command of commands) {
    const key = `${command.command_type || command.command || ""}\u0000${command.command_line || ""}`
    if (seen.has(key)) continue
    seen.add(key)
    result.push(command)
  }
  return result
}

function stripCodeFence(text) {
  const trimmed = String(text || "").trim()
  const match = trimmed.match(/^```(?:patch|diff)?\s*\n([\s\S]*?)\n```\s*$/)
  return match ? match[1].trim() : trimmed
}

function stripApplyPatchPrefix(text) {
  let body = stripCodeFence(text)
  for (const prefix of ["apply_patch <<'PATCH'", "apply_patch <<\"PATCH\"", "apply_patch"]) {
    if (body.startsWith(prefix)) body = body.slice(prefix.length).trimStart()
  }
  return body
}

function extractBeginEnd(text) {
  const body = stripApplyPatchPrefix(text)
  const begin = body.indexOf("*** Begin Patch")
  if (begin < 0) return null
  const endMarker = "*** End Patch"
  const endRelative = body.slice(begin).indexOf(endMarker)
  if (endRelative < 0) return null
  return body.slice(begin, begin + endRelative + endMarker.length).trim()
}

function startsWithPatchHunk(text) {
  const trimmed = stripApplyPatchPrefix(text).trimStart()
  return trimmed.startsWith("*** Add File: ") || trimmed.startsWith("*** Update File: ") || trimmed.startsWith("*** Delete File: ")
}

function normalizePatchText(text) {
  const extracted = extractBeginEnd(text)
  if (extracted) return extracted
  const body = stripApplyPatchPrefix(text)
  if (!startsWithPatchHunk(body)) return body
  const endMarker = "*** End Patch"
  const clipped = body.includes(endMarker)
    ? body.slice(0, body.indexOf(endMarker) + endMarker.length).trim()
    : `${body.trimEnd()}\n${endMarker}`
  return `*** Begin Patch\n${clipped}`
}

function parsePatch(patchText) {
  const changes = []
  let current = null
  let hunk = null
  let started = false
  let ended = false
  const lines = String(patchText || "").split(/\r?\n/)
  function finishChange() {
    if (hunk && current) current.hunks.push(hunk)
    hunk = null
    if (current) changes.push(current)
    current = null
  }
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]
    const lineNumber = index + 1
    if (!started) {
      if (line.trim() === "") continue
      if (line === "*** Begin Patch") {
        started = true
        continue
      }
      return { ok: false, error: `invalid patch: expected *** Begin Patch at line ${lineNumber}` }
    }
    if (line.startsWith("*** Update File: ")) {
      finishChange()
      current = { kind: "update", path: line.slice("*** Update File: ".length), move_path: null, hunks: [], lines: [] }
    } else if (line.startsWith("*** Add File: ")) {
      finishChange()
      current = { kind: "add", path: line.slice("*** Add File: ".length), move_path: null, hunks: [], lines: [] }
    } else if (line.startsWith("*** Delete File: ")) {
      finishChange()
      current = { kind: "delete", path: line.slice("*** Delete File: ".length), move_path: null, hunks: [], lines: [] }
    } else if (line.startsWith("*** Move to: ")) {
      if (!current) return { ok: false, error: "move target without file" }
      if (current.kind !== "update") return { ok: false, error: "move target is only valid for update file changes" }
      current.move_path = line.slice("*** Move to: ".length)
    } else if (line.startsWith("@@")) {
      if (!current) return { ok: false, error: "hunk without file" }
      if (current.kind !== "update") return { ok: false, error: "hunk is only valid for update file changes" }
      if (hunk) current.hunks.push(hunk)
      hunk = []
    } else if (line.startsWith("*** End Patch")) {
      finishChange()
      ended = true
      break
    } else if (line === "*** End of File") {
      continue
    } else if (current) {
      if (current.kind === "add" && line.startsWith("+")) {
        current.lines.push(line.slice(1))
      } else if (hunk) {
        if (/^[ +\-]/.test(line)) hunk.push(line)
        else if (line.trim() === "") hunk.push(` ${line}`)
        else return { ok: false, error: `invalid patch line ${lineNumber}: hunk lines must start with space, +, or -` }
      } else if (line.trim() !== "") {
        return { ok: false, error: `invalid patch line ${lineNumber}: content must be inside a hunk` }
      }
    } else if (line.trim() !== "") {
      return { ok: false, error: `invalid patch line ${lineNumber}: expected file operation` }
    }
  }
  if (!started) return { ok: false, error: "invalid patch: missing *** Begin Patch" }
  if (!ended) return { ok: false, error: "invalid patch: missing *** End Patch" }
  if (changes.length === 0) return { ok: false, error: "no file changes found in patch" }
  for (const change of changes) {
    if (!change.path.trim()) return { ok: false, error: "invalid patch: file path must not be empty" }
    if (change.kind === "add" && (change.move_path || change.hunks.length > 0)) {
      return { ok: false, error: "invalid patch: add file cannot have move target or hunks" }
    }
    if (change.kind === "delete" && (change.move_path || change.hunks.length > 0 || change.lines.length > 0)) {
      return { ok: false, error: "invalid patch: delete file cannot have move target or content" }
    }
    if (change.kind === "update" && change.hunks.some((item) => item.length === 0)) {
      return { ok: false, error: `invalid patch: update file ${change.path} contains an empty hunk` }
    }
  }
  return { ok: true, changes }
}

function validateApplyPatchCommand(command, expectedVariant) {
  const raw = String(command.command_line || "")
  const stripped = stripApplyPatchPrefix(raw)
  const normalizedParse = parsePatch(normalizePatchText(raw))
  const strictText = extractBeginEnd(raw)
  const strictParse = strictText ? parsePatch(strictText) : { ok: false, error: "missing strict Begin/End wrapper" }
  const markerlessText = startsWithPatchHunk(raw)
    ? `*** Begin Patch\n${stripped.includes("*** End Patch") ? stripped : `${stripped.trimEnd()}\n*** End Patch`}`
    : null
  const markerlessParse = markerlessText ? parsePatch(markerlessText) : { ok: false, error: "not markerless patch body" }
  const hasBegin = stripped.includes("*** Begin Patch")
  const hasEnd = stripped.includes("*** End Patch")
  const strictCurrentValid = Boolean(hasBegin && hasEnd && strictParse.ok)
  const strictMarkerlessValid = Boolean(!hasBegin && !hasEnd && startsWithPatchHunk(raw) && markerlessParse.ok)
  return {
    command_type: command.command_type || command.command || null,
    command_line_chars: raw.length,
    has_begin_marker: hasBegin,
    has_end_marker: hasEnd,
    starts_with_hunk_without_marker: startsWithPatchHunk(raw),
    runtime_normalized_valid: Boolean(normalizedParse.ok),
    strict_current_valid: strictCurrentValid,
    strict_markerless_valid: strictMarkerlessValid,
    expected_variant_valid: expectedVariant === "markerless" ? strictMarkerlessValid : strictCurrentValid,
    error: normalizedParse.ok ? null : normalizedParse.error,
    change_count: normalizedParse.ok ? normalizedParse.changes.length : 0,
    paths: normalizedParse.ok ? normalizedParse.changes.map((change) => change.path) : [],
    command_line_excerpt: raw.slice(0, 500),
  }
}

function summarizeCommandValidity(commands, variant) {
  const applyPatch = commands.filter((command) => (command.command_type || command.command) === "apply_patch")
  const validations = applyPatch.map((command) => validateApplyPatchCommand(command, variant))
  return {
    command_count: commands.length,
    apply_patch_count: applyPatch.length,
    valid_apply_patch_count: validations.filter((item) => item.expected_variant_valid).length,
    runtime_normalized_valid_count: validations.filter((item) => item.runtime_normalized_valid).length,
    all_apply_patch_valid_for_variant: applyPatch.length > 0 && validations.every((item) => item.expected_variant_valid),
    validations,
  }
}

function writePatchArtifacts(trialRoot, commands, validity, variant, metadata) {
  const artifactRoot = path.join(trialRoot, "patch-artifacts")
  const patchRoot = path.join(artifactRoot, "patches")
  mkdirp(patchRoot)
  writeJson(path.join(artifactRoot, "commands.json"), commands)
  writeJson(path.join(artifactRoot, "validity.json"), validity)

  const applyPatchCommands = commands.filter((command) => (command.command_type || command.command) === "apply_patch")
  const invalid = []
  const all = []
  applyPatchCommands.forEach((command, index) => {
    const validation = validity.validations[index] || validateApplyPatchCommand(command, variant)
    const ordinal = String(index + 1).padStart(2, "0")
    const raw = String(command.command_line || "")
    const normalized = normalizePatchText(raw)
    const rawPath = path.join(patchRoot, `${ordinal}.raw.patch`)
    const normalizedPath = path.join(patchRoot, `${ordinal}.normalized.patch`)
    const validationPath = path.join(patchRoot, `${ordinal}.validation.json`)
    writeFile(rawPath, raw)
    writeFile(normalizedPath, normalized)
    writeJson(validationPath, {
      ...metadata,
      patch_index: index + 1,
      variant,
      raw_path: rawPath,
      normalized_path: normalizedPath,
      validation,
      command,
    })
    const record = {
      ...metadata,
      patch_index: index + 1,
      variant,
      raw_path: rawPath,
      normalized_path: normalizedPath,
      validation_path: validationPath,
      expected_variant_valid: validation.expected_variant_valid,
      runtime_normalized_valid: validation.runtime_normalized_valid,
      error: validation.error,
      command_line_chars: validation.command_line_chars,
      paths: validation.paths,
    }
    all.push(record)
    if (!validation.expected_variant_valid) invalid.push({ ...record, raw_command_line: raw })
  })
  writeJson(path.join(artifactRoot, "all-patches.json"), all)
  writeJson(path.join(artifactRoot, "invalid-patches.json"), invalid)
  appendJsonl(path.join(runRoot, "all-patches.jsonl"), all)
  appendJsonl(path.join(runRoot, "invalid-patches.jsonl"), invalid)
}

function appendJsonl(file, records) {
  if (!records.length) return
  mkdirp(path.dirname(file))
  fs.appendFileSync(file, records.map((record) => JSON.stringify(record)).join("\n") + "\n", "utf8")
}

function failedTrialSummary(variant, index, error) {
  const trialName = `${variant}-${String(index + 1).padStart(2, "0")}`
  const trialRoot = path.join(runRoot, variant, trialName)
  mkdirp(trialRoot)
  const summary = {
    id: trialName,
    variant,
    index: index + 1,
    marker: null,
    workspace: path.join(trialRoot, "workspace"),
    context_path: path.join(trialRoot, "context.json"),
    request_path: path.join(trialRoot, "request.json"),
    response_path: path.join(trialRoot, "response.json"),
    duration_ms: 0,
    provider: providerName,
    model,
    endpoint: providerEndpoint(providerName),
    http_ok: false,
    http_status: null,
    http_status_text: null,
    provider_error: String(error.stack || error.message || error),
    command_run_call_count: 0,
    validity: {
      command_count: 0,
      apply_patch_count: 0,
      valid_apply_patch_count: 0,
      runtime_normalized_valid_count: 0,
      all_apply_patch_valid_for_variant: false,
      validations: [],
    },
    ok: false,
  }
  writeJson(path.join(trialRoot, "trial-summary.json"), summary)
  writeJson(path.join(trialRoot, "trial-error.json"), {
    error: summary.provider_error,
    variant,
    index: index + 1,
  })
  return summary
}

async function runTrial(variant, template, index) {
  const trialName = `${variant}-${String(index + 1).padStart(2, "0")}`
  const trialRoot = path.join(runRoot, variant, trialName)
  const workspace = path.join(trialRoot, "workspace")
  const marker = `APPLY_PATCH_ABLATION_${variant.toUpperCase()}_${index + 1}_${Date.now()}`
  fs.rmSync(trialRoot, { recursive: true, force: true })
  copyDir(template, workspace)
  writeFile(path.join(workspace, "BENCHMARK_MARKER.txt"), `${marker}\n`)

  const messages = runtimeMessages(variant, marker, workspace)
  const request = buildRequest(messages, variant)
  writeJson(path.join(trialRoot, "context.json"), {
    messages,
    marker_sequence: [
      "system:base_instructions",
      "developer:permissions",
      "user:workspace_snapshot",
      "user:environment_context",
      "user:task",
    ],
    tool: commandRunTool(variant),
  })
  writeJson(path.join(trialRoot, "request.json"), request)

  const started = performance.now()
  const provider = await callProvider(request)
  writeJson(path.join(trialRoot, "response.json"), provider)

  const calls = collectCommandRunCalls(provider.body)
  const commands = commandsFromCommandRunCalls(calls)
  const validity = summarizeCommandValidity(commands, variant)
  writePatchArtifacts(trialRoot, commands, validity, variant, {
    trial_id: trialName,
    index: index + 1,
    marker,
    provider: providerName,
    model,
    workspace,
  })
  const summary = {
    id: trialName,
    variant,
    index: index + 1,
    marker,
    workspace,
    context_path: path.join(trialRoot, "context.json"),
    request_path: path.join(trialRoot, "request.json"),
    response_path: path.join(trialRoot, "response.json"),
    duration_ms: Math.round(performance.now() - started),
    provider: providerName,
    model,
    endpoint: provider.endpoint,
    http_ok: provider.ok,
    http_status: provider.status,
    http_status_text: provider.status_text,
    provider_error: provider.error || null,
    command_run_call_count: calls.length,
    validity,
    ok: provider.ok && validity.all_apply_patch_valid_for_variant,
  }
  writeJson(path.join(trialRoot, "trial-summary.json"), summary)
  return summary
}

async function runWithLimit(tasks, limit) {
  const results = []
  let next = 0
  async function worker() {
    for (;;) {
      const index = next
      next += 1
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
  const valid = group.filter((item) => item.validity.all_apply_patch_valid_for_variant).length
  return {
    variant,
    trials: group.length,
    ok_trials: group.filter((item) => item.ok).length,
    http_successes: group.filter((item) => item.http_ok).length,
    trials_with_command_run: group.filter((item) => item.command_run_call_count > 0).length,
    valid_trials: valid,
    valid_rate: group.length ? valid / group.length : 0,
    runtime_normalized_valid_patches: group.reduce((total, item) => total + item.validity.runtime_normalized_valid_count, 0),
    total_apply_patch_commands: group.reduce((total, item) => total + item.validity.apply_patch_count, 0),
  }
}

async function main() {
  loadDotEnv(path.join(repoRoot, ".env"))
  assert(typeof fetch === "function", "Node fetch API is required")
  fs.rmSync(runRoot, { recursive: true, force: true })
  mkdirp(runRoot)
  const template = createFixtureTemplate(path.join(runRoot, "template"))
  const tasks = []
  for (let index = 0; index < trialsPerGroup; index += 1) {
    for (const variant of variants) {
      tasks.push({
        variant,
        index,
        run: () => runTrial(variant, template, index),
      })
    }
  }
  const started = performance.now()
  const results = await runWithLimit(tasks, concurrency)
  const groups = variants.map((variant) => groupSummary(results, variant))
  const summary = normalizeBusinessSummary({
    ok: groups.every((group) => group.valid_trials === group.trials),
    run_id: runId,
    run_root: runRoot,
    template,
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
      "This benchmark does not use tura runtime, router, command_run executor, or apply_patch executor.",
      "Each trial writes a messy workspace fixture plus context/request/response JSON for direct provider analysis.",
      "current expects *** Begin Patch and *** End Patch wrappers.",
      "markerless expects command_line to start directly with *** Update/Add/Delete File and omit both wrappers.",
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
