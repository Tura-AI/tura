#!/usr/bin/env node
import { createRequire } from "node:module"
import { spawn } from "node:child_process"
import fs from "node:fs"
import fsp from "node:fs/promises"
import path from "node:path"
import process from "node:process"
import {
  assertReleaseArtifacts,
  caseDefinition,
  caseEnv,
  caseTimeoutMs,
  check,
  delay,
  finishCase,
  freePort,
  prepareContext,
  releaseBinary,
  repoRoot,
  startReleaseGateway,
  stopProcess,
  waitForUrl,
} from "./release_lib_release_entry_harness.mjs"

const tuiRequire = createRequire(path.join(repoRoot, "apps", "tui", "package.json"))
const { chromium } = tuiRequire("playwright")
const webTerminalBin = path.join(repoRoot, "apps", "tui", "scripts", "web-terminal.mjs")

assertReleaseArtifacts()
const ctx = await prepareContext("tui", "password-zip")
const definition = caseDefinition("password-zip", ctx)
const timeoutMs = Math.min(caseTimeoutMs("password-zip"), 240_000)
const screenshotsDir = path.join(ctx.runRoot, "screenshots")
await fsp.mkdir(screenshotsDir, { recursive: true })

let gateway
let web
let browser
let page
let screenshotTimer
let terminalSnapshot = ""
const screenshots = []

try {
  gateway = await startReleaseGateway(ctx)
  web = await startWebTerminal(gateway.url)
  browser = await chromium.launch({ headless: true })
  page = await browser.newPage({ viewport: { width: 1440, height: 920 } })
  await page.goto(`${web.url}/rich?instance=password-zip-release`, { waitUntil: "domcontentloaded" })
  await page.waitForFunction(() => typeof window.__turaFit === "function", null, {
    timeout: 20_000,
  })
  await page.evaluate(() => window.__turaFit())
  await waitForScreenText(/Enter to send|回车输入/u, 30_000)
  await capture("00-initial")
  screenshotTimer = setInterval(() => {
    void capture(`progress-${String(screenshots.length).padStart(3, "0")}`).catch(() => undefined)
  }, 10_000)

  await submitPrompt(definition.prompt)
  const started = Date.now()
  const deadline = Date.now() + timeoutMs
  let gatewayCompletion
  while (Date.now() < deadline) {
    terminalSnapshot = await terminalText()
    gatewayCompletion = await inspectGatewayCompletion(gateway.url, ctx.workspace, definition.sentinel)
    if (
      gatewayCompletion.ok &&
      visibleFinalReplyContainsScore(terminalSnapshot, definition.sentinel) &&
      !/thinking\s+\d+s/i.test(terminalSnapshot)
    ) {
      break
    }
    await delay(1_000)
  }
  terminalSnapshot = await terminalText()
  gatewayCompletion = await inspectGatewayCompletion(gateway.url, ctx.workspace, definition.sentinel)
  const visibleFinalReply =
    visibleFinalReplyContainsScore(terminalSnapshot, definition.sentinel) &&
    !/thinking\s+\d+s/i.test(terminalSnapshot)
  await capture("final")

  const result = {
    command: releaseBinary("tura"),
    args: ["--gateway-url", gateway.url, "--cwd", ctx.workspace, "--rich"],
    status: gatewayCompletion.ok && visibleFinalReply ? 0 : 1,
    signal: gatewayCompletion.ok && visibleFinalReply ? null : "timeout",
    durationMs: Date.now() - started,
    stdout: terminalSnapshot,
    stderr: "",
    stdoutPath: path.join(ctx.logs, "tui-terminal.txt"),
    stderrPath: path.join(ctx.logs, "tui-web-terminal.stderr.log"),
  }
  await fsp.writeFile(result.stdoutPath, terminalSnapshot)
  const cleanup = await cleanupBackends()
  await finishCase(ctx, definition, result, {
    surface: "tui",
    gatewayUrl: gateway.url,
    webUrl: web.url,
    screenshots,
    gatewayCompletion,
    validation: [
      check("tui gateway final assistant reply observed", gatewayCompletion.ok, gatewayCompletion),
      check("tui visible final reply includes marker and harness score", visibleFinalReply, {
        hasSentinel: terminalSnapshot.includes(definition.sentinel),
        hasHarnessScore: visibleTextHasHarnessScore(terminalSnapshot),
        tail: terminalSnapshot.split("\n").slice(-18).join("\n"),
      }),
    ],
    cleanup,
  })
} catch (error) {
  if (page) {
    await capture("failure").catch(() => undefined)
    terminalSnapshot = await terminalText().catch(() => terminalSnapshot)
    await fsp.writeFile(path.join(ctx.logs, "tui-terminal-failure.txt"), terminalSnapshot).catch(
      () => undefined,
    )
  }
  throw error
} finally {
  if (screenshotTimer) clearInterval(screenshotTimer)
  await browser?.close().catch(() => undefined)
  await stopProcess(web?.child)
  await stopProcess(gateway?.child)
}

async function startWebTerminal(gatewayUrl) {
  const port = await freePort()
  const child = spawn(process.execPath, [webTerminalBin], {
    cwd: path.join(repoRoot, "apps", "tui"),
    env: {
      ...caseEnv(ctx),
      PORT: String(port),
      TURA_GATEWAY_URL: gatewayUrl,
      TURA_CWD: ctx.workspace,
      TURA_HOME: ctx.turaHome,
      TURA_TUI_BIN: releaseBinary("tura"),
      FORCE_COLOR: "1",
    },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  })
  child.stdout?.pipe(fs.createWriteStream(path.join(ctx.logs, "web-terminal.stdout.log")))
  child.stderr?.pipe(fs.createWriteStream(path.join(ctx.logs, "web-terminal.stderr.log")))
  const url = `http://127.0.0.1:${port}`
  await waitForUrl(`${url}/`, child, 30_000)
  return { child, url }
}

async function submitPrompt(prompt) {
  await page.locator("#terminal").click({ timeout: 5_000 })
  await page.evaluate(async (value) => {
    await window.__turaSendInput(value)
    await window.__turaSendInput("\r")
  }, prompt)
  await waitForScreenText(/Long CLI refactor task|zip-password-finder/u, 10_000)
}

async function waitForScreenText(expected, timeout) {
  const deadline = Date.now() + timeout
  while (Date.now() < deadline) {
    const text = await terminalText()
    if (typeof expected === "string" ? text.includes(expected) : expected.test(text)) return text
    await delay(250)
  }
  throw new Error(`timed out waiting for TUI screen text ${String(expected)}`)
}

async function terminalText() {
  return page.evaluate(() =>
    [...document.querySelectorAll(".xterm-rows > div")]
      .map((node) => node.textContent ?? "")
      .join("\n"),
  )
}

async function capture(name) {
  const file = path.join(screenshotsDir, `${Date.now()}-${name}.png`)
  await page.screenshot({ path: file, fullPage: false })
  screenshots.push(file)
}

async function inspectGatewayCompletion(gatewayUrl, workspace, sentinel) {
  try {
    const sessions = await requestJson(
      `${gatewayUrl}/session?directory=${encodeURIComponent(workspace)}&includeChildren=true`,
    )
    const newest = [...(Array.isArray(sessions) ? sessions : [])].sort(
      (left, right) => Number(right.updated_at ?? 0) - Number(left.updated_at ?? 0),
    )[0]
    if (!newest?.id) return { ok: false, error: "session not found" }
    const messages = await requestJson(`${gatewayUrl}/session/${encodeURIComponent(newest.id)}/message`)
    const assistantTexts = (Array.isArray(messages) ? messages : [])
      .filter((message) => (message?.role ?? message?.info?.role) === "assistant")
      .map(messageText)
      .map((text) => text.trim())
      .filter(Boolean)
    const lastAssistant = assistantTexts.at(-1) ?? ""
    const state = String(newest.state ?? "")
    const status = String(newest.status ?? "")
    const failed = /failed|error/i.test(`${state} ${status}`) || /MANO failed/i.test(lastAssistant)
    return {
      ok: !failed && lastAssistant.includes(sentinel),
      sessionId: newest.id,
      state,
      status,
      lastAssistant: lastAssistant.slice(-1500),
    }
  } catch (error) {
    return { ok: false, error: error instanceof Error ? error.message : String(error) }
  }
}

async function requestJson(url) {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), 2_000)
  try {
    const response = await fetch(url, { signal: controller.signal })
    const text = await response.text()
    if (!response.ok) throw new Error(`${response.status} ${text.slice(0, 300)}`)
    return text ? JSON.parse(text) : null
  } finally {
    clearTimeout(timer)
  }
}

function messageText(message) {
  return (message.parts ?? [])
    .filter((part) => part.type === "text" || part.type === "message" || !part.type)
    .map((part) => part.text ?? part.content ?? "")
    .filter((text) => !looksLikeInternalTaskStatus(text))
    .join("")
}

function looksLikeInternalTaskStatus(text) {
  const trimmed = String(text ?? "").trim()
  if (!trimmed) return false
  if (/^(?:doing|done|question)\s*:\s*\{\s*\}$/iu.test(trimmed)) return true
  try {
    const parsed = JSON.parse(trimmed)
    return Boolean(parsed?.task_status || parsed?.status || parsed?.task_group)
  } catch {
    return false
  }
}

function visibleFinalReplyContainsScore(text, sentinel) {
  return text.includes(sentinel) && visibleTextHasHarnessScore(text)
}

function visibleTextHasHarnessScore(text) {
  return (
    /acceptance harness score/i.test(text) ||
    /score\s*[:=]\s*6\s*\/\s*6/i.test(text) ||
    /ZIP_PASSWORD_REFACTOR_ACCEPTANCE_OK\s+score\s*=\s*6\s*\/\s*6/i.test(text)
  )
}

async function cleanupBackends() {
  const { shutdownBackendDaemons } = await import("./release_lib_release_entry_harness.mjs")
  return shutdownBackendDaemons(ctx)
}
