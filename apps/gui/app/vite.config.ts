import { spawn } from "node:child_process"
import { existsSync } from "node:fs"
import { createServer } from "node:net"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { defineConfig, type Plugin } from "vite"
import solid from "vite-plugin-solid"

export default defineConfig({
  plugins: [turaGatewayStartupPlugin(), solid()],
  server: {
    host: "127.0.0.1",
    port: 5174,
    strictPort: true,
  },
})

// The GUI dev server starts the repo-local target/debug/tura_gateway on port 4126.
const isWindows = process.platform === "win32"
const gatewayBinaryName = isWindows ? "tura_gateway.exe" : "tura_gateway"
const DEV_GATEWAY_URL = "http://127.0.0.1:4126"
let gatewayStartupPromise: Promise<void> | undefined
let ownedGatewayChild: ReturnType<typeof spawn> | undefined
let ownedGatewayShutdownMode: "stdin-eof" | "kill" | undefined

function turaGatewayStartupPlugin(): Plugin {
  return {
    name: "tura-gateway-startup",
    configureServer(server) {
      server.httpServer?.once("close", () => {
        killOwnedGateway()
      })
      server.middlewares.use("/__tura/start-gateway", async (req, res) => {
        if (req.method !== "POST") {
          res.statusCode = 405
          res.end("method not allowed")
          return
        }
        try {
          const body = await readJsonBody(req)
          const gatewayUrl = typeof body.gatewayUrl === "string" ? body.gatewayUrl : DEV_GATEWAY_URL
          if (await healthOk(gatewayUrl)) {
            writeJson(res, { ok: true, status: "connected" })
            return
          }
          const root = repoRoot()
          const status = resolveGatewayBinary(root) ? "starting" : "building"
          gatewayStartupPromise ??= startGatewayTask(root, gatewayUrl).finally(() => {
            gatewayStartupPromise = undefined
          })
          writeJson(res, { ok: true, status })
        } catch (error) {
          res.statusCode = 500
          writeJson(res, { ok: false, error: error instanceof Error ? error.message : String(error) })
        }
      })
    },
  }
}

function resolveGatewayBinary(root: string): string | undefined {
  const candidates = [
    join(root, "target", "debug", gatewayBinaryName),
    join(root, "target", "release", gatewayBinaryName),
  ]
  return candidates.find((candidate) => existsSync(candidate))
}

async function startGatewayTask(root: string, gatewayUrl: string): Promise<void> {
  if (await healthOk(gatewayUrl)) return
  if (!(await canBindGatewayUrl(gatewayUrl))) {
    throw new Error(`gateway port ${gatewayPort(gatewayUrl) ?? "unknown"} is occupied by an unknown or foreign process`)
  }
  let binary = resolveGatewayBinary(root)
  if (!binary) {
    // No dev gateway yet: build target/debug/tura_gateway.
    if (isWindows) {
      await runProcess(
        "powershell",
        ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", join(root, "scripts", "build-debug.ps1"), "-SkipTui"],
        root,
      )
    } else {
      await runProcess("sh", [join(root, "scripts", "build-debug.sh"), "--skip-tui"], root)
    }
    binary = resolveGatewayBinary(root)
  }
  if (!binary) throw new Error("tura_gateway binary not found after build")
  if (await healthOk(gatewayUrl)) return
  if (!(await canBindGatewayUrl(gatewayUrl))) {
    throw new Error(`gateway port ${gatewayPort(gatewayUrl) ?? "unknown"} is occupied by an unknown or foreign process`)
  }
  const port = gatewayPort(gatewayUrl)
  const child = spawn(binary, [], {
    cwd: root,
    stdio: ["pipe", "ignore", "ignore"],
    windowsHide: true,
    env: {
      ...process.env,
      ...(port ? { PORT: port } : {}),
      TURA_GATEWAY_SHUTDOWN_ON_STDIN_EOF: "1",
    },
  })
  ownedGatewayChild = child
  ownedGatewayShutdownMode = "stdin-eof"
  ;(child.stdin as (NodeJS.WritableStream & { unref?: () => void }) | null)?.unref?.()
  child.once("exit", () => {
    if (ownedGatewayChild === child) ownedGatewayChild = undefined
    if (ownedGatewayChild === undefined) ownedGatewayShutdownMode = undefined
  })
}

async function healthOk(gatewayUrl: string): Promise<boolean> {
  try {
    const controller = new AbortController()
    const timer = setTimeout(() => controller.abort(), 1200)
    const response = await fetch(`${gatewayUrl.replace(/\/+$/u, "")}/global/health`, { signal: controller.signal })
    clearTimeout(timer)
    return response.ok
  } catch {
    return false
  }
}

function runProcess(command: string, args: string[], cwd: string): Promise<void> {
  return new Promise((resolveProcess, reject) => {
    const child = spawn(command, args, { cwd, stdio: "ignore", windowsHide: true })
    child.on("error", reject)
    child.on("exit", (code) => {
      if (code === 0) resolveProcess()
      else reject(new Error(`${command} ${args.join(" ")} exited with ${code ?? "signal"}`))
    })
  })
}

function repoRoot(): string {
  let current = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..")
  for (let depth = 0; depth < 4; depth += 1) {
    if (existsSync(join(current, "Cargo.toml")) && existsSync(join(current, "crates", "gateway"))) return current
    const parent = dirname(current)
    if (parent === current) break
    current = parent
  }
  return resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..")
}


function canBindGatewayUrl(gatewayUrl: string): Promise<boolean> {
  const port = Number(gatewayPort(gatewayUrl))
  if (!Number.isInteger(port) || port <= 0) return Promise.resolve(false)
  return new Promise((resolveBind) => {
    const server = createServer()
    server.once("error", () => resolveBind(false))
    server.listen(port, "127.0.0.1", () => {
      server.close(() => resolveBind(true))
    })
  })
}
function gatewayPort(gatewayUrl: string): string | undefined {
  try {
    return new URL(gatewayUrl).port || undefined
  } catch {
    return undefined
  }
}

function readJsonBody(req: import("node:http").IncomingMessage): Promise<Record<string, unknown>> {
  return new Promise((resolveBody, reject) => {
    const chunks: Buffer[] = []
    req.on("data", (chunk) => chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)))
    req.on("error", reject)
    req.on("end", () => {
      const text = Buffer.concat(chunks).toString("utf8").trim()
      if (!text) {
        resolveBody({})
        return
      }
      try {
        resolveBody(JSON.parse(text) as Record<string, unknown>)
      } catch (error) {
        reject(error)
      }
    })
  })
}

function writeJson(res: import("node:http").ServerResponse, payload: unknown): void {
  res.setHeader("content-type", "application/json")
  res.end(JSON.stringify(payload))
}

function killOwnedGateway(): void {
  const child = ownedGatewayChild
  if (!child) return
  ownedGatewayChild = undefined
  const shutdownMode = ownedGatewayShutdownMode
  ownedGatewayShutdownMode = undefined
  try {
    if (shutdownMode === "stdin-eof") {
      child.stdin?.end()
      const timer = setTimeout(() => {
        if (child.exitCode === null && !child.killed) child.kill()
      }, 5_000)
      timer.unref()
    } else {
      child.kill()
    }
  } catch {
    // Already exited.
  }
}
