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
  "scripts/register-cli.ps1",
  "scripts/register-cli.sh",
  "scripts/unregister-cli.ps1",
  "scripts/unregister-cli.sh",
  "commands/read_media/install.ps1",
  "commands/read_media/install.sh",
  "commands/generate_media/install.ps1",
  "commands/generate_media/install.sh",
  "commands/web_discover/install.ps1",
  "commands/web_discover/install.sh",
  "apps/gui/package.json",
  "apps/gui/bun.lock",
  "apps/gui/app/package.json",
  "apps/gui/app/vite.config.ts",
  "apps/gui/sdk/gateway/package.json",
  "apps/tauri/package.json",
  "apps/tauri/bun.lock",
  "npm/tura.mjs",
  "scripts/npm/cli-path.mjs",
  "scripts/npm/release-artifacts.mjs",
  "scripts/npm/install-release.mjs",
  "scripts/npm/package-platform.mjs",
  "scripts/npm/package-release.mjs",
  "scripts/npm/stage-main-package.mjs",
  "scripts/npm/restore-main-package.mjs",
  "scripts/npm/verify-platform-install.mjs"
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

if (packageJson.name !== "tura-ai") {
  console.error("npm package check failed; root package name must be tura-ai.");
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

if (!packageJson.scripts?.postinstall || !packageJson.scripts?.prepack || !packageJson.scripts?.postpack) {
  console.error("npm package check failed; postinstall, prepack, and postpack scripts are required.");
  process.exit(1);
}

const expectedPlatformPackages = ["tura-darwin-arm64", "tura-darwin-x64", "tura-linux-x64", "tura-win32-x64"];
for (const packageName of expectedPlatformPackages) {
  if (packageJson.optionalDependencies?.[packageName] !== packageJson.version) {
    console.error(`npm package check failed; optional dependency ${packageName} must match version ${packageJson.version}.`);
    process.exit(1);
  }
}

if (!Array.isArray(packageJson.keywords) || packageJson.keywords.length === 0) {
  console.error("npm package check failed; keywords are required.");
  process.exit(1);
}

if (!packageJson.repository?.url || !packageJson.homepage || !packageJson.bugs?.url) {
  console.error("npm package check failed; repository, homepage, and bugs URLs are required.");
  process.exit(1);
}

if (packageJson.publishConfig?.access !== "public") {
  console.error("npm package check failed; publishConfig.access must be public.");
  process.exit(1);
}

console.error("npm package metadata looks ready.");
