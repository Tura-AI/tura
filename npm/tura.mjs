#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const executable = process.platform === "win32" ? "tura.exe" : "tura";
const releaseBin = path.join(packageRoot, "target", "release", executable);

if (!existsSync(releaseBin)) {
  console.error("Tura release binary was not found.");
  console.error("Run `npm run install:deps` and `npm run build:release` from the package root, then retry.");
  process.exit(1);
}

const result = spawnSync(releaseBin, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: false
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
