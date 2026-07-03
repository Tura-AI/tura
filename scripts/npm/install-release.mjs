#!/usr/bin/env node
import { chmodSync, cpSync, createWriteStream, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { get } from "node:https";
import { tmpdir } from "node:os";
import path from "node:path";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  executableName,
  executableNames,
  missingPackageFiles,
  missingReleaseFiles,
  platformPackageName,
  releaseArchiveName,
  releaseOutputRoot,
  releaseTag
} from "./release-artifacts.mjs";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
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

function run(command, args, cwd = packageRoot) {
  const result = spawnSync(command, args, { cwd, stdio: "inherit", windowsHide: false });
  if (result.error) {
    fail(result.error.message);
  }
  if ((result.status ?? 1) !== 0) {
    fail(`${command} ${args.join(" ")} failed with exit ${result.status}`);
  }
}

function releaseUrl() {
  if (process.env.TURA_NPM_RELEASE_URL) {
    return process.env.TURA_NPM_RELEASE_URL;
  }
  const baseUrl = (process.env.TURA_NPM_RELEASE_BASE_URL || "https://github.com/Tura-AI/tura/releases/download").replace(/\/$/, "");
  const tag = releaseTag(packageJson.version);
  return `${baseUrl}/${tag}/${releaseArchiveName(packageJson.version)}`;
}

function download(url, destination, redirects = 0) {
  return new Promise((resolve, reject) => {
    get(url, (response) => {
      if ([301, 302, 303, 307, 308].includes(response.statusCode ?? 0)) {
        response.resume();
        if (!response.headers.location || redirects > 5) {
          reject(new Error(`download redirect failed for ${url}`));
          return;
        }
        download(new URL(response.headers.location, url).toString(), destination, redirects + 1).then(resolve, reject);
        return;
      }
      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`download failed with HTTP ${response.statusCode}: ${url}`));
        return;
      }
      const file = createWriteStream(destination);
      response.pipe(file);
      file.on("finish", () => file.close(resolve));
      file.on("error", reject);
    }).on("error", reject);
  });
}

function extractArchive(archivePath) {
  mkdirSync(releaseDir, { recursive: true });
  if (archivePath.endsWith(".zip")) {
    if (process.platform === "win32") {
      run("powershell.exe", [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        `Expand-Archive -LiteralPath '${archivePath.replaceAll("'", "''")}' -DestinationPath '${packageRoot.replaceAll("'", "''")}' -Force`
      ]);
      return;
    }
    run("unzip", ["-oq", archivePath, "-d", packageRoot]);
    return;
  }
  if (archivePath.endsWith(".tar.gz") || archivePath.endsWith(".tgz")) {
    run("tar", ["-xzf", archivePath, "-C", packageRoot]);
    return;
  }
  fail(`unsupported release archive type: ${archivePath}`);
}

function verifyInstall() {
  const missingPackage = missingPackageFiles(packageRoot);
  if (missingPackage.length > 0) {
    fail(`package is missing required config/assets: ${missingPackage.join(", ")}`);
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
  const missingRelease = missingReleaseFiles(packageRoot);
  if (missingRelease.length > 0) {
    fail(`release archive did not install required files: ${missingRelease.map((file) => path.relative(packageRoot, file)).join(", ")}`);
  }
}

function resolvePlatformPackageRoot() {
  if (process.env.TURA_NPM_PLATFORM_PACKAGE_DIR) {
    return path.resolve(process.env.TURA_NPM_PLATFORM_PACKAGE_DIR);
  }

  const packageName = platformPackageName();
  try {
    return path.dirname(require.resolve(`${packageName}/package.json`, { paths: [packageRoot] }));
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
    fail(`platform package ${platformPackageName()} is incomplete: ${missingPlatform.map((file) => path.relative(platformRoot, file)).join(", ")}`);
  }

  cpSync(path.join(platformRoot, "target", "release"), releaseDir, { recursive: true });
  markReleaseExecutablesRunnable();
  verifyInstall();
  log(`release binaries installed from ${platformPackageName()}`);
  return true;
}

function localArchivePath() {
  if (process.env.TURA_NPM_RELEASE_ARCHIVE) {
    return path.resolve(process.env.TURA_NPM_RELEASE_ARCHIVE);
  }
  const candidate = path.join(releaseOutputRoot(packageRoot), releaseArchiveName(packageJson.version));
  return existsSync(candidate) ? candidate : null;
}

if (process.env.TURA_NPM_SKIP_RELEASE_DOWNLOAD === "1" || process.env.TURA_NPM_SKIP_RELEASE_DOWNLOAD === "true") {
  log("release download skipped by TURA_NPM_SKIP_RELEASE_DOWNLOAD");
  process.exit(0);
}

const existingMissing = missingReleaseFiles(packageRoot);
if (existingMissing.length === 0) {
  verifyInstall();
  log("release binaries already present");
  process.exit(0);
}

if (installFromPlatformPackage()) {
  process.exit(0);
}

const localArchive = localArchivePath();
if (localArchive) {
  extractArchive(localArchive);
  markReleaseExecutablesRunnable();
  verifyInstall();
  log(`release binaries installed from ${path.relative(packageRoot, localArchive)}`);
  process.exit(0);
}

const tempRoot = mkdtempSync(path.join(tmpdir(), "tura-npm-release-"));
try {
  const archivePath = path.join(tempRoot, releaseArchiveName(packageJson.version));
  const url = releaseUrl();
  log(`downloading ${url}`);
  await download(url, archivePath);
  extractArchive(archivePath);
  markReleaseExecutablesRunnable();
  verifyInstall();
  log("release binaries installed");
} finally {
  rmSync(tempRoot, { recursive: true, force: true });
}
