#!/usr/bin/env node
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const releaseDir = path.join(repoRoot, "release");
const platformOutDir = path.join(releaseDir, "npm-platform");
const mainOutDir = path.join(releaseDir, "npm-main");

function fail(message) {
  console.error(`[tura verify-platform-install] ${message}`);
  process.exit(1);
}

function npmCommand() {
  return process.platform === "win32" ? "npm.cmd" : "npm";
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? repoRoot,
    env: options.env ?? process.env,
    shell: process.platform === "win32",
    stdio: options.capture ? ["ignore", "pipe", "inherit"] : "inherit",
    windowsHide: false
  });
  if (result.error) {
    fail(result.error.message);
  }
  if ((result.status ?? 1) !== 0) {
    fail(`${command} ${args.join(" ")} failed with exit ${result.status}`);
  }
  return options.capture ? result.stdout.toString("utf8") : "";
}

function safeTempDir(name) {
  const root = path.resolve(tmpdir());
  const target = path.resolve(root, name);
  if (!target.startsWith(root + path.sep)) {
    fail(`refusing to use temp path outside temp root: ${target}`);
  }
  rmSync(target, { recursive: true, force: true });
  mkdirSync(target, { recursive: true });
  return target;
}

function firstTarball(root) {
  if (!existsSync(root)) return null;
  const tarballs = readdirSync(root)
    .filter((entry) => entry.endsWith(".tgz"))
    .sort();
  return tarballs.length > 0 ? path.join(root, tarballs[tarballs.length - 1]) : null;
}

function firstPlatformPackageDir(root) {
  const nodeModules = path.join(root, "node_modules");
  if (!existsSync(nodeModules)) return null;
  const packageNames = readdirSync(nodeModules)
    .filter((entry) => entry.startsWith("tura-"))
    .sort();
  return packageNames.length > 0 ? path.join(nodeModules, packageNames[0]) : null;
}

function packageMain() {
  mkdirSync(mainOutDir, { recursive: true });
  const output = run(npmCommand(), ["pack", "--json", "--pack-destination", mainOutDir], {
    capture: true
  });
  let parsed;
  try {
    parsed = JSON.parse(output);
  } catch {
    fail(`npm pack did not return JSON: ${output.slice(0, 500)}`);
  }
  const filename = parsed?.[0]?.filename;
  if (!filename) {
    fail("npm pack output did not include a tarball filename.");
  }
  return path.join(mainOutDir, filename);
}

const packageJson = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8"));
const platformPackage = firstTarball(platformOutDir);
if (!platformPackage) {
  fail("platform package tarball was not produced.");
}
const mainPackage = packageMain();
if (!existsSync(mainPackage)) {
  fail(`main package tarball was not produced: ${mainPackage}`);
}

const suffix = packageJson.version.replaceAll(/[^a-zA-Z0-9_.-]/g, "-");
const platformInstallDir = safeTempDir(`tura-platform-package-check-${suffix}`);
const installDir = safeTempDir(`tura-npm-install-check-${suffix}`);
const registrationHome = path.join(installDir, "home");
mkdirSync(registrationHome, { recursive: true });

run(npmCommand(), ["init", "-y", "--silent"], { cwd: platformInstallDir });
run(npmCommand(), ["install", "--omit=optional", platformPackage], {
  cwd: platformInstallDir
});
const platformPackageDir = firstPlatformPackageDir(platformInstallDir);
if (!platformPackageDir) {
  fail("installed platform package directory was not found.");
}

run(npmCommand(), ["init", "-y", "--silent"], { cwd: installDir });
const installEnv = {
  ...process.env,
  TURA_NPM_PLATFORM_PACKAGE_DIR: platformPackageDir
};
if (process.platform !== "win32") {
  installEnv.HOME = registrationHome;
}
run(npmCommand(), ["install", "--foreground-scripts", "--omit=optional", mainPackage], {
  cwd: installDir,
  env: installEnv
});

const mainPackageDir = path.join(installDir, "node_modules", packageJson.name);
const installedReleaseDir = path.join(mainPackageDir, "target", "release");
const executableExtension = process.platform === "win32" ? ".exe" : "";
const binName = process.platform === "win32" ? "tura.cmd" : "tura";
const requiredPaths = [
  path.join(installedReleaseDir, `tura${executableExtension}`),
  path.join(installedReleaseDir, `tura_exec${executableExtension}`),
  path.join(installedReleaseDir, "config", "provider_config.json"),
  path.join(installedReleaseDir, "tura_gui", "index.html"),
  path.join(installedReleaseDir, "commands", "web_discover", "command.toml"),
  path.join(installedReleaseDir, "README.md"),
  path.join(installedReleaseDir, "scripts", "ARCHITECTURE.md"),
  path.join(installedReleaseDir, "scripts", "register-cli.ps1"),
  path.join(installedReleaseDir, "scripts", "register-cli.sh"),
  path.join(installedReleaseDir, "scripts", "unregister-cli.ps1"),
  path.join(installedReleaseDir, "scripts", "unregister-cli.sh"),
  path.join(installDir, "node_modules", ".bin", binName)
];
const missing = requiredPaths.filter((requiredPath) => !existsSync(requiredPath));
if (missing.length > 0) {
  fail(`installed package is missing required release files:\n${missing.join("\n")}`);
}

function pathContains(value, entry) {
  const normalize = (item) => path.resolve(item.trim()).replaceAll("\\", "/").replace(/\/$/, "").toLowerCase();
  const expected = normalize(entry);
  return (value || "")
    .split(path.delimiter)
    .filter(Boolean)
    .some((item) => normalize(item) === expected);
}

function verifyCliRegistration() {
  if (process.platform === "win32") {
    const userPath = run(
      "powershell.exe",
      ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", "[Environment]::GetEnvironmentVariable('Path', 'User')"],
      { capture: true }
    );
    if (!pathContains(userPath, installedReleaseDir)) {
      fail(`CLI registration did not add release directory to the user PATH: ${installedReleaseDir}`);
    }
    return;
  }

  const profiles = [".profile", ".bash_profile", ".bashrc", ".zprofile", ".zshrc"]
    .map((name) => path.join(registrationHome, name))
    .filter((profile) => existsSync(profile));
  const registered = profiles.some((profile) => readFileSync(profile, "utf8").includes(installedReleaseDir));
  if (!registered) {
    fail(`CLI registration did not add release directory to a shell profile: ${installedReleaseDir}`);
  }
}

function cleanupCliRegistration() {
  const script = path.join(
    mainPackageDir,
    "scripts",
    process.platform === "win32" ? "unregister-cli.ps1" : "unregister-cli.sh"
  );
  if (!existsSync(script)) {
    return;
  }
  const args = process.platform === "win32"
    ? ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, "-Quiet"]
    : [script];
  const command = process.platform === "win32" ? "powershell.exe" : "sh";
  spawnSync(command, args, {
    cwd: mainPackageDir,
    env: process.platform === "win32" ? process.env : { ...process.env, HOME: registrationHome },
    shell: process.platform === "win32",
    stdio: "ignore",
    windowsHide: false
  });
}

verifyCliRegistration();
cleanupCliRegistration();
console.log(`[tura verify-platform-install] installed release files verified in ${installedReleaseDir}`);
