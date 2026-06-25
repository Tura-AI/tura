#!/usr/bin/env node
import assert from "node:assert/strict"
import { spawn, spawnSync } from "node:child_process"
import fs from "node:fs"
import path from "node:path"
import process from "node:process"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"
import { businessRunPaths, normalizeBusinessSummary } from "../lib/business_paths.mjs"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDir, "..", "..", "..")
const homeDir = process.env.USERPROFILE || process.env.HOME || ""
const runId = process.env.COMMAND_RUN_AGENT_RUN_ID || `game-prompts-${Date.now()}`
const runPaths = businessRunPaths("frontend-game-prompt-difficulty", runId)
const runRoot = runPaths.run_root
const summaryPath = runPaths.summary_path
const model = process.env.COMMAND_RUN_AGENT_CODEX_MODEL || "gpt-5.5"
const turaModel = process.env.COMMAND_RUN_AGENT_TURA_MODEL || (model.includes("/") ? model : `openai/${model}`)
const reasoning = process.env.COMMAND_RUN_AGENT_REASONING_EFFORT || "high"
const serviceTier = process.env.COMMAND_RUN_AGENT_SERVICE_TIER || "priority"
const timeoutMs = Number(process.env.COMMAND_RUN_AGENT_TIMEOUT_MS || 20 * 60_000)
const agents = parseAgents(process.env.COMMAND_RUN_AGENT_AGENTS || "codex,tura-thinking,tura-fast")
const selectedTasksRaw = process.env.COMMAND_RUN_GAME_BENCH_TASKS || "all"
const selectedDifficultiesRaw = process.env.COMMAND_RUN_GAME_BENCH_DIFFICULTIES || "all"
const selectedCasesRaw = process.env.COMMAND_RUN_GAME_BENCH_CASES || "all"
const maxCases = Number(process.env.COMMAND_RUN_GAME_BENCH_MAX_CASES || 0)
const parallelism = Math.max(1, Number(process.env.COMMAND_RUN_AGENT_PARALLELISM || 1))
const skipTuraBuild = (process.env.COMMAND_RUN_AGENT_SKIP_TURA_BUILD || "0") === "1"
const allowFailure = (process.env.COMMAND_RUN_AGENT_ALLOW_FAILURE || "1") !== "0"
const prepOnly = (process.env.COMMAND_RUN_AGENT_PREP_ONLY || "0") === "1"
const reportOnly = (process.env.COMMAND_RUN_AGENT_REPORT_ONLY || "0") === "1"

const turaExe = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura_exec.exe" : "tura_exec")
const codexExe = path.join(
  process.env.COMMAND_RUN_AGENT_CODEX_CURRENT_ROOT || path.join(homeDir, "Documents", "Codex"),
  "codex-rs",
  "target",
  "debug",
  process.platform === "win32" ? "codex.exe" : "codex",
)

const DIFFICULTIES = [
  {
    id: "easy",
    label: "Easy",
    brief: "Build the core playable loop with compact scope and clear controls.",
  },
  {
    id: "medium",
    label: "Medium",
    brief: "Add multiple enemy/object types, UI state, pickups or weapons, and clear feedback.",
  },
  {
    id: "hard",
    label: "Hard",
    brief: "Add boss or heavy-unit behaviors, projectiles, damage states, richer level structure, and stronger audiovisual polish.",
  },
  {
    id: "extreme",
    label: "Extreme",
    brief: "Deliver the full cinematic or game-feel target with special events, polished effects, responsive layout, verification notes, and maintainable code.",
  },
]

const TASKS = [
  {
    id: "air-combat-3d",
    title: "3D Modern Air Combat Shooter",
    summary: "A player pilots a modern fighter jet in a realistic sky battlefield, aims with the mouse, shoots varied aircraft, dodges missiles, and triggers strange aerial phenomena.",
    basePrompt: `Develop a playable 3D modern air combat shooter.

The player pilots a modern fighter jet through a sky battlefield. Mouse movement controls an aiming reticle, and the player can fire at enemy aircraft. Enemy types must include fast drones, enemy fighters, helicopters, bombers, and heavy transports. Different enemies should have different sizes, health values, movement styles, and damage thresholds: small drones should be destroyed quickly, while bombers and heavy transports should require multiple hits.

Large enemies can launch missiles at the player. The player's jet should take damage after repeated hits and can eventually crash or explode.

The battlefield should feel like a real air-combat environment: layered clouds, high-altitude sunlight, distant mountains, ocean, cities, smoke, contrails, engine trails, missile trails, muzzle flashes, and explosions. Jet weapons, missile launch sounds, explosion sounds, and visual feedback should feel as close to a realistic air battle as practical for a browser game.

Random high-speed missiles, enemy rockets, and anti-air fire should appear in the sky. The player should dodge by maneuvering and should be able to shoot some incoming flying targets to detonate them early.

Occasionally spawn a mysterious flying object. If the player hits it, trigger an impossible scene such as a sky fracture, time freeze, lightning storm, enemy transformation, or space distortion.`,
    difficultyAddons: {
      easy: [
        "Implement one player jet, one drone enemy type, mouse aiming, shooting, hit detection, score, health, and restart.",
        "Use simple but readable 3D shapes if assets are not available, but preserve the jet/sky battlefield identity.",
      ],
      medium: [
        "Add at least three enemy classes: drones, fighters, and bombers, each with different size, movement, and health.",
        "Add enemy missiles from heavy targets, visible player damage, smoke trails, and a compact HUD.",
      ],
      hard: [
        "Add helicopters, heavy transports, incoming rockets, anti-air fire, early detonation of some incoming targets, and player crash/explosion states.",
        "Add audio hooks or generated sound effects for machine guns, missiles, explosions, and damage warnings.",
      ],
      extreme: [
        "Add the full environment treatment: mountains, ocean, city silhouettes, volumetric-feeling clouds, sun glare used with restraint, contrails, missile trails, and layered explosion smoke.",
        "Add mysterious flying object events with at least three impossible effects such as time freeze, lightning storm, sky fracture, enemy transformation, or space distortion.",
      ],
    },
  },
  {
    id: "jurassic-tank-3d",
    title: "3D Jurassic Tank Shooter",
    summary: "A modern tank drives through a Jurassic battlefield, shoots dinosaurs and pterosaurs, survives giant creatures, and discovers impossible objects.",
    basePrompt: `Develop a playable 3D Jurassic tank shooter.

The player drives a modern tank through the Jurassic era. Mouse movement controls an aiming reticle, and the tank can fire at dinosaurs. Dinosaur species and body sizes should vary, and different species should have different health values and behaviors. Large dinosaurs should require multiple hits before being defeated.

Large dinosaurs can stomp or ram the tank. If the tank takes enough damage, it should become disabled or destroyed.

The sky should contain flying pterosaurs of different types, and the player should be able to shoot them. UFO saucers can also appear in the sky, attack the player, and crash if hit.

Random mysterious objects should appear to add surprise. If the player hits a mysterious object, an impossible scene should occur.

The game scene should recreate a Jurassic environment as realistically as practical in a browser: dense prehistoric jungle, cliffs, ferns, mist, mud, sunlight shafts, distant volcanoes, water, and large creatures. Dinosaurs and the tank should feel close to real-world forms rather than generic placeholders. Tank cannon audio, impact sounds, explosions, dust, smoke, recoil, and visual feedback should feel powerful and physically grounded.`,
    difficultyAddons: {
      easy: [
        "Implement one controllable tank, one small dinosaur enemy, mouse aiming, cannon shots, hit detection, health, score, and restart.",
        "Use simple dinosaur silhouettes or low-poly forms if needed, but keep the Jurassic tank-shooter identity clear.",
      ],
      medium: [
        "Add at least three dinosaur classes with different size, health, speed, and attacks.",
        "Add tank recoil, dust, muzzle flash, impact particles, and a HUD for health, ammo or reload, score, and wave.",
      ],
      hard: [
        "Add pterosaurs, UFO attackers, large dinosaur stomp damage, disabled-tank state, and flying target crash behavior.",
        "Add environmental obstacles and simple driving constraints so the tank feels grounded in terrain.",
      ],
      extreme: [
        "Add the full Jurassic atmosphere: layered jungle, cliffs, mist, water, distant volcano, animated foliage, cinematic lighting, realistic tank/dinosaur scale relationships, and strong cannon audio/visual feel.",
        "Add mysterious object effects with at least three impossible scenes such as time dilation, gravity inversion, dinosaur mutation, portal opening, or sudden meteor storm.",
      ],
    },
  },
  {
    id: "dark-pixel-metroidvania",
    title: "Dark Pixel Metroidvania Action Game",
    summary: "A runnable Claude Artifacts-friendly 2D pixel action game about a demon hunter exploring a gothic castle, fighting monsters, collecting items, and defeating bosses.",
    basePrompt: `Develop a playable 2D pixel-art Metroidvania-style action game that can run directly inside Claude Artifacts or a simple browser page.

The player is a demon hunter exploring a dark gothic castle. The game should include side-scrolling exploration, jumping, attacking monsters, collecting items, and defeating bosses. The world should include areas such as a castle hall, underground catacombs, clock tower, and library.

Controls:
- A / D or arrow keys: move
- Space: jump
- Mouse left button or J: attack
- K: cast a skill
- Shift: dodge

The player can use weapons such as a long whip, short sword, or magic projectile. The map should contain coins, health potions, keys, and hidden items.

Enemies should include skeleton soldiers, bats, ghosts, werewolves, demon mages, and large monsters. Different enemies should have different health values and attack patterns. Bosses should appear after some regions, with visible boss health bars and special abilities.

The UI must show player health, magic, coin count, current weapon, and boss health when relevant. The visual direction should include pixel lighting, moonlight, candles, lightning, broken windows, iron gates, gothic castle backgrounds, monster death effects, boss battles, simple map exploration, and a retro dark-pixel atmosphere.

Do not use existing game characters, music, or copyrighted assets. Create an original dark pixel action game.`,
    difficultyAddons: {
      easy: [
        "Implement one screen or short level, movement, jump, one attack, one enemy type, health, coins, and restart.",
        "Make it runnable as a single HTML/CSS/JS or React artifact without external copyrighted assets.",
      ],
      medium: [
        "Add at least three rooms or regions, two weapons or skills, collectibles, keys, health potions, and three enemy types.",
        "Add pixel-style lighting, candles, moonlight, and hit/death feedback.",
      ],
      hard: [
        "Add multiple castle areas, hidden items, dodge, magic meter, boss fight with health bar and special attacks, and at least five enemy types.",
        "Add meaningful attack ranges, enemy patterns, knockback, invulnerability frames, and progression gates.",
      ],
      extreme: [
        "Deliver the full retro dark-fantasy atmosphere with castle hall, catacombs, clock tower, and library, each visually distinct and connected by simple exploration.",
        "Add strong game feel: hit stop, screen shake, monster death effects, boss phases, weapon switching, responsive UI, and clear verification notes for Claude Artifacts compatibility.",
      ],
    },
  },
  {
    id: "anime-silhouette-tactics",
    title: "2D Anime Silhouette Tactical Turn-Based RPG",
    summary: "An original Japanese-animation-inspired side-view tactics RPG with silhouette characters, story, party roles, tactical positioning, boss phases, and animated backgrounds.",
    basePrompt: `Develop a playable 2D Japanese-animation-inspired tactical turn-based RPG.

The game uses a horizontal side-view silhouette art style. Characters, monsters, and bosses should appear as black silhouettes or dark contour shapes. Backgrounds should be refined 2D anime-style scenes such as twilight forest, ruined castle, moonlit valley, burning battlefield, and mysterious temple.

The game should include distinctive original character portraits, a story premise, and clear character roles. The player commands an adventuring party in turn-based combat. Party roles include swordsman, archer, mage, assassin, and healer. Each role should have different range, skills, health, and tactical purpose. The player can select a character, move position, attack enemies, cast skills, or defend.

Enemies include shadow soldiers, giant beasts, ghosts, demons, and large bosses. Different enemies should have different health values, attacks, and weaknesses. Bosses should require multi-turn fights and have special skills and phase changes.

The UI should show health, skill cooldowns, turn order, character statuses, enemy health bars, and victory/defeat messages. Combat should include simple strategy: positioning, skill ranges, counters or weaknesses, and energy accumulation.

The visual atmosphere should have strong silhouette art direction, smooth-feeling character actions, animated cloud mist, moonlight, fire, falling leaves, and an epic, mysterious, slightly dark-fantasy tone.

Do not use existing game characters, music, or copyrighted assets. Create an original silhouette tactical RPG.`,
    difficultyAddons: {
      easy: [
        "Implement one battle scene, three playable roles, two enemy types, turn order, move/attack/defend, health bars, and victory/defeat.",
        "Use original silhouettes and simple background animation.",
      ],
      medium: [
        "Add all five party roles, at least four enemy types, cooldowns, energy accumulation, and clear status effects.",
        "Add portraits or character cards, story intro text, and a refined anime-style background.",
      ],
      hard: [
        "Add tactical range visualization, positioning lanes or grid slots, strengths/weaknesses, boss phase changes, healing, and enemy special skills.",
        "Add multiple battle backgrounds or a small map exploration flow between fights.",
      ],
      extreme: [
        "Deliver the full original dark-fantasy RPG feel with strong silhouette art direction, character identity, story beats, animated mist/moon/fire/leaves, boss phases, and smooth action feedback.",
        "Add clear UI polish, readable turn sequencing, meaningful strategy choices, and verification notes that the game is playable without external copyrighted assets.",
      ],
    },
  },
]

function parseAgents(value) {
  const alias = new Map([
    ["codex", "codex"],
    ["current", "codex"],
    ["codex-current", "codex"],
    ["tura", "tura-thinking"],
    ["tura-thinking", "tura-thinking"],
    ["thinking", "tura-thinking"],
    ["tura-fast", "tura-fast"],
    ["fast", "tura-fast"],
    ["tura-fast-text-only", "tura-fast-text-only"],
    ["fast-text-only", "tura-fast-text-only"],
    ["text-only", "tura-fast-text-only"],
  ])
  const counts = new Map()
  return String(value)
    .split(",")
    .map((item) => alias.get(item.trim().toLowerCase()))
    .filter(Boolean)
    .map((agent) => {
      const next = (counts.get(agent) || 0) + 1
      counts.set(agent, next)
      return next === 1 ? agent : `${agent}-${next}`
    })
}

function agentKind(agentId) {
  return String(agentId).replace(/-\d+$/, "")
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true })
}

function writeFile(file, text) {
  mkdirp(path.dirname(file))
  fs.writeFileSync(file, text)
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"))
}

function run(command, args, options = {}) {
  const started = performance.now()
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    input: options.input,
    text: true,
    encoding: "utf8",
    timeout: options.timeoutMs || timeoutMs,
    maxBuffer: options.maxBuffer || 256 * 1024 * 1024,
    env: { ...process.env, ...(options.env || {}) },
    shell: options.shell || false,
    windowsHide: true,
  })
  return {
    command,
    args,
    status: result.status,
    signal: result.signal,
    stdout: result.stdout || "",
    stderr: result.stderr || "",
    duration_ms: Math.round(performance.now() - started),
    error: result.error ? String(result.error.stack || result.error.message || result.error) : null,
  }
}

function runOk(command, args, options = {}) {
  const result = run(command, args, options)
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status || result.signal}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}\nERROR:\n${result.error || ""}`)
  }
  return result
}

function endWritable(stream) {
  return new Promise((resolve) => {
    if (!stream) {
      resolve()
      return
    }
    stream.once("finish", resolve)
    stream.once("error", resolve)
    stream.end()
  })
}

async function runLive(command, args, options = {}) {
  const started = performance.now()
  const stdoutPath = options.stdoutPath
  const stderrPath = options.stderrPath
  const statusPath = options.statusPath
  mkdirp(path.dirname(stdoutPath))
  const stdoutStream = fs.createWriteStream(stdoutPath, { flags: "w" })
  const stderrStream = fs.createWriteStream(stderrPath, { flags: "w" })
  let stdout = ""
  let stderr = ""
  let timedOut = false
  let childExitStatus = null
  let childExitSignal = null
  let settled = false

  writeFile(statusPath, JSON.stringify({ status: "running", started_at: new Date().toISOString(), command, args, cwd: options.cwd || repoRoot }, null, 2))
  return await new Promise((resolve) => {
    let closeGraceTimer = null
    let timeoutGraceTimer = null
    const child = spawn(command, args, {
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      stdio: ["ignore", "pipe", "pipe"],
      shell: options.shell || false,
      windowsHide: true,
    })

    function settle(status, signal, error = null) {
      if (settled) return
      settled = true
      clearTimeout(timer)
      clearTimeout(closeGraceTimer)
      clearTimeout(timeoutGraceTimer)
      const result = {
        command,
        args,
        status,
        signal,
        stdout,
        stderr,
        duration_ms: Math.round(performance.now() - started),
        error: error || (timedOut ? `timed out after ${timeoutMs}ms` : null),
      }
      Promise.all([endWritable(stdoutStream), endWritable(stderrStream)]).finally(() => {
        writeFile(statusPath, JSON.stringify({ status: timedOut ? "timeout" : "closed", result }, null, 2))
        resolve(result)
      })
    }

    const timer = setTimeout(() => {
      timedOut = true
      killProcessTree(child.pid)
      timeoutGraceTimer = setTimeout(() => {
        settle(childExitStatus ?? 1, childExitSignal)
      }, 3000)
    }, options.timeoutMs || timeoutMs)

    child.stdout.on("data", (chunk) => {
      const text = chunk.toString("utf8")
      stdout += text
      stdoutStream.write(text)
    })
    child.stderr.on("data", (chunk) => {
      const text = chunk.toString("utf8")
      stderr += text
      stderrStream.write(text)
    })
    child.on("error", (error) => {
      settle(null, null, String(error.stack || error.message || error))
    })
    child.on("exit", (status, signal) => {
      childExitStatus = status
      childExitSignal = signal
      closeGraceTimer = setTimeout(() => {
        settle(timedOut ? (status ?? 1) : status, signal)
      }, 1000)
    })
    child.on("close", (status, signal) => {
      settle(timedOut ? (status ?? 1) : status, signal)
    })
  })
}

function killProcessTree(pid) {
  if (!pid) return
  try {
    if (process.platform === "win32") {
      spawnSync("taskkill", ["/pid", String(pid), "/t", "/f"], { windowsHide: true })
    } else {
      process.kill(-pid, "SIGTERM")
    }
  } catch {}
}

function serviceTierConfigArgs() {
  const tier = String(serviceTier || "").trim()
  if (!tier || tier === "default" || tier === "none" || tier === "off") return []
  return ["-c", `service_tier="${tier}"`]
}

function turaServiceTierConfigArgs() {
  const tier = String(serviceTier || "").trim()
  if (!tier || tier === "default" || tier === "none" || tier === "off") return []
  return tier === "priority" ? ["-p"] : []
}

function parseCsvSelection(value, allValues) {
  if (!value || value === "all" || value === "*") return allValues
  const set = new Set(
    String(value)
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean),
  )
  return allValues.filter((item) => set.has(item))
}

function buildCases() {
  const taskIds = parseCsvSelection(selectedTasksRaw, TASKS.map((task) => task.id))
  const difficultyIds = parseCsvSelection(selectedDifficultiesRaw, DIFFICULTIES.map((difficulty) => difficulty.id))
  let cases = []
  for (const task of TASKS.filter((item) => taskIds.includes(item.id))) {
    for (const difficulty of DIFFICULTIES.filter((item) => difficultyIds.includes(item.id))) {
      cases.push({
        id: `${task.id}.${difficulty.id}`,
        task,
        difficulty,
      })
    }
  }
  if (selectedCasesRaw && selectedCasesRaw !== "all" && selectedCasesRaw !== "*") {
    const selected = new Set(
      selectedCasesRaw
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean),
    )
    cases = cases.filter((item) => selected.has(item.id))
  }
  if (maxCases > 0) cases = cases.slice(0, maxCases)
  return cases
}

function promptForCase(benchmarkCase) {
  const { task, difficulty } = benchmarkCase
  const addons = task.difficultyAddons[difficulty.id] || []
  return [
    `Benchmark task: ${task.title}`,
    `Difficulty: ${difficulty.label}`,
    "",
    task.basePrompt,
    "",
    `Difficulty-specific requirements (${difficulty.label}):`,
    `- ${difficulty.brief}`,
    ...addons.map((item) => `- ${item}`),
    "",
    "Implementation rules:",
    "- Build the actual playable game, not a landing page or design mock.",
    "- Work only inside the current benchmark workspace.",
    "- Use original assets or generated/code-native assets only; do not use existing copyrighted game characters, music, or media.",
    "- If you use a package manager, create package.json with useful scripts such as dev, build, test, verify, or capture.",
    "- If the game can run as plain HTML, include index.html at the workspace root.",
    "- Include README.md with exact run instructions and IMPLEMENTATION_NOTES.md describing completed features, controls, missing limitations, and verification performed.",
    "- Verify that the game has a nonblank first screen and that the main interaction loop is playable.",
    "- Do not ask the user for clarification during this benchmark; make reasonable decisions and finish the implementation.",
  ].join("\n")
}

function taskReadme(benchmarkCase, agentId) {
  const { task, difficulty } = benchmarkCase
  return [
    `# ${task.title}`,
    "",
    `Benchmark case: \`${benchmarkCase.id}\``,
    `Agent: \`${agentId}\``,
    `Difficulty: ${difficulty.label}`,
    "",
    task.summary,
    "",
    "## Prompt",
    "",
    promptForCase(benchmarkCase),
    "",
  ].join("\n")
}

function prepareWorkspace(benchmarkCase, agentId) {
  const workspace = path.join(runRoot, benchmarkCase.id, agentId, "workspace")
  fs.rmSync(workspace, { recursive: true, force: true })
  mkdirp(workspace)
  writeFile(path.join(workspace, "BENCHMARK_TASK.md"), taskReadme(benchmarkCase, agentId))
  writeFile(path.join(workspace, "benchmark-case.json"), JSON.stringify({
    id: benchmarkCase.id,
    task_id: benchmarkCase.task.id,
    task_title: benchmarkCase.task.title,
    difficulty: benchmarkCase.difficulty.id,
    difficulty_label: benchmarkCase.difficulty.label,
  }, null, 2))
  return workspace
}

function turaAgentName(agentId) {
  if (agentKind(agentId) === "tura-fast-text-only") return "fast-text-only"
  return agentKind(agentId) === "tura-fast" ? "fast" : "thinking"
}

async function runCodex(agentId, benchmarkCase, workspace, agentDir) {
  return await runLive(codexExe, [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "-C",
    workspace,
    "-m",
    model,
    "--dangerously-bypass-approvals-and-sandbox",
    "-c",
    `model_reasoning_effort="${reasoning}"`,
    ...serviceTierConfigArgs(),
    promptForCase(benchmarkCase),
  ], {
    cwd: workspace,
    timeoutMs,
    stdoutPath: path.join(agentDir, "codex.stdout.jsonl"),
    stderrPath: path.join(agentDir, "codex.stderr.log"),
    statusPath: path.join(agentDir, "codex.status.json"),
  })
}

async function runTura(agentId, benchmarkCase, workspace, agentDir) {
  const sessionId = `${agentId}-${benchmarkCase.id}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
  const result = await runLive(turaExe, [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "--session-id",
    sessionId,
    "--sandbox",
    "--agent-id",
    turaAgentName(agentId),
    "-m",
    turaModel,
    ...turaServiceTierConfigArgs(),
    "--model-reasoning-effort",
    reasoning,
    "--cwd",
    workspace,
    promptForCase(benchmarkCase),
  ], {
    cwd: workspace,
    timeoutMs,
    env: {
      OPENAI_LOGIN: process.env.OPENAI_LOGIN || "oauth",
      TURA_ENV_PATH: process.env.TURA_ENV_PATH || path.join(repoRoot, ".env"),
      TURA_COMMAND_RUN_SHELL: process.env.TURA_COMMAND_RUN_SHELL || "shell_command",
      TURA_COMMAND_RUN_STRICT_JSON: "0",
      COMMAND_RUN_AGENT_TIMEOUT_MS: String(timeoutMs),
    },
    stdoutPath: path.join(agentDir, "tura.stdout.jsonl"),
    stderrPath: path.join(agentDir, "tura.stderr.log"),
    statusPath: path.join(agentDir, "tura.status.json"),
  })
  result.session_id = sessionId
  return result
}

function collectFiles(dir) {
  if (!fs.existsSync(dir)) return []
  const out = []
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const file = path.join(dir, entry.name)
    if (entry.isDirectory()) {
      const relative = path.relative(dir, file).replaceAll("\\", "/")
      if (["node_modules", ".git", "target", "dist", "build", ".next", ".vite"].includes(relative)) continue
      out.push(...collectFiles(file))
    } else if (entry.isFile()) {
      out.push(file)
    }
  }
  return out.sort()
}

function collectArtifacts(workspace) {
  return collectFiles(workspace)
    .filter((file) => {
      const relative = path.relative(workspace, file).replaceAll("\\", "/")
      if (/\/node_modules\/|\/\.git\/|\/target\/|\/dist\/|\/build\/|\/\.next\/|\/\.vite\//.test(`/${relative}/`)) return false
      return /\.(html|css|js|jsx|ts|tsx|json|md|png|jpe?g|webp|gif|svg|mp3|wav|ogg|glb|gltf|obj|fbx|zip)$/i.test(file)
    })
    .map((file) => ({
      path: path.relative(workspace, file).replaceAll("\\", "/"),
      bytes: fs.statSync(file).size,
    }))
}

function parseJsonl(text) {
  return String(text || "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      try {
        return JSON.parse(line)
      } catch {
        return null
      }
    })
    .filter(Boolean)
}

function emptyUsage() {
  return { input_tokens: 0, cached_input_tokens: 0, output_tokens: 0, reasoning_tokens: 0, total_tokens: 0, turns: [] }
}

function addUsage(total, usage) {
  if (!usage) return
  const input = Number(usage.input_tokens ?? usage.inputTokens ?? usage.prompt_tokens ?? 0)
  const cached = Number(
    usage.cached_input_tokens ??
      usage.cache_read_input_tokens ??
      usage.input_token_details?.cached_tokens ??
      usage.input_tokens_details?.cached_tokens ??
      usage.prompt_tokens_details?.cached_tokens ??
      0,
  )
  const output = Number(usage.output_tokens ?? usage.outputTokens ?? usage.completion_tokens ?? 0)
  const reasoningTokens = Number(
    usage.reasoning_output_tokens ??
      usage.reasoning_tokens ??
      usage.reasoningTokens ??
      usage.output_tokens_details?.reasoning_tokens ??
      usage.completion_tokens_details?.reasoning_tokens ??
      0,
  )
  const totalTokens = Number(usage.total_tokens ?? usage.totalTokens ?? input + output + reasoningTokens)
  total.input_tokens += input
  total.cached_input_tokens += cached
  total.output_tokens += output
  total.reasoning_tokens += reasoningTokens
  total.total_tokens += totalTokens
  total.turns.push({ input_tokens: input, cached_input_tokens: cached, output_tokens: output, reasoning_tokens: reasoningTokens, total_tokens: totalTokens })
}

function usageFromEvents(events) {
  const total = emptyUsage()
  for (const event of events) {
    addUsage(total, event.usage)
    addUsage(total, event.message?.usage)
    if (event.type === "event_msg" && event.payload?.type === "token_count") {
      addUsage(total, event.payload?.info?.last_token_usage || event.payload?.info)
    }
  }
  total.llm_turns = total.turns.length
  return total
}

function eventStats(events) {
  const commands = new Map()
  for (const event of events) {
    const item = event.item || {}
    if (item.type !== "command_execution") continue
    const key = item.id || item.command || JSON.stringify(item).slice(0, 160)
    const existing = commands.get(key) || {}
    commands.set(key, { ...existing, ...item })
  }
  const finalCommands = [...commands.values()]
  const completedCommands = finalCommands.filter((item) => item.status === "completed")
  const succeededCommands = completedCommands.filter((item) => Number(item.exit_code || 0) === 0)
  const failedCommands = completedCommands.filter((item) => Number(item.exit_code || 0) !== 0)
  return {
    events: events.length,
    turns: events.filter((event) => event.type === "turn.started" || event.type === "thread.started").length,
    command_executions: finalCommands.length,
    completed_command_executions: completedCommands.length,
    successful_command_executions: succeededCommands.length,
    failed_command_executions: failedCommands.length,
    command_success_rate: completedCommands.length ? succeededCommands.length / completedCommands.length : null,
    command_completion_rate: finalCommands.length ? completedCommands.length / finalCommands.length : null,
    command_stats_source: finalCommands.length ? "stdout-jsonl" : "none",
  }
}

function mergeUsage(items) {
  const total = emptyUsage()
  for (const usage of items) {
    for (const key of ["input_tokens", "cached_input_tokens", "output_tokens", "reasoning_tokens", "total_tokens"]) {
      total[key] += Number(usage?.[key] || 0)
    }
    total.turns.push(...(usage?.turns || []))
  }
  total.llm_turns = total.turns.length
  return total
}

function providerLogRoot() {
  return path.join(repoRoot, "log", "provider")
}

function providerLogsForAgent(agentId, runIdText, sinceMs = 0, untilMs = Number.POSITIVE_INFINITY) {
  const root = providerLogRoot()
  if (!fs.existsSync(root)) return []
  const runNeedle = String(runIdText || "")
  const agentNeedle = `Agent: ${agentKind(agentId)}`
  const files = []
  for (const day of fs.readdirSync(root)) {
    const dir = path.join(root, day)
    if (!fs.statSync(dir).isDirectory()) continue
    for (const name of fs.readdirSync(dir)) {
      if (name.endsWith(".json")) files.push(path.join(dir, name))
    }
  }
  return files
    .filter((file) => {
      const mtime = fs.statSync(file).mtimeMs
      if (mtime < sinceMs || mtime > untilMs) return false
      let text = ""
      try {
        text = fs.readFileSync(file, "utf8")
      } catch {
        return false
      }
      if (runNeedle && !text.includes(runNeedle)) return false
      return text.includes(agentNeedle) || text.includes(agentId)
    })
    .sort()
}

function usageFromProviderLogs(files) {
  const items = []
  for (const file of files) {
    try {
      const data = readJson(file)
      const usage = data.metrics?.usage || data.response?.usage
      if (!usage) continue
      const total = emptyUsage()
      addUsage(total, {
        input_tokens: usage.input_tokens,
        cached_input_tokens: usage.cached_input_tokens ?? usage.input_tokens_details?.cached_tokens,
        output_tokens: usage.output_tokens,
        reasoning_tokens: usage.reasoning_tokens ?? usage.output_tokens_details?.reasoning_tokens,
        total_tokens: usage.total_tokens,
      })
      items.push(total)
    } catch {}
  }
  return mergeUsage(items)
}

function providerTiming(files) {
  const durations = []
  let success = 0
  let failure = 0
  for (const file of files) {
    try {
      const data = readJson(file)
      const duration = Number(data.duration_ms)
      if (Number.isFinite(duration)) durations.push(duration)
      if (data.success === true) success += 1
      else if (data.success === false) failure += 1
    } catch {}
  }
  const sum = durations.reduce((acc, value) => acc + value, 0)
  return {
    provider_call_count: files.length,
    provider_success_count: success,
    provider_failure_count: failure,
    provider_duration_ms_sum: Math.round(sum),
    provider_duration_ms_avg: durations.length ? Math.round(sum / durations.length) : null,
    provider_duration_ms_min: durations.length ? Math.round(Math.min(...durations)) : null,
    provider_duration_ms_max: durations.length ? Math.round(Math.max(...durations)) : null,
  }
}

function tps(outputTokens, durationMs) {
  const durationSeconds = Number(durationMs) / 1000
  if (!outputTokens || !Number.isFinite(durationSeconds) || durationSeconds <= 0) return null
  return outputTokens / durationSeconds
}

function round(value, digits = 3) {
  if (value === null || value === undefined || !Number.isFinite(Number(value))) return null
  const scale = 10 ** digits
  return Math.round(Number(value) * scale) / scale
}

function rowsToCsv(rows) {
  if (!rows.length) return ""
  const columns = Object.keys(rows[0])
  const escape = (value) => {
    const text = value === null || value === undefined ? "" : String(value)
    return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text
  }
  return [columns.join(","), ...rows.map((row) => columns.map((column) => escape(row[column])).join(","))].join("\n")
}

function metricRow(result) {
  const usage = result.usage || emptyUsage()
  const events = result.events || {}
  const telemetry = result.telemetry || {}
  return {
    case_id: result.case_id,
    task_id: result.task_id,
    difficulty: result.difficulty,
    agent: result.id,
    kind: result.kind,
    model: result.model,
    tura_agent: result.tura_agent || "",
    status: result.run?.status ?? "",
    duration_ms: result.run?.duration_ms ?? result.elapsed_ms ?? "",
    provider_call_count: telemetry.provider_call_count ?? 0,
    provider_duration_ms_sum: telemetry.provider_duration_ms_sum ?? 0,
    input_tokens: usage.input_tokens || 0,
    cached_input_tokens: usage.cached_input_tokens || 0,
    output_tokens: usage.output_tokens || 0,
    reasoning_tokens: usage.reasoning_tokens || 0,
    total_tokens: usage.total_tokens || 0,
    wall_output_tps: round(telemetry.wall_output_tps),
    provider_output_tps: round(telemetry.provider_output_tps),
    command_executions: events.command_executions ?? 0,
    completed_command_executions: events.completed_command_executions ?? 0,
    successful_command_executions: events.successful_command_executions ?? 0,
    failed_command_executions: events.failed_command_executions ?? 0,
    command_success_rate: round(events.command_success_rate),
    command_completion_rate: round(events.command_completion_rate),
    artifact_count: result.artifacts?.length || 0,
    artifact_bytes: (result.artifacts || []).reduce((sum, item) => sum + Number(item.bytes || 0), 0),
    stdout_path: result.stdout_path || "",
    stderr_path: result.stderr_path || "",
    workspace: result.workspace || "",
  }
}

function metricsMarkdown(metrics) {
  const rows = metrics.rows || []
  const lines = ["# Game Prompt Difficulty Benchmark Metrics", ""]
  lines.push(`Run: \`${runPaths.run_id}\``)
  lines.push(`Cases: ${metrics.case_count}`)
  lines.push(`Agents: ${metrics.agent_count}`)
  lines.push("")
  lines.push("| Case | Difficulty | Agent | Status | Duration ms | Total tokens | Commands | Artifacts |")
  lines.push("| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |")
  for (const row of rows) {
    lines.push(`| ${row.case_id} | ${row.difficulty} | ${row.agent} | ${row.status} | ${row.duration_ms} | ${row.total_tokens} | ${row.command_executions} | ${row.artifact_count} |`)
  }
  return `${lines.join("\n")}\n`
}

function writeMetricReports(summary) {
  const reportRoot = path.join(runRoot, "reports")
  mkdirp(reportRoot)
  const rows = (summary.results || []).map(metricRow)
  const metrics = {
    schema: "tura.benchmark.game-prompt-difficulty.metrics.v1",
    run_id: runPaths.run_id,
    case_count: summary.cases?.length || 0,
    agent_count: summary.agents?.length || 0,
    aggregate_usage: mergeUsage((summary.results || []).map((result) => result.usage)),
    rows,
  }
  const files = {
    json: path.join(reportRoot, "metrics.json"),
    csv: path.join(reportRoot, "metrics.csv"),
    markdown: path.join(reportRoot, "metrics.md"),
  }
  writeFile(files.json, JSON.stringify(metrics, null, 2))
  writeFile(files.csv, rowsToCsv(rows))
  writeFile(files.markdown, metricsMarkdown(metrics))
  return { files, metrics }
}

async function runAgent(agentId, benchmarkCase) {
  const agentDir = path.join(runRoot, benchmarkCase.id, agentId)
  const workspace = prepareWorkspace(benchmarkCase, agentId)
  const providerSinceMs = Date.now() - 2000
  const started = performance.now()
  let result
  if (agentKind(agentId) === "codex") result = await runCodex(agentId, benchmarkCase, workspace, agentDir)
  else result = await runTura(agentId, benchmarkCase, workspace, agentDir)
  const providerUntilMs = Date.now() + 2000
  const events = parseJsonl(result.stdout)
  const stdoutUsage = usageFromEvents(events)
  const provider_logs = providerLogsForAgent(agentId, runPaths.run_id, providerSinceMs, providerUntilMs)
  const providerUsage = usageFromProviderLogs(provider_logs)
  const usage = providerUsage.total_tokens > 0 ? providerUsage : stdoutUsage
  const provider_timer = providerTiming(provider_logs)
  const stats = {
    id: agentId,
    kind: agentKind(agentId),
    case_id: benchmarkCase.id,
    task_id: benchmarkCase.task.id,
    task_title: benchmarkCase.task.title,
    difficulty: benchmarkCase.difficulty.id,
    difficulty_label: benchmarkCase.difficulty.label,
    workspace,
    model: agentKind(agentId) === "codex" ? model : turaModel,
    tura_agent: agentKind(agentId).startsWith("tura-") ? turaAgentName(agentId) : null,
    reasoning,
    service_tier: serviceTier,
    elapsed_ms: Math.round(performance.now() - started),
    run: {
      status: result.status,
      signal: result.signal,
      duration_ms: result.duration_ms,
      error: result.error,
      stderr_tail: result.stderr.split(/\r?\n/).filter(Boolean).slice(-25).join("\n"),
    },
    telemetry: {
      provider_since_ms: Math.round(providerSinceMs),
      provider_until_ms: Math.round(providerUntilMs),
      usage_source: providerUsage.total_tokens > 0 ? "provider-log" : "stdout-jsonl",
      stdout_usage: stdoutUsage,
      provider_usage: providerUsage,
      ...provider_timer,
      wall_output_tps: tps(usage.output_tokens, result.duration_ms),
      provider_output_tps: tps(usage.output_tokens, provider_timer.provider_duration_ms_sum),
    },
    usage,
    events: eventStats(events),
    provider_logs,
    artifacts: collectArtifacts(workspace),
    stdout_path: agentKind(agentId) === "codex" ? path.join(agentDir, "codex.stdout.jsonl") : path.join(agentDir, "tura.stdout.jsonl"),
    stderr_path: agentKind(agentId) === "codex" ? path.join(agentDir, "codex.stderr.log") : path.join(agentDir, "tura.stderr.log"),
  }
  writeFile(path.join(agentDir, "agent-summary.json"), JSON.stringify(stats, null, 2))
  return stats
}

async function runWithParallelism(jobs, limit) {
  const results = []
  let cursor = 0
  async function worker() {
    while (cursor < jobs.length) {
      const index = cursor++
      results[index] = await jobs[index]()
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, jobs.length) }, () => worker()))
  return results
}

function flushOutput(text, stream = process.stdout) {
  return new Promise((resolve) => {
    stream.write(`${text}\n`, resolve)
  })
}

async function reportOnlyMode() {
  assert(fs.existsSync(summaryPath), `missing summary for report-only mode: ${summaryPath}`)
  const summary = readJson(summaryPath)
  const { files, metrics } = writeMetricReports(summary)
  summary.metric_files = files
  summary.aggregate_usage = metrics.aggregate_usage
  summary.standard_metrics = { ...(summary.standard_metrics || {}), token_usage: metrics.aggregate_usage }
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  await flushOutput(JSON.stringify({ ok: true, report_only: true, summary_path: summaryPath, metric_files: files }, null, 2))
}

async function main() {
  mkdirp(runRoot)
  if (reportOnly) {
    await reportOnlyMode()
    return
  }
  const cases = buildCases()
  assert(cases.length > 0, "no benchmark cases selected")
  assert(agents.length > 0, "COMMAND_RUN_AGENT_AGENTS selected no supported agents")
  writeFile(path.join(runRoot, "cases.json"), JSON.stringify(cases.map((item) => ({
    id: item.id,
    task_id: item.task.id,
    task_title: item.task.title,
    difficulty: item.difficulty.id,
    difficulty_label: item.difficulty.label,
  })), null, 2))
  for (const benchmarkCase of cases) {
    writeFile(path.join(runRoot, "prompts", `${benchmarkCase.id}.md`), promptForCase(benchmarkCase))
  }
  if (prepOnly) {
    await flushOutput(JSON.stringify({ ok: true, prep_only: true, run_root: runRoot, cases: cases.map((item) => item.id), prompts_dir: path.join(runRoot, "prompts") }, null, 2))
    return
  }
  if (agents.some((agent) => agentKind(agent) === "codex")) {
    assert(fs.existsSync(codexExe), `missing codex exe: ${codexExe}`)
  }
  if (agents.some((agent) => agentKind(agent).startsWith("tura-"))) {
    if (!skipTuraBuild || !fs.existsSync(turaExe)) {
      runOk("cargo", ["build", "-p", "gateway", "--bin", "tura_exec"], { cwd: repoRoot, timeoutMs: 5 * 60_000 })
    }
    assert(fs.existsSync(turaExe), `missing tura exe: ${turaExe}`)
  }
  const jobs = []
  for (const benchmarkCase of cases) {
    for (const agent of agents) {
      jobs.push(async () => {
        console.log(`[game-prompt-difficulty] running ${benchmarkCase.id} with ${agent}`)
        return await runAgent(agent, benchmarkCase)
      })
    }
  }
  const results = await runWithParallelism(jobs, parallelism)
  const aggregateUsage = mergeUsage(results.map((result) => result.usage))
  const summary = normalizeBusinessSummary({
    ok: results.every((result) => result.run.status === 0) || allowFailure,
    accepted_failures: allowFailure,
    evaluation_mode: "cost-and-artifact-human-quality-review",
    task: "Generate playable browser games across four prompt families and four English difficulty levels.",
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    timeout_ms: timeoutMs,
    agents,
    cases: cases.map((item) => ({
      id: item.id,
      task_id: item.task.id,
      task_title: item.task.title,
      difficulty: item.difficulty.id,
      difficulty_label: item.difficulty.label,
    })),
    aggregate_usage: aggregateUsage,
    results,
  }, runPaths)
  const { files, metrics } = writeMetricReports(summary)
  summary.metric_files = files
  summary.aggregate_usage = metrics.aggregate_usage
  summary.standard_metrics.token_usage = metrics.aggregate_usage
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  await flushOutput(JSON.stringify(summary, null, 2))
  process.exit(summary.ok ? 0 : 1)
}

main().catch(async (error) => {
  const summary = normalizeBusinessSummary({
    ok: false,
    error: String(error?.stack || error?.message || error),
    model,
    tura_model: turaModel,
    reasoning,
    service_tier: serviceTier,
    agents,
  }, runPaths)
  writeFile(summaryPath, JSON.stringify(summary, null, 2))
  await flushOutput(JSON.stringify(summary, null, 2), process.stderr)
  process.exit(1)
})
