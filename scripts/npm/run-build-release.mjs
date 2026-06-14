#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const userArgs = process.argv.slice(2);

const psFlagMap = new Map([
  ["--skip-tui", "-SkipTui"],
  ["--clean", "-Clean"],
  ["-clean", "-Clean"],
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
  const script = path.join(repoRoot, "scripts", "build-release.ps1");
  const mappedArgs = userArgs.map((arg) => psFlagMap.get(arg) ?? arg);
  let result = run("pwsh", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, ...mappedArgs]);
  if (result.error?.code === "ENOENT") {
    result = run("powershell.exe", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, ...mappedArgs]);
  }
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
