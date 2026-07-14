#!/usr/bin/env node
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  executableName,
  executableNames,
  missingNpmPlatformFiles,
  requiredReleaseRuntimeFiles,
  platformPackageName,
} from "./release-artifacts.mjs";
import { isCliPathRegistered, unregisterCliPath } from "./cli-path.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const packageJson = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8"));
const fixtureRoot = path.join(
  tmpdir(),
  `tura-npm-install-fixture-${process.platform}-${process.arch}-${packageJson.version}`,
);
const packageOutDir = path.join(fixtureRoot, "packages");
const platformRoot = path.join(fixtureRoot, "platform-package");
const platformReleaseDir = path.join(platformRoot, "target", "release");
const platformInstallDir = path.join(fixtureRoot, "platform-install");
const installDir = path.join(fixtureRoot, "main-install");
const registrationHome = path.join(fixtureRoot, "home");

function fail(message) {
  console.error(`[tura npm install fixture] ${message}`);
  process.exit(1);
}

function npmCommand() {
  return process.platform === "win32" ? "npm.cmd" : "npm";
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? repoRoot,
    env: options.env ?? process.env,
    shell: process.platform === "win32",
    encoding: "utf8",
    stdio: options.capture ? ["ignore", "pipe", "pipe"] : "inherit",
    windowsHide: true,
  });
  if (result.error) {
    fail(result.error.message);
  }
  if ((result.status ?? 1) !== 0) {
    const detail = [result.stdout, result.stderr].filter(Boolean).join("\n").trim();
    fail(`${command} ${args.join(" ")} failed with exit ${result.status}${detail ? `\n${detail}` : ""}`);
  }
  return options.capture ? result.stdout : "";
}

function parsePackOutput(output) {
  try {
    const parsed = JSON.parse(output);
    const filename = parsed?.[0]?.filename;
    if (filename) return filename;
  } catch {
    // handled below
  }
  fail(`npm pack did not return a package filename: ${output.slice(0, 500)}`);
}

function writeFixtureExecutable(file) {
  mkdirSync(path.dirname(file), { recursive: true });
  if (process.platform === "win32") {
    writeFileSync(file, "@echo off\r\nexit /b 0\r\n");
    return;
  }
  writeFileSync(file, "#!/bin/sh\nexit 0\n");
  chmodSync(file, 0o755);
}

function writeFixtureFile(relativePath, content = "fixture\n") {
  const file = path.join(platformReleaseDir, relativePath);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, content);
}

function createFixturePlatformPackage() {
  mkdirSync(platformReleaseDir, { recursive: true });
  for (const name of executableNames) {
    writeFixtureExecutable(path.join(platformReleaseDir, executableName(name)));
  }
  mkdirSync(path.join(platformReleaseDir, "config"), { recursive: true });
  writeFileSync(path.join(platformReleaseDir, "config", "provider_config.json"), "{}\n");
  mkdirSync(path.join(platformReleaseDir, "tura_gui_dist"), { recursive: true });
  writeFileSync(path.join(platformReleaseDir, "tura_gui_dist", "index.html"), "<!doctype html><title>Tura fixture</title>\n");
  for (const file of requiredReleaseRuntimeFiles) {
    writeFixtureFile(file);
  }
  for (const configFile of requiredReleaseRuntimeFiles.filter((file) => file.endsWith(".json"))) {
    writeFixtureFile(configFile, "{}\n");
  }
  writeFileSync(
    path.join(platformRoot, "package.json"),
    `${JSON.stringify(
      {
        name: platformPackageName(),
        version: packageJson.version,
        description: "Tura npm install fixture platform package.",
        type: "module",
        license: packageJson.license,
        os: [process.platform],
        cpu: [process.arch],
        files: ["target/release/**"],
      },
      null,
      2,
    )}\n`,
  );
  cpSync(path.join(repoRoot, "LICENSE"), path.join(platformRoot, "LICENSE"));
  writeFileSync(path.join(platformRoot, "README.md"), "# Tura npm fixture\n");
}

function packPlatformPackage() {
  const output = run(npmCommand(), ["pack", platformRoot, "--json", "--pack-destination", packageOutDir], {
    capture: true,
  });
  return path.join(packageOutDir, parsePackOutput(output));
}

function packMainPackage() {
  const output = run(npmCommand(), ["pack", "--json", "--pack-destination", packageOutDir], {
    capture: true,
  });
  return path.join(packageOutDir, parsePackOutput(output));
}

function firstPlatformPackageDir(root) {
  const nodeModules = path.join(root, "node_modules");
  const packageDir = path.join(nodeModules, platformPackageName());
  return existsSync(packageDir) ? packageDir : null;
}

function installEnv(platformPackageDir) {
  const env = {
    ...process.env,
    TURA_NPM_PLATFORM_PACKAGE_DIR: platformPackageDir,
  };
  if (process.platform !== "win32") {
    env.HOME = registrationHome;
  } else {
    const nodeDir = path.dirname(process.execPath);
    env.Path = nodeDir;
    env.PATH = nodeDir;
  }
  return env;
}

function verifyInstalledPackage(platformPackageDir) {
  run(npmCommand(), ["init", "-y", "--silent"], { cwd: installDir });
  const env = installEnv(platformPackageDir);
  run(npmCommand(), ["install", "--foreground-scripts", "--omit=optional", path.join(packageOutDir, `tura-ai-${packageJson.version}.tgz`)], {
    cwd: installDir,
    env,
  });

  const mainPackageDir = path.join(installDir, "node_modules", packageJson.name);
  const installedReleaseDir = path.join(mainPackageDir, "target", "release");
  const missing = missingNpmPlatformFiles(mainPackageDir);
  if (missing.length > 0) {
    fail(`installed package is missing release files:\n${missing.join("\n")}`);
  }
  const binName = process.platform === "win32" ? "tura.cmd" : "tura";
  const binPath = path.join(installDir, "node_modules", ".bin", binName);
  if (!existsSync(binPath)) {
    fail(`npm bin shim was not installed: ${binPath}`);
  }

  if (process.platform !== "win32") {
    process.env.HOME = registrationHome;
  }
  if (!isCliPathRegistered({ packageRoot: mainPackageDir, releaseDir: installedReleaseDir })) {
    fail(`CLI registration did not add release directory: ${installedReleaseDir}`);
  }

  run(binPath, ["doctor-cli-path"], { cwd: installDir, env });
  run(binPath, ["unregister-cli"], { cwd: installDir, env });
  if (isCliPathRegistered({ packageRoot: mainPackageDir, releaseDir: installedReleaseDir })) {
    unregisterCliPath({ packageRoot: mainPackageDir, releaseDir: installedReleaseDir, quiet: true });
    fail(`CLI unregistration did not remove release directory: ${installedReleaseDir}`);
  }
}

rmSync(fixtureRoot, { recursive: true, force: true });
mkdirSync(packageOutDir, { recursive: true });
mkdirSync(platformInstallDir, { recursive: true });
mkdirSync(installDir, { recursive: true });
mkdirSync(registrationHome, { recursive: true });

try {
  createFixturePlatformPackage();
  const platformPackage = packPlatformPackage();
  packMainPackage();
  run(npmCommand(), ["init", "-y", "--silent"], { cwd: platformInstallDir });
  run(npmCommand(), ["install", "--omit=optional", platformPackage], { cwd: platformInstallDir });
  const platformPackageDir = firstPlatformPackageDir(platformInstallDir);
  if (!platformPackageDir) {
    fail("installed fixture platform package directory was not found");
  }
  verifyInstalledPackage(platformPackageDir);
  console.log(`[tura npm install fixture] ${process.platform}-${process.arch} install, binaries, and CLI registration verified`);
} finally {
  rmSync(fixtureRoot, { recursive: true, force: true });
}
