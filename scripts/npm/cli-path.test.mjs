#!/usr/bin/env node
import assert from "node:assert/strict";
import test from "node:test";
import path from "node:path";
import {
  ensureWindowsPowerShellCommand,
  resolveWindowsPowerShellCommand,
} from "./cli-path.mjs";

test("resolveWindowsPowerShellCommand falls back to the Windows PowerShell system path", () => {
  const systemPowerShell = path.win32.join("C:\\Windows", "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
  const actual = resolveWindowsPowerShellCommand({
    env: {
      Path: "C:\\Program Files\\nodejs",
      SystemRoot: "C:\\Windows",
    },
    pathExists: (candidate) => candidate === systemPowerShell,
  });

  assert.equal(actual, systemPowerShell);
});

test("resolveWindowsPowerShellCommand honors explicit TURA_POWERSHELL_PATH first", () => {
  const explicit = path.win32.join("D:\\Tools", "pwsh.exe");
  const actual = resolveWindowsPowerShellCommand({
    env: {
      TURA_POWERSHELL_PATH: explicit,
      Path: "C:\\Windows\\System32\\WindowsPowerShell\\v1.0",
      SystemRoot: "C:\\Windows",
    },
    pathExists: () => true,
  });

  assert.equal(actual, explicit);
});

test("resolveWindowsPowerShellCommand reads Windows PATH entries on any host platform", () => {
  const expected = path.win32.join("C:\\Windows", "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
  const actual = resolveWindowsPowerShellCommand({
    env: {
      Path: "C:\\Program Files\\nodejs;C:\\Windows\\System32\\WindowsPowerShell\\v1.0",
      SystemRoot: "C:\\MissingWindows",
    },
    pathExists: (candidate) => candidate === expected,
  });

  assert.equal(actual, expected);
});

test("resolveWindowsPowerShellCommand returns null when no candidate exists", () => {
  const actual = resolveWindowsPowerShellCommand({
    env: {
      Path: "C:\\Program Files\\nodejs",
      SystemRoot: "C:\\Windows",
    },
    pathExists: () => false,
  });

  assert.equal(actual, null);
});

test("ensureWindowsPowerShellCommand prefers where.exe hits before install", () => {
  const pwsh = path.win32.join("C:\\Program Files", "PowerShell", "7", "pwsh.exe");
  const calls = [];
  const actual = ensureWindowsPowerShellCommand({
    platform: "win32",
    env: {
      Path: "C:\\Program Files\\nodejs",
      SystemRoot: "C:\\Windows",
    },
    pathExists: (candidate) => candidate === pwsh,
    spawnSyncFn: (command, args) => {
      calls.push([command, ...args]);
      if (command === "where.exe" && args[0] === "pwsh.exe") {
        return { status: 0, stdout: `${pwsh}\r\n`, stderr: "" };
      }
      return { status: 1, stdout: "", stderr: "" };
    },
  });

  assert.equal(actual, pwsh);
  assert.deepEqual(calls, [["where.exe", "pwsh.exe"]]);
});

test("ensureWindowsPowerShellCommand registers standard PowerShell path when where.exe misses", () => {
  const systemPowerShell = path.win32.join("C:\\Windows", "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
  const env = {
    Path: "C:\\Program Files\\nodejs",
    SystemRoot: "C:\\Windows",
  };
  const calls = [];
  const actual = ensureWindowsPowerShellCommand({
    platform: "win32",
    env,
    pathExists: (candidate) => candidate === systemPowerShell,
    spawnSyncFn: (command, args) => {
      calls.push([command, ...args]);
      if (command === systemPowerShell) {
        return { status: 0, stdout: "", stderr: "" };
      }
      return { status: 1, stdout: "", stderr: "" };
    },
  });

  assert.equal(actual, systemPowerShell);
  assert.equal(env.Path.startsWith(path.win32.dirname(systemPowerShell)), true);
  assert.equal(calls.some((call) => call[0] === "winget"), false);
  assert.equal(calls.some((call) => call[0] === systemPowerShell), true);
});

test("ensureWindowsPowerShellCommand installs and registers PowerShell 7 when no candidate exists", () => {
  const installed = path.win32.join("C:\\Program Files", "PowerShell", "7", "pwsh.exe");
  const env = {
    Path: "C:\\Program Files\\nodejs",
    ProgramFiles: "C:\\Program Files",
    SystemRoot: "C:\\Windows",
  };
  const calls = [];
  let wingetInstalled = false;
  const actual = ensureWindowsPowerShellCommand({
    platform: "win32",
    env,
    quiet: true,
    pathExists: (candidate) => wingetInstalled && candidate === installed,
    spawnSyncFn: (command, args) => {
      calls.push([command, ...args]);
      if (command === "winget") {
        wingetInstalled = true;
        return { status: 0, stdout: "", stderr: "" };
      }
      if (command === installed) {
        return { status: 0, stdout: "", stderr: "" };
      }
      return { status: 1, stdout: "", stderr: "" };
    },
  });

  assert.equal(actual, installed);
  assert.equal(env.Path.startsWith(path.win32.dirname(installed)), true);
  assert.equal(calls.some((call) => call[0] === "winget"), true);
  assert.equal(calls.some((call) => call[0] === installed), true);
});
