#!/usr/bin/env node
import { existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { verifyInstalledRuntime } from "./verify-installed-runtime.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const packageJson = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8"));
const modeArgument = process.argv.slice(2).find((argument) => argument.startsWith("--install-mode="));
const installMode = modeArgument?.slice("--install-mode=".length) ?? "local";

function fail(message) {
  throw new Error(`[tura install-and-verify-published] ${message}`);
}

function npmCommand() {
  return process.platform === "win32" ? "npm.cmd" : "npm";
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, {
    cwd,
    env: process.env,
    shell: process.platform === "win32",
    stdio: "inherit",
    windowsHide: true,
  });
  if (result.error) fail(result.error.message);
  if ((result.status ?? 1) !== 0) fail(`${command} ${args.join(" ")} exited ${result.status}`);
}

if (!new Set(["global", "local"]).has(installMode)) {
  fail(`unsupported install mode: ${installMode}`);
}

const suffix = `${packageJson.version}-${installMode}-${process.platform}-${process.arch}`.replaceAll(/[^a-zA-Z0-9_.-]/gu, "-");
const root = path.resolve(process.env.RUNNER_TEMP || tmpdir(), `tura-published-install-${suffix}`);
const tempRoot = path.resolve(process.env.RUNNER_TEMP || tmpdir());
if (!root.startsWith(`${tempRoot}${path.sep}`)) fail(`refusing temp path outside ${tempRoot}: ${root}`);
rmSync(root, { recursive: true, force: true });
mkdirSync(root, { recursive: true });

const spec = `${packageJson.name}@${packageJson.version}`;
let installRoot;
let binPath;
if (installMode === "global") {
  run(npmCommand(), ["install", "--global", "--prefix", root, spec, "--no-audit", "--no-fund"], repoRoot);
  installRoot = process.platform === "win32"
    ? path.join(root, "node_modules")
    : path.join(root, "lib", "node_modules");
  binPath = process.platform === "win32"
    ? path.join(root, "tura.cmd")
    : path.join(root, "bin", "tura");
} else {
  run(npmCommand(), ["init", "-y", "--silent"], root);
  run(npmCommand(), ["install", spec, "--no-audit", "--no-fund"], root);
  installRoot = path.join(root, "node_modules");
  binPath = path.join(root, "node_modules", ".bin", process.platform === "win32" ? "tura.cmd" : "tura");
}

if (!existsSync(binPath)) fail(`installed tura wrapper is missing: ${binPath}`);
run(binPath, ["--help"], root);
await verifyInstalledRuntime({ installRoot, binPath });
console.log(`[tura install-and-verify-published] verified ${spec} (${installMode}) with npm ${process.env.npm_config_user_agent ?? "unknown"}`);
