#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { resolveWindowsPowerShellCommand } from "./cli-path.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const userArgs = process.argv.slice(2);

const psFlagMap = new Map([
  ["--skip-commands", "-SkipCommands"],
  ["--skip-apps", "-SkipApps"],
  ["--skip-uv", "-SkipUv"],
  ["--skip-bun", "-SkipBun"],
  ["--environment-only", "-EnvironmentOnly"],
  ["--check-only", "-CheckOnly"],
  ["--offline", "-Offline"],
  ["--help", "-Help"],
  ["-h", "-Help"]
]);

function run(command, args) {
  return spawnSync(command, args, {
    cwd: repoRoot,
    stdio: "inherit",
    windowsHide: false
  });
}

if (process.platform === "win32") {
  const script = path.join(repoRoot, "scripts", "install.ps1");
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

const script = path.join(repoRoot, "scripts", "install.sh");
const result = run("sh", [script, ...userArgs]);
if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}
process.exit(result.status ?? 1);
