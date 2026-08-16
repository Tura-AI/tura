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
  requiredReleaseRuntimeFiles,
  platformPackageName,
} from "./release-artifacts.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const packageJson = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8"));
const installModeArg = process.argv.find((arg) => arg.startsWith("--install-mode="));
const installMode = installModeArg?.split("=", 2)[1] ?? process.env.TURA_NPM_INSTALL_MODE ?? "local";
if (!new Set(["global", "local"]).has(installMode)) {
  console.error(`[tura npm install fixture] unsupported install mode: ${installMode}`);
  process.exit(1);
}
const fixtureRoot = path.join(
  tmpdir(),
  `tura-npm-install-fixture-${process.platform}-${process.arch}-${installMode}-${packageJson.version}`,
);
const packageOutDir = path.join(fixtureRoot, "packages");
const platformRoot = path.join(fixtureRoot, "platform-package");
const platformReleaseDir = path.join(platformRoot, "target", "release");
const installDir = path.join(fixtureRoot, "main-install");
const globalPrefix = path.join(fixtureRoot, "npm-prefix");

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
    const packEntries = Array.isArray(parsed) ? parsed : Object.values(parsed ?? {});
    const filename = packEntries[0]?.filename;
    if (filename) return filename;
  } catch {
    // handled below
  }
  fail(`npm pack did not return a package filename: ${output.slice(0, 500)}`);
}

function writeFixtureExecutable(file) {
  mkdirSync(path.dirname(file), { recursive: true });
  if (process.platform === "win32") {
    if (path.basename(file).toLowerCase() === "tura.exe") {
      cpSync(process.execPath, file);
      return;
    }
    writeFileSync(file, "@echo off\r\nexit /b 0\r\n");
    return;
  }
  writeFileSync(file, "#!/bin/sh\nprintf '%s\\n' 'Tura simulated release help'\nexit 0\n");
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
  for (const runtimeResource of [
    "crates/tools/src/command_run/schema.json",
    "crates/tools/src/commands/apply_patch/command.toml",
    "commands/web_discover/command.toml",
  ]) {
    writeFixtureFile(runtimeResource, readFileSync(path.join(repoRoot, runtimeResource), "utf8"));
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

function globalNodeModules() {
  return process.platform === "win32"
    ? path.join(globalPrefix, "node_modules")
    : path.join(globalPrefix, "lib", "node_modules");
}

function verifyInstalledPackage(platformPackage, mainPackage) {
  const env = { ...process.env };
  let nodeModules;
  let binPath;
  if (installMode === "global") {
    mkdirSync(globalPrefix, { recursive: true });
    env.npm_config_prefix = globalPrefix;
    run(npmCommand(), ["install", "--global", "--omit=optional", platformPackage, mainPackage], { env });
    nodeModules = globalNodeModules();
    binPath = process.platform === "win32"
      ? path.join(globalPrefix, "tura.cmd")
      : path.join(globalPrefix, "bin", "tura");
  } else {
    run(npmCommand(), ["init", "-y", "--silent"], { cwd: installDir, env });
    run(npmCommand(), ["install", "--omit=optional", platformPackage, mainPackage], { cwd: installDir, env });
    nodeModules = path.join(installDir, "node_modules");
    binPath = path.join(nodeModules, ".bin", process.platform === "win32" ? "tura.cmd" : "tura");
  }

  const mainPackageDir = path.join(nodeModules, packageJson.name);
  const platformPackageDir = path.join(nodeModules, platformPackageName());
  if (!existsSync(mainPackageDir)) {
    fail(`main package was not installed: ${mainPackageDir}`);
  }
  const fixtureTura = path.join(platformReleaseDir, executableName("tura"));
  const installedTura = path.join(platformPackageDir, "target", "release", executableName("tura"));
  if (!existsSync(installedTura)) {
    fail(`platform package was not installed with its tura executable: ${platformPackageDir}`);
  }
  if (!readFileSync(installedTura).equals(readFileSync(fixtureTura))) {
    fail("installed tura executable does not match the simulated platform release.");
  }
  if (!existsSync(binPath)) {
    fail(`npm bin shim was not installed: ${binPath}`);
  }
  if (existsSync(path.join(mainPackageDir, "target", "release"))) {
    fail("main package unexpectedly contains copied release files; no postinstall copy should occur.");
  }

  const installedManifest = JSON.parse(readFileSync(path.join(mainPackageDir, "package.json"), "utf8"));
  if (installedManifest.scripts?.postinstall) {
    fail("installed main package unexpectedly defines postinstall.");
  }

  run(binPath, ["--help"], { cwd: installDir, env });
  run(npmCommand(), ["list", ...(installMode === "global" ? ["--global"] : []), packageJson.name, platformPackageName(), "--depth=0"], {
    cwd: installDir,
    env,
  });
}

rmSync(fixtureRoot, { recursive: true, force: true });
mkdirSync(packageOutDir, { recursive: true });
mkdirSync(installDir, { recursive: true });

try {
  createFixturePlatformPackage();
  const platformPackage = packPlatformPackage();
  const mainPackage = packMainPackage();
  verifyInstalledPackage(platformPackage, mainPackage);
  console.log(`[tura npm install fixture] ${process.platform}-${process.arch} ${installMode} simulated release verified without postinstall`);
} finally {
  rmSync(fixtureRoot, { recursive: true, force: true });
}
