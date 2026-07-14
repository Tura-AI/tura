#!/usr/bin/env node
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
);
const packageJsonPath = path.join(repoRoot, "package.json");
const backupPath = path.join(repoRoot, ".npm-package-json.backup");

function fail(message) {
  console.error(`[tura stage-main-package] ${message}`);
  process.exit(1);
}

if (existsSync(backupPath)) {
  fail(
    "package.json backup already exists; run node scripts/npm/restore-main-package.mjs before packing again.",
  );
}

const rootPackage = JSON.parse(readFileSync(packageJsonPath, "utf8"));
const packageName = process.env.TURA_NPM_PACKAGE_NAME || rootPackage.name;
const runtimePackage = {
  name: packageName,
  version: rootPackage.version,
  description: rootPackage.description,
  type: rootPackage.type,
  license: rootPackage.license,
  author: rootPackage.author,
  keywords: rootPackage.keywords,
  homepage: rootPackage.homepage,
  repository: rootPackage.repository,
  bugs: rootPackage.bugs,
  bin: rootPackage.bin,
  engines: rootPackage.engines,
  os: rootPackage.os,
  files: [
    "LICENSE",
    "README.md",
    "npm/tura.mjs",
    "scripts/npm/cli-path.mjs",
    "scripts/npm/install-release.mjs",
    "scripts/npm/release-artifacts.mjs",
  ],
  scripts: {
    postinstall: "node ./scripts/npm/install-release.mjs",
  },
  optionalDependencies: rootPackage.optionalDependencies,
  publishConfig: rootPackage.publishConfig,
};

writeFileSync(backupPath, readFileSync(packageJsonPath, "utf8"));
writeFileSync(packageJsonPath, `${JSON.stringify(runtimePackage, null, 2)}\n`);
console.error("[tura stage-main-package] staged slim npm package metadata.");
