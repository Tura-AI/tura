#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..", "..");

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
  schema.input_schema.properties.commands.items.properties.step.description.includes("task_status compact_context"),
  "command_run schema must tell the model task_status compact_context belongs after summarized work",
);

const promptPath = path.join(repoRoot, "crates", "tools", "src", "commands", "task_status", "prompt.md");
const prompt = fs.readFileSync(promptPath, "utf8");
assert(prompt.includes("250,000 tokens"), "task_status compact_context prompt must include the 250k trigger");
assert(prompt.includes("new task no longer depends on the current main context"), "task_status compact_context prompt must include the new-task dependency rule");
assert(prompt.includes("Do not duplicate obvious dialogue history"), "task_status compact_context prompt must avoid duplicating conversation history");

const taskStatusSchemaPath = path.join(repoRoot, "crates", "tools", "src", "commands", "task_status", "schema.json");
const taskStatusSchema = JSON.parse(fs.readFileSync(taskStatusSchemaPath, "utf8"));
assert(taskStatusSchema.properties.compact_context, "task_status schema must expose compact_context");

run("cargo", ["test", "-p", "tools", "compact_context", "--", "--nocapture"]);
assert(
  !fs.existsSync(path.join(repoRoot, "crates", "tools", "src", "commands", "compact_context")),
  "standalone compact_context command directory must be removed",
);
run("cargo", ["test", "-p", "tools", "task_status", "--", "--nocapture"]);
run("cargo", [
  "test",
  "-p",
  "runtime",
  "compact_session_context_replaces_prior_tool_context",
  "--",
  "--nocapture",
]);
run("cargo", [
  "test",
  "-p",
  "runtime",
  "compact_context_required_formats_dynamic_limit_and_current_turn_instruction",
  "--",
  "--nocapture",
]);

console.log(JSON.stringify({
  ok: true,
  coverage: [
    "task_status schema and prompt expose compact_context handoff guidance",
    "context checkpoint hides prior tool history, preserves later command_run backfill, and reinjects workspace snapshot",
    "context-threshold prompt requires task_status compact_context",
  ],
}, null, 2));
