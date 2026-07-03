#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { platformPackageName } from "../scripts/npm/release-artifacts.mjs";

const packageRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const executable = process.platform === "win32" ? "tura.exe" : "tura";
const require = createRequire(import.meta.url);
const providerConfig = path.join(
  packageRoot,
  "crates",
  "provider",
  "config",
  "provider_config.json",
);

function platformReleaseBin() {
  try {
    const platformRoot = path.dirname(
      require.resolve(`${platformPackageName()}/package.json`, {
        paths: [packageRoot],
      }),
    );
    return path.join(platformRoot, "target", "release", executable);
  } catch {
    return null;
  }
}

const localReleaseBin = path.join(packageRoot, "target", "release", executable);
const releaseBin = existsSync(localReleaseBin)
  ? localReleaseBin
  : platformReleaseBin();

if (!releaseBin || !existsSync(releaseBin)) {
  console.error("Tura release binary was not found.");
  console.error(
    `Install ${platformPackageName()}, run npm install again, or set TURA_NPM_RELEASE_ARCHIVE to a platform release archive and reinstall.`,
  );
  process.exit(1);
}

const result = spawnSync(releaseBin, process.argv.slice(2), {
  env: {
    ...process.env,
    TURA_RELEASE_BIN_DIR:
      process.env.TURA_RELEASE_BIN_DIR || path.dirname(releaseBin),
    TURA_PROJECT_ROOT: process.env.TURA_PROJECT_ROOT || packageRoot,
    TURA_PROVIDER_CONFIG: process.env.TURA_PROVIDER_CONFIG || providerConfig,
  },
  stdio: "inherit",
  windowsHide: false,
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
