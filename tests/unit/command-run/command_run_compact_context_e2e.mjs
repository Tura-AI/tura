#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..");

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: "pipe",
    shell: process.platform === "win32",
  });
  if (result.status !== 0) {
    process.stdout.write(result.stdout || "");
    process.stderr.write(result.stderr || "");
    throw new Error(`${command} ${args.join(" ")} failed with status ${result.status}`);
  }
  return result.stdout;
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

const schemaPath = path.join(repoRoot, "crates", "tools", "src", "command_run", "schema.json");
const schema = JSON.parse(fs.readFileSync(schemaPath, "utf8"));
assert(
  schema.input_schema.properties.commands.items.properties.step.description.includes("compact_context"),
  "command_run schema must tell the model compact_context belongs in the final highest step",
);

const promptPath = path.join(repoRoot, "crates", "tools", "src", "commands", "compact_context", "prompt.md");
const prompt = fs.readFileSync(promptPath, "utf8");
assert(prompt.includes("200,000 tokens"), "compact_context prompt must include the 200k trigger");
assert(prompt.includes("final command"), "compact_context prompt must require final-step placement");
assert(prompt.includes("15,000 English words"), "compact_context prompt must include the 20k-token output cap as words");

run("cargo", ["test", "-p", "code-tools", "compact_context", "--", "--nocapture"]);
run("cargo", [
  "test",
  "-p",
  "code-tools-suite",
  "compact_session_context_replaces_prior_tool_context",
  "--",
  "--nocapture",
]);
run("cargo", [
  "test",
  "-p",
  "code-tools-suite",
  "messages_for_turn_injects_compact_context_prompt_at_default_220k_threshold",
  "--",
  "--nocapture",
]);

console.log(JSON.stringify({
  ok: true,
  coverage: [
    "compact_context command routes through command_run and enforces final highest step",
    "context checkpoint hides prior tool history, preserves later command_run backfill, and reinjects workspace snapshot",
    "220k-token threshold injects a user continuation requiring compact_context",
  ],
}, null, 2));
