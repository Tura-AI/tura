#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const packageJson = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8"));

const requiredFiles = [
  "LICENSE",
  "README.md",
  "scripts/install.ps1",
  "scripts/install.sh",
  "commands/read_media/install.ps1",
  "commands/read_media/install.sh",
  "commands/image_generate/install.ps1",
  "commands/image_generate/install.sh",
  "commands/web_discover/install.ps1",
  "commands/web_discover/install.sh",
  "npm/tura.mjs"
];

const missing = requiredFiles.filter((file) => !existsSync(path.join(repoRoot, file)));
if (missing.length > 0) {
  console.error(`npm package check failed; missing files: ${missing.join(", ")}`);
  process.exit(1);
}

if (packageJson.private === true) {
  console.error("npm package check failed; root package.json must not be private.");
  process.exit(1);
}

if (packageJson.license !== "AGPL-3.0-or-later") {
  console.error("npm package check failed; license must be AGPL-3.0-or-later.");
  process.exit(1);
}

if (!packageJson.bin?.tura) {
  console.error("npm package check failed; bin.tura is required.");
  process.exit(1);
}

console.log("npm package metadata looks ready.");
