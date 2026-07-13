#!/usr/bin/env node
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
} from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  cliPathRegistrationSkipped,
  ensureWindowsPowerShellCommand,
  registerCliPath,
} from "./cli-path.mjs";
import {
  executableName,
  executableNames,
  missingReleaseFiles,
  missingReleaseRuntimeFiles,
  platformPackageName,
} from "./release-artifacts.mjs";

const packageRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
);
const packageJsonPath = path.join(packageRoot, "package.json");
const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
const releaseDir = path.join(packageRoot, "target", "release");
const require = createRequire(import.meta.url);

function log(message) {
  console.log(`[tura postinstall] ${message}`);
}

function fail(message) {
  console.error(`[tura postinstall] ${message}`);
  process.exit(1);
}

function pathKey() {
  return process.platform === "win32" ? "Path" : "PATH";
}

function prependExistingPathEntries(entries) {
  const key = pathKey();
  const delimiter = path.delimiter;
  const current = process.env[key] || "";
  const existing = current.split(delimiter).filter(Boolean);
  const normalized = new Set(existing.map((entry) => path.resolve(entry).toLowerCase()));
  const next = [];
  for (const entry of entries) {
    if (!entry || !existsSync(entry)) {
      continue;
    }
    const resolved = path.resolve(entry);
    const marker = resolved.toLowerCase();
    if (!normalized.has(marker)) {
      next.push(resolved);
      normalized.add(marker);
    }
  }
  if (next.length > 0) {
    process.env[key] = [...next, ...existing].join(delimiter);
  }
}

function refreshRuntimePath() {
  if (process.platform === "win32") {
    prependExistingPathEntries([
      "C:\\Program Files\\PowerShell\\7",
      "C:\\Program Files (x86)\\PowerShell\\7",
    ]);
    return;
  }
  prependExistingPathEntries([
    "/opt/homebrew/bin",
    "/usr/local/bin",
  ]);
}

function runtimeDependencyCheckSkipped() {
  return (
    process.env.TURA_NPM_SKIP_RUNTIME_DEPENDENCY_CHECK === "1" ||
    process.env.TURA_NPM_SKIP_RUNTIME_DEPENDENCY_CHECK === "true"
  );
}

function commandExists(name) {
  const result = spawnSync(process.platform === "win32" ? "where.exe" : "sh", process.platform === "win32" ? [name] : ["-c", `command -v ${name}`], {
    shell: false,
    stdio: "ignore",
    windowsHide: true,
  });
  return !result.error && (result.status ?? 1) === 0;
}

function requireRuntimeCommand(name, hint) {
  if (commandExists(name)) {
    return;
  }
  fail(`${name} was not found. ${hint}`);
}

function ensureRuntimeDependencies() {
  if (runtimeDependencyCheckSkipped()) {
    log("runtime dependency check skipped by TURA_NPM_SKIP_RUNTIME_DEPENDENCY_CHECK");
    return;
  }
  refreshRuntimePath();
  if (process.platform === "win32") {
    ensurePowerShellCliPath();
    return;
  }
  requireRuntimeCommand("sh", "Install a POSIX shell and ensure it is on PATH.");
  requireRuntimeCommand("tar", "Install tar and ensure it is on PATH.");
  if (process.platform === "darwin") {
    requireRuntimeCommand("zsh", "Install zsh or set up macOS command-line tools so zsh is available.");
  } else if (process.platform === "linux") {
    requireRuntimeCommand("bash", "Install bash and ensure it is on PATH.");
  }
}

function registerCli() {
  if (cliPathRegistrationSkipped()) {
    log("CLI registration skipped by TURA_NPM_SKIP_CLI_REGISTRATION");
    return;
  }
  registerCliPath({ packageRoot, releaseDir, quiet: true });
  log("CLI command registered");
}

function ensurePowerShellCliPath() {
  if (process.platform !== "win32") {
    return;
  }
  const powerShell = ensureWindowsPowerShellCommand({ quiet: true });
  if (!powerShell) {
    fail("PowerShell was not found. Restore Windows PowerShell to PATH, set TURA_POWERSHELL_PATH, or install PowerShell 7.");
  }
}

function verifyInstall() {
  const missingRelease = missingReleaseFiles(packageRoot);
  if (missingRelease.length > 0) {
    fail(
      `release archive did not install required files: ${missingRelease.map((file) => path.relative(packageRoot, file)).join(", ")}`,
    );
  }
  const missingRuntime = missingReleaseRuntimeFiles(packageRoot);
  if (missingRuntime.length > 0) {
    fail(
      `release archive did not install runtime config files: ${missingRuntime.map((file) => path.relative(packageRoot, file)).join(", ")}`,
    );
  }
}

function markReleaseExecutablesRunnable() {
  if (process.platform === "win32") {
    return;
  }
  for (const name of executableNames) {
    const file = path.join(releaseDir, executableName(name));
    if (existsSync(file)) {
      chmodSync(file, 0o755);
    }
  }
}

function resolvePlatformPackageRoot() {
  if (process.env.TURA_NPM_PLATFORM_PACKAGE_DIR) {
    return path.resolve(process.env.TURA_NPM_PLATFORM_PACKAGE_DIR);
  }

  const packageName = platformPackageName();
  try {
    return path.dirname(
      require.resolve(`${packageName}/package.json`, { paths: [packageRoot] }),
    );
  } catch {
    return null;
  }
}

function installFromPlatformPackage() {
  const platformRoot = resolvePlatformPackageRoot();
  if (!platformRoot) {
    return false;
  }

  const missingPlatform = missingReleaseFiles(platformRoot);
  if (missingPlatform.length > 0) {
    fail(
      `platform package ${platformPackageName()} is incomplete: ${missingPlatform.map((file) => path.relative(platformRoot, file)).join(", ")}`,
    );
  }
  const missingRuntime = missingReleaseRuntimeFiles(platformRoot);
  if (missingRuntime.length > 0) {
    fail(
      `platform package ${platformPackageName()} is missing runtime config files: ${missingRuntime.map((file) => path.relative(platformRoot, file)).join(", ")}`,
    );
  }

  mkdirSync(path.dirname(releaseDir), { recursive: true });
  cpSync(path.join(platformRoot, "target", "release"), releaseDir, {
    recursive: true,
  });
  markReleaseExecutablesRunnable();
  verifyInstall();
  registerCli();
  log(`release binaries installed from ${platformPackageName()}`);
  return true;
}

ensureRuntimeDependencies();

const existingMissing = missingReleaseFiles(packageRoot);
if (existingMissing.length === 0) {
  verifyInstall();
  registerCli();
  log("release binaries already present");
  process.exit(0);
}

if (installFromPlatformPackage()) {
  process.exit(0);
}

const packageName = platformPackageName();
fail(
  `platform package ${packageName} is unavailable; reinstall ${packageJson.name} with optional dependencies enabled after confirming ${packageName}@${packageJson.version} is published`,
);
