#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { resolveWindowsPowerShellCommand } from "./cli-path.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const userArgs = process.argv.slice(2);

const psFlagMap = new Map([
  ["--skip-tui", "-SkipTui"],
  ["--skip-gui", "-SkipGui"],
  ["--skip-tauri", "-SkipTauri"],
  ["--backend-only", "-BackendOnly"],
  ["--clean", "-Clean"],
  ["-clean", "-Clean"],
  ["--help", "-Help"],
  ["-h", "-Help"]
]);

if (userArgs.includes("--skip-apps")) {
  console.error("--skip-apps was removed for release builds because it was ambiguous. Use --backend-only, --skip-tui, --skip-gui, or --skip-tauri explicitly.");
  process.exit(2);
}

function run(command, args) {
  return spawnSync(command, args, {
    cwd: repoRoot,
    stdio: "inherit",
    windowsHide: false
  });
}

if (process.platform === "win32") {
  const script = path.join(repoRoot, "scripts", "build-release.ps1");
  const mappedArgs = userArgs.map((arg) => psFlagMap.get(arg) ?? arg);
  const powerShell = resolveWindowsPowerShellCommand();
  if (!powerShell) {
    console.error("PowerShell was not found. Restore Windows PowerShell to PATH, set TURA_POWERSHELL_PATH, or install PowerShell 7.");
    process.exit(1);
  }
  const result = run(powerShell, ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, ...mappedArgs]);
  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }
  process.exit(result.status ?? 1);
}

const script = path.join(repoRoot, "scripts", "build-release.sh");
const result = run("sh", [script, ...userArgs]);
if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}
process.exit(result.status ?? 1);
