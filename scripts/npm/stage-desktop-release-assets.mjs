#!/usr/bin/env node
import { copyFileSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  desktopBundleAssets,
  mismatchedDesktopBundleAssets,
  platformTriple,
  releaseOutputRoot,
  releaseTag
} from "./release-artifacts.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const packageJson = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8"));
const args = process.argv.slice(2);
const outIndex = args.indexOf("--out-dir");
const outDir = outIndex >= 0
  ? path.resolve(args[outIndex + 1] ?? "")
  : path.join(releaseOutputRoot(repoRoot), "desktop");

function fail(message) {
  console.error(`[tura stage-desktop-release-assets] ${message}`);
  process.exit(1);
}

const sources = desktopBundleAssets(repoRoot);
if (sources.length === 0) {
  fail("Tauri did not produce an installable desktop bundle.");
}
const staleSources = mismatchedDesktopBundleAssets(repoRoot, packageJson.version);
if (staleSources.length > 0) {
  fail(`Tauri bundle version does not match ${packageJson.version}: ${staleSources.map((source) => path.basename(source)).join(", ")}`);
}

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
const usedNames = new Set();
for (const source of sources) {
  const extension = path.extname(source);
  const qualifier = extension.toLowerCase() === ".exe" ? "-setup" : "";
  const assetName = `tura-gui-${releaseTag(packageJson.version)}-${platformTriple()}${qualifier}${extension}`;
  if (usedNames.has(assetName)) {
    fail(`multiple Tauri bundles map to the GitHub Release asset ${assetName}`);
  }
  usedNames.add(assetName);
  const destination = path.join(outDir, assetName);
  copyFileSync(source, destination);
  console.log(destination);
}
