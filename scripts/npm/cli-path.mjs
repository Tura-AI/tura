#!/usr/bin/env node
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const blockStart = "# >>> tura release commands >>>";
const blockEnd = "# <<< tura release commands <<<";

function fail(message) {
  throw new Error(message);
}

function executableName(name, platform = process.platform) {
  return platform === "win32" ? `${name}.exe` : name;
}

function say(message, quiet) {
  if (!quiet) {
    console.log(message);
  }
}

function packageRootFromThisFile() {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
}

export function defaultReleaseDir(packageRoot = packageRootFromThisFile()) {
  return path.join(packageRoot, "target", "release");
}

export function cliPathRegistrationSkipped(env = process.env) {
  return (
    env.TURA_NPM_SKIP_CLI_REGISTRATION === "1" ||
    env.TURA_NPM_SKIP_CLI_REGISTRATION === "true"
  );
}

function assertReleaseBinaries(releaseDir) {
  for (const name of ["tura", "tura_exec"]) {
    const executable = path.join(releaseDir, executableName(name));
    if (!existsSync(executable)) {
      fail(`Missing ${executable}. Install the Tura release binaries first.`);
    }
  }
}

function normalizeProfileText(value) {
  return value.replace(/\r\n/g, "\n");
}

function removeTuraBlock(value) {
  const source = normalizeProfileText(value);
  const pattern = new RegExp(
    `\\n?${blockStart.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}[\\s\\S]*?${blockEnd.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\n?`,
    "g",
  );
  return source.replace(pattern, "\n").replace(/\n{3,}/g, "\n\n").trimEnd();
}

function posixHome() {
  return process.env.HOME || homedir();
}

function posixProfiles(home = posixHome(), platform = process.platform) {
  const profiles = [
    path.join(home, ".profile"),
    path.join(home, ".bash_profile"),
    path.join(home, ".bashrc"),
    path.join(home, ".zprofile"),
    path.join(home, ".zshrc"),
  ];
  const ensure = [path.join(home, ".profile")];
  if (platform === "darwin") {
    ensure.push(path.join(home, ".zprofile"), path.join(home, ".zshrc"));
  }
  return { profiles, ensure };
}

function ensureFile(file) {
  if (existsSync(file)) {
    return;
  }
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, "");
}

function registerPosix(releaseDir, { quiet = false } = {}) {
  const resolvedReleaseDir = path.resolve(releaseDir);
  const { profiles, ensure } = posixProfiles();
  for (const profile of ensure) {
    ensureFile(profile);
  }
  const block = `${blockStart}\nexport PATH="${resolvedReleaseDir}:$PATH"\n${blockEnd}\n`;
  for (const profile of profiles) {
    if (!existsSync(profile)) {
      continue;
    }
    const existing = readFileSync(profile, "utf8");
    const cleaned = removeTuraBlock(existing);
    const separator = cleaned.length > 0 ? "\n\n" : "";
    writeFileSync(profile, `${cleaned}${separator}${block}`, "utf8");
  }
  say(`Registered Tura CLI path: ${resolvedReleaseDir}`, quiet);
  return true;
}

function unregisterPosix(releaseDir, packageRoot, { quiet = false } = {}) {
  const { profiles } = posixProfiles();
  for (const profile of profiles) {
    if (!existsSync(profile)) {
      continue;
    }
    const existing = readFileSync(profile, "utf8");
    const cleaned = removeTuraBlock(existing);
    if (cleaned !== normalizeProfileText(existing).trimEnd()) {
      writeFileSync(profile, `${cleaned}${cleaned ? "\n" : ""}`, "utf8");
    }
  }
  rmSync(path.join(packageRoot, "cli-bin"), { recursive: true, force: true });
  say(`Removed Tura CLI path: ${path.resolve(releaseDir)}`, quiet);
  return true;
}

function pathRegisteredPosix(releaseDir) {
  const resolvedReleaseDir = path.resolve(releaseDir);
  const { profiles } = posixProfiles();
  return profiles
    .filter((profile) => existsSync(profile))
    .some((profile) => readFileSync(profile, "utf8").includes(resolvedReleaseDir));
}

function envValue(env, names) {
  for (const name of names) {
    const value = env[name];
    if (typeof value === "string" && value.trim()) {
      return value;
    }
  }
  return null;
}

function pathEntries(env) {
  const value = envValue(env, ["Path", "PATH", "path"]);
  return value
    ? value
        .split(path.win32.delimiter)
        .map((entry) => entry.trim().replace(/^"|"$/g, ""))
        .filter(Boolean)
    : [];
}

function setPathEntries(env, entries) {
  const key = env.Path !== undefined ? "Path" : env.PATH !== undefined ? "PATH" : "Path";
  env[key] = entries.join(path.win32.delimiter);
}

function normalizeWindowsPathEntry(entry) {
  return path.win32.normalize(entry).replace(/[\\/]+$/, "").toLowerCase();
}

function prependPathEntries(env, entries) {
  const existing = pathEntries(env);
  const known = new Set(existing.map(normalizeWindowsPathEntry));
  const next = [];
  for (const entry of entries) {
    if (!entry) continue;
    const normalized = normalizeWindowsPathEntry(entry);
    if (!known.has(normalized)) {
      next.push(entry);
      known.add(normalized);
    }
  }
  setPathEntries(env, [...next, ...existing]);
}






















































function windowsExecutableNames(name, env) {
  if (path.win32.extname(name)) {
    return [name];
  }
  const pathExt = envValue(env, ["PATHEXT", "PathExt"]) || ".COM;.EXE;.BAT;.CMD";
  return pathExt.split(";").filter(Boolean).map((extension) => `${name}${extension.toLowerCase()}`);
}

function pushUnique(candidates, value) {
  if (value && !candidates.includes(value)) {
    candidates.push(value);
  }
}

function windowsPathCommand(name, { env = process.env, pathExists = existsSync } = {}) {
  for (const entry of pathEntries(env)) {
    for (const executable of windowsExecutableNames(name, env)) {
      const candidate = path.win32.join(entry, executable);
      if (pathExists(candidate)) {
        return candidate;
      }
    }
  }
  return null;
}

function whereWindowsCommand(name, { env = process.env, spawnSyncFn = spawnSync } = {}) {
  const commandName = path.win32.extname(name) ? name : `${name}.exe`;
  const result = spawnSyncFn("where.exe", [commandName], {
    env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  if (result.error || (result.status ?? 1) !== 0) {
    return null;
  }
  return result.stdout
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .find(Boolean) ?? null;
}

function powerShell7InstallCandidates(env) {
  const candidates = [];
  const programFiles = envValue(env, ["ProgramFiles", "PROGRAMFILES"]) || "C:\\Program Files";
  const programFilesX86 = envValue(env, ["ProgramFiles(x86)", "PROGRAMFILES(X86)"]);
  for (const root of [programFiles, programFilesX86]) {
    pushUnique(candidates, root && path.win32.join(root, "PowerShell", "7", "pwsh.exe"));
  }
  return candidates;
}




















































export function resolveWindowsPowerShellCommand({ env = process.env, pathExists = existsSync } = {}) {
  const candidates = [];
  pushUnique(candidates, env.TURA_POWERSHELL_PATH);

  for (const entry of pathEntries(env)) {
    for (const name of ["pwsh", "powershell"]) {
      for (const executable of windowsExecutableNames(name, env)) {
        pushUnique(candidates, path.win32.join(entry, executable));
      }
    }
  }

  for (const candidate of powerShell7InstallCandidates(env)) {
    pushUnique(candidates, candidate);
  }

  const systemRoot = envValue(env, ["SystemRoot", "SYSTEMROOT", "windir", "WINDIR"]) || "C:\\Windows";
  pushUnique(candidates, path.win32.join(systemRoot, "Sysnative", "WindowsPowerShell", "v1.0", "powershell.exe"));
  pushUnique(candidates, path.win32.join(systemRoot, "System32", "WindowsPowerShell", "v1.0", "powershell.exe"));

  return candidates.find((candidate) => path.win32.isAbsolute(candidate) && pathExists(candidate)) || null;
}

function registerPowerShellCommandPath(powerShell, env, spawnSyncFn) {
  const installDir = path.win32.dirname(powerShell);
  prependPathEntries(env, [installDir]);
  persistWindowsPathEntries(powerShell, [installDir], env, spawnSyncFn);
  return powerShell;
}

function persistWindowsPathEntries(powerShell, entries, env, spawnSyncFn) {
  if (!entries.length) {
    return;
  }
  const script = String.raw`
$ErrorActionPreference = "Stop"
$entriesToAdd = ConvertFrom-Json -InputObject $env:TURA_POWERSHELL_PATH_ENTRIES
function Normalize-PathEntry {
  param([string]$PathEntry)
  if ([string]::IsNullOrWhiteSpace($PathEntry)) { return "" }
  try {
    return (Resolve-Path -LiteralPath $PathEntry).ProviderPath.TrimEnd('\').ToLowerInvariant()
  } catch {
    return ([System.IO.Path]::GetFullPath($PathEntry)).TrimEnd('\').ToLowerInvariant()
  }
}
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$pathEntries = @()
if (-not [string]::IsNullOrWhiteSpace($userPath)) {
  $pathEntries = @($userPath -split [IO.Path]::PathSeparator | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
}
$known = @{}
foreach ($entry in $pathEntries) { $known[(Normalize-PathEntry $entry)] = $true }
foreach ($entry in $entriesToAdd) {
  if ([string]::IsNullOrWhiteSpace($entry)) { continue }
  $normalized = Normalize-PathEntry $entry
  if (-not $known.ContainsKey($normalized)) {
    $pathEntries = @($entry) + $pathEntries
    $known[$normalized] = $true
  }
}
[Environment]::SetEnvironmentVariable("Path", ($pathEntries -join [IO.Path]::PathSeparator), "User")
`;
  const result = spawnSyncFn(
    powerShell,
    ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script],
    {
      env: { ...process.env, ...env, TURA_POWERSHELL_PATH_ENTRIES: JSON.stringify(entries) },
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    },
  );
  if (result.error) {
    fail(result.error.message);
  }
  if ((result.status ?? 1) !== 0) {
    fail((result.stderr || result.stdout || "PowerShell PATH update failed").trim());
  }
}

function installPowerShell7WithWinget({ env, spawnSyncFn }) {
  const result = spawnSyncFn(
    "winget",
    [
      "install",
      "--id",
      "Microsoft.PowerShell",
      "--exact",
      "--source",
      "winget",
      "--accept-package-agreements",
      "--accept-source-agreements",
      "--disable-interactivity",
    ],
    {
      env,
      stdio: "inherit",
      shell: true,
      windowsHide: false,
    },
  );
  if (result.error) {
    fail(result.error.message);
  }
  if ((result.status ?? 1) !== 0) {
    fail(`winget install Microsoft.PowerShell failed with exit ${result.status ?? result.signal}`);
  }
}

export function ensureWindowsPowerShellCommand({
  env = process.env,
  pathExists = existsSync,
  spawnSyncFn = spawnSync,
  platform = process.platform,
  quiet = false,
} = {}) {
  const resolved = resolveWindowsPowerShellCommand({ env, pathExists });
  if (platform !== "win32") {
    return resolved;
  }

  for (const name of ["pwsh", "powershell"]) {
    const whereHit = whereWindowsCommand(name, { env, spawnSyncFn });
    if (whereHit && pathExists(whereHit)) {
      prependPathEntries(env, [path.win32.dirname(whereHit)]);
      return whereHit;
    }
  }

  if (resolved) {
    return registerPowerShellCommandPath(resolved, env, spawnSyncFn);
  }

  if (env.TURA_NPM_SKIP_POWERSHELL_INSTALL === "1" || env.TURA_NPM_SKIP_POWERSHELL_INSTALL === "true") {
    return null;
  }

  if (!quiet) {
    say("PowerShell 7 (pwsh) was not found on PATH; installing Microsoft.PowerShell with winget.", quiet);
  }
  installPowerShell7WithWinget({ env, spawnSyncFn });

  const installed = powerShell7InstallCandidates(env).find((candidate) => pathExists(candidate));
  if (!installed) {
    fail("Microsoft.PowerShell installed, but pwsh.exe was not found under Program Files.");
  }
  return registerPowerShellCommandPath(installed, env, spawnSyncFn);
}

function runPowerShell(script, env) {
  const mergedEnv = { ...process.env, ...env };
  const powerShell = ensureWindowsPowerShellCommand({ env: mergedEnv, quiet: true });
  if (!powerShell) {
    fail("PowerShell was not found. Restore Windows PowerShell to PATH, set TURA_POWERSHELL_PATH, or install PowerShell 7.");
  }
  const result = spawnSync(
    powerShell,
    ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script],
    {
      env: mergedEnv,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    },
  );
  if (result.error) {
    fail(result.error.message);
  }
  if ((result.status ?? 1) !== 0) {
    fail((result.stderr || result.stdout || "PowerShell CLI path update failed").trim());
  }
  return result.stdout.trim();
}

const windowsPathFunctions = String.raw`
$ErrorActionPreference = "Stop"
function Normalize-PathEntry {
  param([string]$PathEntry)
  if ([string]::IsNullOrWhiteSpace($PathEntry)) { return "" }
  try {
    return (Resolve-Path -LiteralPath $PathEntry).ProviderPath.TrimEnd('\').ToLowerInvariant()
  } catch {
    return ([System.IO.Path]::GetFullPath($PathEntry)).TrimEnd('\').ToLowerInvariant()
  }
}
function Same-PathEntry {
  param([string]$Left, [string]$Right)
  return (Normalize-PathEntry $Left) -eq (Normalize-PathEntry $Right)
}
function Filter-PathEntries {
  param([string]$Value, [string]$ReleaseDir, [string]$StaleCliBin)
  if ([string]::IsNullOrWhiteSpace($Value)) { return @() }
  return @($Value -split [IO.Path]::PathSeparator | Where-Object {
    -not [string]::IsNullOrWhiteSpace($_) -and
    -not (Same-PathEntry $_ $ReleaseDir) -and
    -not (Same-PathEntry $_ $StaleCliBin)
  })
}
`;

function registerWindows(releaseDir, packageRoot, { quiet = false } = {}) {
  const script = `${windowsPathFunctions}
$releaseDir = [System.IO.Path]::GetFullPath($env:TURA_CLI_RELEASE_DIR)
$staleCliBin = [System.IO.Path]::GetFullPath($env:TURA_CLI_STALE_BIN)
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$entries = @(Filter-PathEntries $userPath $releaseDir $staleCliBin)
$entries += $releaseDir
[Environment]::SetEnvironmentVariable("Path", ($entries -join [IO.Path]::PathSeparator), "User")
$env:Path = ((Filter-PathEntries $env:Path $releaseDir $staleCliBin) + $releaseDir) -join [IO.Path]::PathSeparator
Write-Output $releaseDir
`;
  const registered = runPowerShell(script, {
    TURA_CLI_RELEASE_DIR: path.resolve(releaseDir),
    TURA_CLI_STALE_BIN: path.join(packageRoot, "cli-bin"),
  });
  say(`Registered Tura CLI path: ${registered}`, quiet);
  return true;
}

function unregisterWindows(releaseDir, packageRoot, { quiet = false } = {}) {
  const script = `${windowsPathFunctions}
$releaseDir = [System.IO.Path]::GetFullPath($env:TURA_CLI_RELEASE_DIR)
$staleCliBin = [System.IO.Path]::GetFullPath($env:TURA_CLI_STALE_BIN)
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$entries = @(Filter-PathEntries $userPath $releaseDir $staleCliBin)
[Environment]::SetEnvironmentVariable("Path", ($entries -join [IO.Path]::PathSeparator), "User")
$env:Path = (Filter-PathEntries $env:Path $releaseDir $staleCliBin) -join [IO.Path]::PathSeparator
if (Test-Path -LiteralPath $staleCliBin) {
  Remove-Item -LiteralPath $staleCliBin -Recurse -Force
}
Write-Output $releaseDir
`;
  const removed = runPowerShell(script, {
    TURA_CLI_RELEASE_DIR: path.resolve(releaseDir),
    TURA_CLI_STALE_BIN: path.join(packageRoot, "cli-bin"),
  });
  say(`Removed Tura CLI path: ${removed}`, quiet);
  return true;
}

function pathRegisteredWindows(releaseDir) {
  const script = `${windowsPathFunctions}
$releaseDir = [System.IO.Path]::GetFullPath($env:TURA_CLI_RELEASE_DIR)
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$found = $false
if (-not [string]::IsNullOrWhiteSpace($userPath)) {
  foreach ($entry in ($userPath -split [IO.Path]::PathSeparator)) {
    if (-not [string]::IsNullOrWhiteSpace($entry) -and (Same-PathEntry $entry $releaseDir)) {
      $found = $true
      break
    }
  }
}
if ($found) { Write-Output "true" } else { Write-Output "false" }
`;
  return runPowerShell(script, {
    TURA_CLI_RELEASE_DIR: path.resolve(releaseDir),
    TURA_CLI_STALE_BIN: "",
  }) === "true";
}

export function registerCliPath({
  packageRoot = packageRootFromThisFile(),
  releaseDir = defaultReleaseDir(packageRoot),
  quiet = false,
  requireBinaries = true,
} = {}) {
  const resolvedReleaseDir = path.resolve(releaseDir);
  if (requireBinaries) {
    assertReleaseBinaries(resolvedReleaseDir);
  }
  if (process.platform === "win32") {
    return registerWindows(resolvedReleaseDir, packageRoot, { quiet });
  }
  return registerPosix(resolvedReleaseDir, { quiet });
}

export function unregisterCliPath({
  packageRoot = packageRootFromThisFile(),
  releaseDir = defaultReleaseDir(packageRoot),
  quiet = false,
} = {}) {
  const resolvedReleaseDir = path.resolve(releaseDir);
  if (process.platform === "win32") {
    return unregisterWindows(resolvedReleaseDir, packageRoot, { quiet });
  }
  return unregisterPosix(resolvedReleaseDir, packageRoot, { quiet });
}

export function isCliPathRegistered({
  packageRoot = packageRootFromThisFile(),
  releaseDir = defaultReleaseDir(packageRoot),
} = {}) {
  const resolvedReleaseDir = path.resolve(releaseDir);
  if (process.platform === "win32") {
    return pathRegisteredWindows(resolvedReleaseDir);
  }
  return pathRegisteredPosix(resolvedReleaseDir);
}

function parseArgs(args) {
  const options = {
    action: args[0],
    packageRoot: packageRootFromThisFile(),
    releaseDir: null,
    quiet: false,
  };
  for (let index = 1; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--quiet") {
      options.quiet = true;
    } else if (arg === "--release-dir") {
      index += 1;
      options.releaseDir = args[index] ? path.resolve(args[index]) : null;
    } else if (arg === "--package-root") {
      index += 1;
      options.packageRoot = args[index] ? path.resolve(args[index]) : options.packageRoot;
    } else {
      fail(`Unknown option: ${arg}`);
    }
  }
  options.releaseDir = options.releaseDir || defaultReleaseDir(options.packageRoot);
  return options;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.action === "register") {
    registerCliPath(options);
  } else if (options.action === "unregister") {
    unregisterCliPath(options);
  } else if (options.action === "check") {
    if (!isCliPathRegistered(options)) {
      process.exit(1);
    }
  } else {
    fail("Usage: node scripts/npm/cli-path.mjs register|unregister|check [--release-dir DIR] [--package-root DIR] [--quiet]");
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[tura cli-path] ${error.message}`);
    process.exit(1);
  });
}
