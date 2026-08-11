#!/usr/bin/env node
import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { platformPackageName } from "./release-artifacts.mjs";
import {
  ensureRuntimeDependencies,
  runtimeDependencyCheckSkipped,
  runtimeDependencyCommands,
} from "./install-release.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

test("postinstall probes only runtime commands for each supported platform", () => {
  const observed = [];
  ensureRuntimeDependencies({
    platform: "linux",
    env: {},
    exists: (command) => {
      observed.push(command);
      return true;
    },
  });
  assert.deepEqual(observed, runtimeDependencyCommands("linux"));
  assert.deepEqual(observed, ["sh", "tar", "bash"]);
  assert.equal(observed.some((command) => ["cargo", "rustc", "bun", "uv"].includes(command)), false);
});

test("postinstall skip flag bypasses runtime command probes", () => {
  let probes = 0;
  ensureRuntimeDependencies({
    platform: "linux",
    env: { TURA_NPM_SKIP_RUNTIME_DEPENDENCY_CHECK: "true" },
    exists: () => {
      probes += 1;
      return false;
    },
  });
  assert.equal(probes, 0);
  assert.equal(runtimeDependencyCheckSkipped({ TURA_NPM_SKIP_RUNTIME_DEPENDENCY_CHECK: "1" }), true);
});

test("Windows postinstall delegates to the PowerShell runtime check", () => {
  let powershellChecks = 0;
  ensureRuntimeDependencies({
    platform: "win32",
    env: {},
    exists: () => {
      throw new Error("Windows must not use POSIX command probes");
    },
    ensurePowerShell: () => {
      powershellChecks += 1;
    },
  });
  assert.equal(powershellChecks, 1);
});

test("postinstall fails directly when the platform package is unavailable", () => {
  const fixtureRoot = mkdtempSync(path.join(tmpdir(), "tura-postinstall-missing-platform-"));
  const fixtureScripts = path.join(fixtureRoot, "scripts", "npm");
  const lureUrl = "https://127.0.0.1:9/should-not-be-downloaded.zip";

  try {
    mkdirSync(fixtureScripts, { recursive: true });
    for (const file of ["cli-path.mjs", "install-release.mjs", "release-artifacts.mjs"]) {
      copyFileSync(path.join(repoRoot, "scripts", "npm", file), path.join(fixtureScripts, file));
    }
    const platformPackageDir = path.join(fixtureRoot, "node_modules", platformPackageName());
    mkdirSync(platformPackageDir, { recursive: true });
    writeFileSync(path.join(platformPackageDir, "package.json"), "not valid JSON\n");
    writeFileSync(
      path.join(fixtureRoot, "package.json"),
      `${JSON.stringify({ name: "tura-ai", version: "0.0.0-test", type: "module" })}\n`,
    );

    const env = {
      ...process.env,
      TURA_NPM_RELEASE_URL: lureUrl,
      TURA_NPM_SKIP_CLI_REGISTRATION: "1",
      TURA_NPM_SKIP_RUNTIME_DEPENDENCY_CHECK: "1",
    };
    delete env.TURA_NPM_PLATFORM_PACKAGE_DIR;
    delete env.TURA_NPM_RELEASE_ARCHIVE;
    delete env.TURA_NPM_SKIP_RELEASE_DOWNLOAD;

    const result = spawnSync(process.execPath, [path.join(fixtureScripts, "install-release.mjs")], {
      cwd: fixtureRoot,
      env,
      encoding: "utf8",
      windowsHide: true,
    });
    const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;

    assert.equal(result.status, 1, output);
    assert.match(output, new RegExp(`platform package ${platformPackageName()} is unavailable`, "u"));
    assert.doesNotMatch(output, /downloading/iu);
    assert.doesNotMatch(output, new RegExp(lureUrl.replaceAll(/[.*+?^${}()|[\]\\]/g, "\\$&"), "u"));
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});
