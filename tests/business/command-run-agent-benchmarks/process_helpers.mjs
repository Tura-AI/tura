import { spawnSync } from "node:child_process"
import { performance } from "node:perf_hooks"
import process from "node:process"

export function isolatedProcessOptions(options = {}) {
  return {
    ...options,
    detached: process.platform === "win32" ? options.detached : true,
  }
}

export function killProcessTree(pid) {
  if (!pid) return
  if (process.platform === "win32") {
    spawnSync("taskkill", ["/pid", String(pid), "/t", "/f"], { windowsHide: true })
    return
  }
  try {
    process.kill(-pid, "SIGTERM")
  } catch {
    try {
      process.kill(pid, "SIGTERM")
    } catch {}
  }
  const started = performance.now()
  while (performance.now() - started < 100) {}
  try {
    process.kill(-pid, "SIGKILL")
  } catch {
    try {
      process.kill(pid, "SIGKILL")
    } catch {}
  }
}

export function endStream(stream) {
  try {
    stream?.end()
  } catch {}
}
