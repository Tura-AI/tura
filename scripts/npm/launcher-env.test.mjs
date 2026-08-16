#!/usr/bin/env node
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import {
  launchEnvironment,
  missingRuntimeResources,
  requiredRuntimeResources,
} from "./launcher-env.mjs";

test("npm launcher loads packaged runtime resources without relocating persistent state", () => {
  const packageRoot = path.resolve("C:/npm/tura-ai");
  const releaseDir = path.join(packageRoot, "node_modules", "tura-win32-x64", "target", "release");
  const releaseBin = path.join(releaseDir, "tura.exe");
  const providerConfig = path.join(releaseDir, "config", "provider_config.json");
  const env = launchEnvironment({
    baseEnv: { PATH: "fixture-path" },
    providerConfig,
    releaseBin,
    releaseDir,
  });

  assert.equal(env.TURA_PROJECT_ROOT, releaseDir);
  assert.equal(env.TURA_RELEASE_BIN_DIR, releaseDir);
  assert.equal(env.TURA_PROVIDER_CONFIG, providerConfig);
  assert.equal(env.PATH, "fixture-path");
  assert.equal(Object.hasOwn(env, "TURA_HOME"), false);
});

test("npm launcher preserves explicit runtime overrides", () => {
  const env = launchEnvironment({
    baseEnv: {
      TURA_HOME: "C:/custom/home",
      TURA_PROJECT_ROOT: "C:/custom/root",
      TURA_PROVIDER_CONFIG: "C:/custom/provider.json",
      TURA_RELEASE_BIN_DIR: "C:/custom/bin",
    },
    providerConfig: "C:/release/config/provider_config.json",
    releaseBin: "C:/release/tura.exe",
    releaseDir: "C:/release",
  });

  assert.equal(env.TURA_HOME, "C:/custom/home");
  assert.equal(env.TURA_PROJECT_ROOT, "C:/custom/root");
  assert.equal(env.TURA_PROVIDER_CONFIG, "C:/custom/provider.json");
  assert.equal(env.TURA_RELEASE_BIN_DIR, "C:/custom/bin");
});

test("runtime resource validation reports every missing packaged resource", () => {
  const releaseDir = mkdtempSync(path.join(tmpdir(), "tura-launcher-resources-"));
  try {
    for (const relativePath of requiredRuntimeResources) {
      const target = path.join(releaseDir, relativePath);
      if (path.extname(target)) {
        mkdirSync(path.dirname(target), { recursive: true });
        writeFileSync(target, "fixture\n");
      } else {
        mkdirSync(target, { recursive: true });
      }
    }
    assert.deepEqual(missingRuntimeResources(releaseDir), []);

    const missing = path.join(releaseDir, "crates", "tools", "src", "command_run", "schema.json");
    rmSync(missing);
    assert.deepEqual(missingRuntimeResources(releaseDir), [missing]);
  } finally {
    rmSync(releaseDir, { recursive: true, force: true });
  }
});
