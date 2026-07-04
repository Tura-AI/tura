#!/usr/bin/env node
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";
import { isCliPathRegistered, unregisterCliPath } from "./cli-path.mjs";

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
  const tarball = path.join(mainOutDir, filename);
  verifyPackedMainPackage(tarball, parsed?.[0]);
  return tarball;
}

function readTarEntry(tarball, entryName) {
  const archive = gunzipSync(readFileSync(tarball));
  let offset = 0;
  while (offset + 512 <= archive.length) {
    const name = archive.toString("utf8", offset, offset + 100).replace(/\0.*$/u, "");
    if (!name) return null;
    const prefix = archive.toString("utf8", offset + 345, offset + 500).replace(/\0.*$/u, "");
    const fullName = prefix ? `${prefix}/${name}` : name;
    const sizeText = archive.toString("utf8", offset + 124, offset + 136).replace(/\0.*$/u, "").trim();
    const size = Number.parseInt(sizeText || "0", 8);
    const dataStart = offset + 512;
    if (fullName === entryName) {
      return archive.subarray(dataStart, dataStart + size).toString("utf8");
    }
    offset = dataStart + Math.ceil(size / 512) * 512;
  }
  return null;
}

function verifyPackedMainPackage(tarball, packInfo) {
  const expectedFiles = new Set([
    "LICENSE",
    "README.md",
    "npm/tura.mjs",
    "package.json",
    "scripts/npm/cli-path.mjs",
    "scripts/npm/install-release.mjs",
    "scripts/npm/release-artifacts.mjs"
  ]);
  const packedFiles = (packInfo?.files ?? []).map((file) => file.path.replaceAll("\\", "/")).sort();
  const missing = [...expectedFiles].filter((file) => !packedFiles.includes(file));
  const extra = packedFiles.filter((file) => !expectedFiles.has(file));
  if (missing.length > 0 || extra.length > 0) {
    fail(
      `main npm package contents are not slim runtime files.\nMissing:\n${missing.join("\n") || "(none)"}\nExtra:\n${extra.join("\n") || "(none)"}`
    );
  }

  const packedPackageJsonText = readTarEntry(tarball, "package/package.json");
  if (!packedPackageJsonText) {
    fail("packed main package did not contain package/package.json.");
  }
  const packedPackageJson = JSON.parse(packedPackageJsonText);
  const scripts = packedPackageJson.scripts ?? {};
  const scriptNames = Object.keys(scripts).sort();
  if (scriptNames.length !== 1 || scripts.postinstall !== "node ./scripts/npm/install-release.mjs") {
    fail(`packed main package contains unexpected npm scripts: ${scriptNames.join(", ") || "(none)"}`);
  }
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

function verifyCliRegistration() {
  if (!isCliPathRegistered({ packageRoot: mainPackageDir, releaseDir: installedReleaseDir })) {
    fail(`CLI registration did not add release directory to the user CLI path: ${installedReleaseDir}`);
  }
}

function cleanupCliRegistration() {
  unregisterCliPath({ packageRoot: mainPackageDir, releaseDir: installedReleaseDir, quiet: true });
}

function verifyCliUnregistration() {
  const binPath = path.join(installDir, "node_modules", ".bin", binName);
  run(binPath, ["unregister-cli"], {
    cwd: installDir,
    env: process.platform === "win32" ? process.env : { ...process.env, HOME: registrationHome }
  });
  if (isCliPathRegistered({ packageRoot: mainPackageDir, releaseDir: installedReleaseDir })) {
    cleanupCliRegistration();
    fail(`CLI unregistration did not remove release directory from the user CLI path: ${installedReleaseDir}`);
  }
}

verifyCliRegistration();
verifyCliUnregistration();
console.log(`[tura verify-platform-install] installed release files verified in ${installedReleaseDir}`);
