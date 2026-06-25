import assert from "node:assert/strict";
import test from "node:test";
import { promptPayload } from "../../src/commands/run.js";
import { commandRunShellForCommand, parseRun } from "../../src/cli.js";

test("run shell flags override the command-run surface", () => {
  assert.equal(parseRun(["--bash", "inspect"], false).commandRunShell, "bash");
  assert.equal(parseRun(["--zsh", "inspect"], false).commandRunShell, "zsh");
  assert.equal(parseRun(["--shll", "inspect"], false).commandRunShell, "shell_command");
  assert.equal(parseRun(["-c", "command_run_shell=zsh", "inspect"], false).commandRunShell, "zsh");
  assert.throws(() => parseRun(["-c", "command_run_shell=zash", "inspect"], false), /bash/);
});

test("run defaults to high thinking with priority routing", () => {
  const parsed = parseRun(["hello"], false);

  assert.equal(parsed.agent, "thinking");
  assert.equal(parsed.modelVariant, "high");
  assert.equal(parsed.modelAccelerationEnabled, true);
});

test("top-level shell commands cover only the documented surfaces", () => {
  assert.equal(commandRunShellForCommand("bash"), "bash");
  assert.equal(commandRunShellForCommand("zsh"), "zsh");
  assert.equal(commandRunShellForCommand("shll"), "shell_command");
  assert.equal(commandRunShellForCommand("zash"), undefined);
});

test("prompt payload forwards the command-run shell override to the gateway", () => {
  const payload = promptPayload("inspect", { source: "cli", commandRunShell: "zsh" });

  assert.equal(payload.command_run_shell, "zsh");
});
