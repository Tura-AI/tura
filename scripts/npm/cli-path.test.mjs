#!/usr/bin/env node
import assert from "node:assert/strict";
import test from "node:test";
import path from "node:path";
import { resolveWindowsPowerShellCommand } from "./cli-path.mjs";

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
