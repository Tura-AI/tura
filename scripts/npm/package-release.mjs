#!/usr/bin/env node
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { resolveWindowsPowerShellCommand } from "./cli-path.mjs";
import {
  bundleCandidates,
  executableName,
  executableNames,
  firstExistingPath,
  guiDistCandidates,
  missingPackageFiles,
  missingReleaseRuntimeFiles,
  releaseArchiveName,
  releaseConfigFiles,
  releaseRuntimeExcludedDirs,
  releaseRuntimeFiles,
  releaseOutputRoot,
  releaseRoot
} from "./release-artifacts.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const packageJson = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8"));
const args = process.argv.slice(2);
const outIndex = args.indexOf("--out-dir");
const outDir = outIndex >= 0 ? path.resolve(args[outIndex + 1] ?? "") : releaseOutputRoot(repoRoot);
const binaryOnly = args.includes("--binary");
const includeDesktop = !args.includes("--no-desktop");
const archiveName = releaseArchiveName(packageJson.version);
const archivePath = path.join(outDir, archiveName);

function fail(message) {
  console.error(`[tura package-release] ${message}`);
  process.exit(1);
}

function run(command, commandArgs, cwd = repoRoot) {
  const result = spawnSync(command, commandArgs, { cwd, stdio: "inherit", windowsHide: false });
  if (result.error) {
    fail(result.error.message);
  }
  if ((result.status ?? 1) !== 0) {
    fail(`${command} ${commandArgs.join(" ")} failed with exit ${result.status}`);
  }
}

function copyDirectory(source, destination, label) {
  if (!source || !existsSync(source)) {
    fail(`missing release ${label}`);
  }
  rmSync(destination, { recursive: true, force: true });
  cpSync(source, destination, { recursive: true });
}

function removeExcludedRuntimeDirs(root) {
  if (!existsSync(root) || !statSync(root).isDirectory()) return;
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const fullPath = path.join(root, entry.name);
    if (!entry.isDirectory()) continue;
    if (releaseRuntimeExcludedDirs.includes(entry.name)) {
      rmSync(fullPath, { recursive: true, force: true });
      continue;
    }
    removeExcludedRuntimeDirs(fullPath);
  }
}

function copyRuntimePath(source, destination, label) {
  if (!existsSync(source)) {
    fail(`missing release runtime ${label}`);
  }
  rmSync(destination, { recursive: true, force: true });
  mkdirSync(path.dirname(destination), { recursive: true });
  cpSync(source, destination, { recursive: true });
  removeExcludedRuntimeDirs(destination);
}

const missingPackage = missingPackageFiles(repoRoot);
if (missingPackage.length > 0) {
  fail(`missing required package files: ${missingPackage.join(", ")}`);
}

const sourceRelease = releaseRoot(repoRoot);
const stageRoot = mkdtempSync(path.join(tmpdir(), "tura-release-stage-"));
try {
  const stageRelease = path.join(stageRoot, "target", "release");
  mkdirSync(stageRelease, { recursive: true });

  for (const name of executableNames) {
    const fileName = executableName(name);
    const source = path.join(sourceRelease, fileName);
    if (!existsSync(source)) {
      fail(`missing release binary: ${path.relative(repoRoot, source)}`);
    }
    cpSync(source, path.join(stageRelease, fileName));
  }

  if (includeDesktop) {
    const desktopGuiName = executableName("tura_gui");
    const desktopGui = path.join(sourceRelease, desktopGuiName);
    if (!existsSync(desktopGui)) {
      fail(`missing desktop GUI binary: ${path.relative(repoRoot, desktopGui)}`);
    }
    cpSync(desktopGui, path.join(stageRelease, desktopGuiName));
    copyDirectory(firstExistingPath(bundleCandidates(repoRoot)), path.join(stageRelease, "bundle"), "Tauri bundle");
  }

  copyDirectory(firstExistingPath(guiDistCandidates(repoRoot)), path.join(stageRelease, "tura_gui"), "GUI dist");

  for (const [sourceRelative, releaseRelative] of releaseConfigFiles) {
    const source = path.join(repoRoot, sourceRelative);
    const destination = path.join(stageRelease, releaseRelative);
    if (!existsSync(source)) {
      fail(`missing release config source: ${sourceRelative}`);
    }
    mkdirSync(path.dirname(destination), { recursive: true });
    cpSync(source, destination);
  }

  if (!binaryOnly) {
    for (const [releaseRelative] of releaseRuntimeFiles) {
      copyRuntimePath(
        path.join(sourceRelease, releaseRelative),
        path.join(stageRelease, releaseRelative),
        releaseRelative
      );
    }
    const missingRuntime = missingReleaseRuntimeFiles(stageRoot);
    if (missingRuntime.length > 0) {
      fail(`release archive is missing runtime config files: ${missingRuntime.map((file) => path.relative(stageRoot, file)).join(", ")}`);
    }
  }

  mkdirSync(outDir, { recursive: true });
  rmSync(archivePath, { force: true });
  if (process.platform === "win32") {
    const powerShell = resolveWindowsPowerShellCommand();
    if (!powerShell) {
      fail("PowerShell was not found. Restore Windows PowerShell to PATH, set TURA_POWERSHELL_PATH, or install PowerShell 7.");
    }
    run(powerShell, [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-Command",
      `Compress-Archive -Path '${path.join(stageRoot, "target").replaceAll("'", "''")}' -DestinationPath '${archivePath.replaceAll("'", "''")}' -Force`
    ]);
  } else {
    run("tar", ["-czf", archivePath, "target"], stageRoot);
  }

  console.log(archivePath);
} finally {
  rmSync(stageRoot, { recursive: true, force: true });
}
