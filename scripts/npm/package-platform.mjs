#!/usr/bin/env node
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  bundleCandidates,
  executableName,
  executableNames,
  firstExistingPath,
  guiDistCandidates,
  missingPackageFiles,
  platformPackageName,
  releaseArchiveName,
  releaseConfigFiles,
  releaseOutputRoot,
  releaseRoot
} from "./release-artifacts.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const rootPackage = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8"));
const args = process.argv.slice(2);
const outIndex = args.indexOf("--out-dir");
const outDir = outIndex >= 0 ? path.resolve(args[outIndex + 1] ?? "") : path.join(releaseOutputRoot(repoRoot), "npm-platform");
const pack = args.includes("--pack");
const platformName = platformPackageName();
const packageDir = path.join(outDir, platformName);
const sourceRelease = releaseRoot(repoRoot);
const stageRelease = path.join(packageDir, "target", "release");

function fail(message) {
  console.error(`[tura package-platform] ${message}`);
  process.exit(1);
}

function run(command, commandArgs, cwd = packageDir) {
  const result = spawnSync(command, commandArgs, {
    cwd,
    shell: process.platform === "win32",
    stdio: "inherit",
    windowsHide: false
  });
  if (result.error) {
    fail(result.error.message);
  }
  if ((result.status ?? 1) !== 0) {
    fail(`${command} ${commandArgs.join(" ")} failed with exit ${result.status}`);
  }
}

function npmCommand() {
  return process.platform === "win32" ? "npm.cmd" : "npm";
}

function copyDirectory(source, destination, label) {
  if (!source || !existsSync(source)) {
    fail(`missing release ${label}`);
  }
  rmSync(destination, { recursive: true, force: true });
  cpSync(source, destination, { recursive: true });
}

const missingPackage = missingPackageFiles(repoRoot);
if (missingPackage.length > 0) {
  fail(`missing required package files: ${missingPackage.join(", ")}`);
}

rmSync(packageDir, { recursive: true, force: true });
mkdirSync(stageRelease, { recursive: true });

for (const name of executableNames) {
  const fileName = executableName(name);
  const source = path.join(sourceRelease, fileName);
  if (!existsSync(source)) {
    fail(`missing release binary: ${path.relative(repoRoot, source)}`);
  }
  cpSync(source, path.join(stageRelease, fileName));
}

const desktopGuiName = executableName("tura_gui");
const desktopGui = path.join(sourceRelease, desktopGuiName);
if (!existsSync(desktopGui)) {
  fail(`missing desktop GUI binary: ${path.relative(repoRoot, desktopGui)}`);
}
cpSync(desktopGui, path.join(stageRelease, desktopGuiName));

copyDirectory(firstExistingPath(guiDistCandidates(repoRoot)), path.join(stageRelease, "tura_gui"), "GUI dist");
copyDirectory(firstExistingPath(bundleCandidates(repoRoot)), path.join(stageRelease, "bundle"), "Tauri bundle");

for (const [sourceRelative, releaseRelative] of releaseConfigFiles) {
  const source = path.join(repoRoot, sourceRelative);
  const destination = path.join(stageRelease, releaseRelative);
  if (!existsSync(source)) {
    fail(`missing release config source: ${sourceRelative}`);
  }
  mkdirSync(path.dirname(destination), { recursive: true });
  cpSync(source, destination);
}

writeFileSync(
  path.join(packageDir, "package.json"),
  `${JSON.stringify(
    {
      name: platformName,
      version: rootPackage.version,
      description: `Platform release binaries for ${rootPackage.name} on ${process.platform}-${process.arch}.`,
      type: "module",
      license: rootPackage.license,
      repository: rootPackage.repository,
      bugs: rootPackage.bugs,
      homepage: rootPackage.homepage,
      os: [process.platform],
      cpu: [process.arch],
      files: ["target/release/**"],
      publishConfig: rootPackage.publishConfig
    },
    null,
    2
  )}\n`
);
cpSync(path.join(repoRoot, "LICENSE"), path.join(packageDir, "LICENSE"));
writeFileSync(
  path.join(packageDir, "README.md"),
  `# ${platformName}\n\nNative release artifacts for ${rootPackage.name} ${rootPackage.version}. Install ${rootPackage.name}; do not install this package directly.\n\nRelease archive equivalent: ${releaseArchiveName(rootPackage.version)}\n`
);

console.log(packageDir);

if (pack) {
  run(npmCommand(), ["pack", "--json", "--pack-destination", outDir]);
}
