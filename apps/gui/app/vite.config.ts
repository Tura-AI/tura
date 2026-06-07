import { spawn } from "node:child_process"
import { existsSync } from "node:fs"
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

const gatewayBinaryName = process.platform === "win32" ? "gateway.exe" : "gateway"
let gatewayStartupPromise: Promise<void> | undefined

function turaGatewayStartupPlugin(): Plugin {
  return {
    name: "tura-gateway-startup",
    configureServer(server) {
      server.middlewares.use("/__tura/start-gateway", async (req, res) => {
        if (req.method !== "POST") {
          res.statusCode = 405
          res.end("method not allowed")
          return
        }
        try {
          const body = await readJsonBody(req)
          const gatewayUrl = typeof body.gatewayUrl === "string" ? body.gatewayUrl : "http://127.0.0.1:4096"
          if (await healthOk(gatewayUrl)) {
            writeJson(res, { ok: true, status: "connected" })
            return
          }
          const root = repoRoot()
          const binary = join(root, "target", "debug", gatewayBinaryName)
          const status = existsSync(binary) ? "starting" : "building"
          gatewayStartupPromise ??= startGatewayTask(root, binary, gatewayUrl).finally(() => {
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

async function startGatewayTask(root: string, binary: string, gatewayUrl: string): Promise<void> {
  if (await healthOk(gatewayUrl)) return
  if (!existsSync(binary)) {
    await runProcess("cargo", ["build", "-p", "gateway", "--bin", "gateway"], root)
  }
  if (await healthOk(gatewayUrl)) return
  const port = gatewayPort(gatewayUrl)
  const child = spawn(binary, [], {
    cwd: root,
    detached: true,
    stdio: "ignore",
    windowsHide: true,
    env: { ...process.env, ...(port ? { PORT: port } : {}) },
  })
  child.unref()
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
