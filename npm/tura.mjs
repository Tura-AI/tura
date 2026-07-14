#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { platformPackageName } from "../scripts/npm/release-artifacts.mjs";
import {
  defaultReleaseDir,
  isCliPathRegistered,
  registerCliPath,
  unregisterCliPath,
} from "../scripts/npm/cli-path.mjs";

const packageRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const executable = process.platform === "win32" ? "tura.exe" : "tura";
const require = createRequire(import.meta.url);

function firstExistingPath(candidates) {
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

function platformReleaseDir() {
  try {
    const platformRoot = path.dirname(
      require.resolve(`${platformPackageName()}/package.json`, {
        paths: [packageRoot],
      }),
    );
    return path.join(platformRoot, "target", "release");
  } catch {
    return null;
  }
}

function installedReleaseDir() {
  const localReleaseDir = defaultReleaseDir(packageRoot);
  if (existsSync(path.join(localReleaseDir, executable))) {
    return localReleaseDir;
  }
  const platformDir = platformReleaseDir();
  if (platformDir && existsSync(path.join(platformDir, executable))) {
    return platformDir;
  }
  return localReleaseDir;
}

const args = process.argv.slice(2);
const releaseDir = installedReleaseDir();

if (args[0] === "register-cli") {
  registerCliPath({ packageRoot, releaseDir, quiet: false });
  process.exit(0);
}

if (args[0] === "unregister-cli") {
  unregisterCliPath({ packageRoot, releaseDir, quiet: false });
  process.exit(0);
}

if (args[0] === "doctor-cli-path") {
  if (isCliPathRegistered({ packageRoot, releaseDir })) {
    console.log(`Tura CLI path is registered: ${releaseDir}`);
    process.exit(0);
  }
  console.error(`Tura CLI path is not registered: ${releaseDir}`);
  process.exit(1);
}

const releaseBin = path.join(releaseDir, executable);

if (!releaseBin || !existsSync(releaseBin)) {
  console.error("Tura release binary was not found.");
  console.error(
    `Install ${platformPackageName()}, run npm install again, or set TURA_NPM_RELEASE_ARCHIVE to a platform release archive and reinstall.`,
  );
  process.exit(1);
}

const providerConfig = firstExistingPath([
  path.join(packageRoot, "crates", "provider", "config", "provider_config.json"),
  path.join(releaseDir, "config", "provider_config.json"),
]);

const result = spawnSync(releaseBin, process.argv.slice(2), {
  env: {
    ...process.env,
    TURA_RELEASE_BIN_DIR:
      process.env.TURA_RELEASE_BIN_DIR || path.dirname(releaseBin),
    TURA_PROJECT_ROOT: process.env.TURA_PROJECT_ROOT || packageRoot,
    ...(providerConfig
      ? { TURA_PROVIDER_CONFIG: process.env.TURA_PROVIDER_CONFIG || providerConfig }
      : {}),
  },
  stdio: "inherit",
  windowsHide: false,
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
