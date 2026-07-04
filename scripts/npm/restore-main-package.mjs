#!/usr/bin/env node
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const packageJsonPath = path.join(repoRoot, "package.json");
const backupPath = path.join(repoRoot, ".npm-package-json.backup");

if (!existsSync(backupPath)) {
  process.exit(0);
}

writeFileSync(packageJsonPath, readFileSync(backupPath, "utf8"));
rmSync(backupPath, { force: true });
console.error("[tura restore-main-package] restored repository package metadata.");
