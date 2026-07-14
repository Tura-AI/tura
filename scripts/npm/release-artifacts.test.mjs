#!/usr/bin/env node
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import {
  desktopBundleAssets,
  executableName,
  executableNames,
  guiDistCandidates,
  missingReleaseFiles,
  mismatchedDesktopBundleAssets
} from "./release-artifacts.mjs";

function writeFixtureFile(root, relativePath) {
  const file = path.join(root, relativePath);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, "fixture\n");
}

test("desktop bundle discovery accepts installable assets for every release platform", () => {
  const fixtures = [
    ["win32", "bundle/nsis/tura_gui-setup.exe"],
    ["darwin", "bundle/dmg/tura_gui.dmg"],
    ["linux", "bundle/appimage/tura_gui.AppImage"]
  ];
  for (const [platform, asset] of fixtures) {
    const root = mkdtempSync(path.join(tmpdir(), `tura-desktop-assets-${platform}-`));
    try {
      writeFixtureFile(path.join(root, "target", "release"), asset);
      assert.deepEqual(
        desktopBundleAssets(root, platform).map((file) => path.relative(root, file).replaceAll("\\", "/")),
        [`target/release/${asset}`]
      );
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }
});

test("release validation rejects a desktop bundle directory without an installer", () => {
  const root = mkdtempSync(path.join(tmpdir(), "tura-desktop-required-"));
  try {
    const releaseDir = path.join(root, "target", "release");
    for (const name of executableNames) {
      writeFixtureFile(releaseDir, executableName(name, "win32"));
    }
    writeFixtureFile(releaseDir, executableName("tura_gui", "win32"));
    writeFixtureFile(releaseDir, "config/provider_config.json");
    writeFixtureFile(releaseDir, "tura_gui_dist/index.html");
    mkdirSync(path.join(releaseDir, "bundle"), { recursive: true });

    const missingInstaller = missingReleaseFiles(root, "win32");
    assert.equal(missingInstaller.length, 1);
    assert.match(missingInstaller[0], /installer\.msi\|\.exe$/u);

    writeFixtureFile(releaseDir, "bundle/msi/tura_gui.msi");
    assert.deepEqual(missingReleaseFiles(root, "win32"), []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("GUI dist discovery never treats the Unix desktop executable as a directory", () => {
  const root = mkdtempSync(path.join(tmpdir(), "tura-gui-dist-"));
  try {
    const releaseDir = path.join(root, "target", "release");
    writeFixtureFile(releaseDir, "tura_gui");
    writeFixtureFile(releaseDir, "tura_gui_dist/index.html");

    assert.deepEqual(guiDistCandidates(root), [path.join(releaseDir, "tura_gui_dist")]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("desktop bundle validation detects stale installer versions", () => {
  const root = mkdtempSync(path.join(tmpdir(), "tura-desktop-version-"));
  try {
    writeFixtureFile(path.join(root, "target", "release"), "bundle/msi/tura_gui_0.1.31_x64_en-US.msi");
    writeFixtureFile(path.join(root, "target", "release"), "bundle/nsis/tura_gui_0.1.32_x64-setup.exe");
    writeFixtureFile(path.join(root, "target", "release"), "bundle/nsis/tura_gui_0.1.320_x64-setup.exe");
    assert.deepEqual(
      mismatchedDesktopBundleAssets(root, "0.1.32", "win32").map((file) => path.basename(file)),
      ["tura_gui_0.1.31_x64_en-US.msi", "tura_gui_0.1.320_x64-setup.exe"]
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
