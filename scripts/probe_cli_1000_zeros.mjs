import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { performance } from "node:perf_hooks";

const repoRoot = process.cwd();
const runRoot = path.join(repoRoot, ".tmp-cli-zero-probe", String(Date.now()));
const prompt =
  "Output exactly 1000 characters, all zeros, no spaces, no punctuation, no markdown, no newline.";

const turaBin = path.join(repoRoot, "target", "debug", process.platform === "win32" ? "tura.exe" : "tura");
const codexBin = path.join(
  "C:",
  "Users",
  "liuliu",
  "Documents",
  "codex-main",
  "codex-rs",
  "target",
  "debug",
  process.platform === "win32" ? "codex.exe" : "codex",
);

function configValue(value) {
  return value;
}

function spawnLogged(name, command, args, options = {}) {
  return new Promise((resolve) => {
    const started = performance.now();
    let firstOutputMs = null;
    let firstAgentMessageMs = null;
    let stdout = "";
    let stderr = "";
    let pendingStdout = "";
    const child = spawn(command, args, {
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
    });
    const mark = () => {
      if (firstOutputMs === null) firstOutputMs = performance.now() - started;
    };
    child.stdout.on("data", (chunk) => {
      mark();
      const text = chunk.toString();
      stdout += text;
      pendingStdout += text;
      const lines = pendingStdout.split(/\r?\n/);
      pendingStdout = lines.pop() || "";
      for (const line of lines) {
        try {
          const event = JSON.parse(line);
          if (
            firstAgentMessageMs === null &&
            (event.type === "item.completed" || event.item?.type === "agent_message")
          ) {
            firstAgentMessageMs = performance.now() - started;
          }
        } catch {
          // ignore non-json stdout
        }
      }
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
    });
    child.on("close", (code) => {
      resolve({
        name,
        code,
        duration_ms: Math.round(performance.now() - started),
        first_output_ms: firstOutputMs === null ? null : Math.round(firstOutputMs),
        first_agent_message_ms:
          firstAgentMessageMs === null ? null : Math.round(firstAgentMessageMs),
        stdout,
        stderr,
      });
    });
    child.stdin.end(prompt);
  });
}

function extractText(stdout) {
  const lines = stdout.split(/\r?\n/).filter(Boolean);
  let text = "";
  for (const line of lines) {
    try {
      const event = JSON.parse(line);
      const payload = event.msg || event;
      if (typeof payload.message === "string") text += payload.message;
      if (typeof payload.text === "string") text += payload.text;
      if (typeof payload.item?.text === "string") text += payload.item.text;
      if (payload.type === "agent_message" && typeof payload.message === "string") {
        text += payload.message;
      }
      if (payload.type === "message" && typeof payload.content === "string") {
        text += payload.content;
      }
    } catch {
      text += line;
    }
  }
  return text;
}

function extractUsage(stdout) {
  const usage = [];
  for (const line of stdout.split(/\r?\n/)) {
    if (!line.trim()) continue;
    try {
      const event = JSON.parse(line);
      const raw = JSON.stringify(event);
      const matches = raw.match(/"usage"\s*:\s*\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\}/g) || [];
      for (const match of matches) {
        try {
          usage.push(JSON.parse(`{${match}}`).usage);
        } catch {
          // ignore partial regex captures
        }
      }
    } catch {
      // ignore non-json stdout
    }
  }
  return usage;
}

async function writeRun(run) {
  const dir = path.join(runRoot, run.name);
  await fs.mkdir(dir, { recursive: true });
  await fs.writeFile(path.join(dir, "stdout.jsonl"), run.stdout);
  await fs.writeFile(path.join(dir, "stderr.log"), run.stderr);
}

function summarize(run) {
  const text = extractText(run.stdout);
  const zeros = (text.match(/0/g) || []).length;
  const usage = extractUsage(run.stdout);
  const outputTokens = usage.reduce((sum, item) => sum + Number(item.output_tokens || item.completion_tokens || 0), 0);
  const outputTps =
    outputTokens > 0 && run.duration_ms > 0 ? +(outputTokens / (run.duration_ms / 1000)).toFixed(2) : null;
  return {
    name: run.name,
    exit_code: run.code,
    duration_ms: run.duration_ms,
    first_output_ms: run.first_output_ms,
    first_agent_message_ms: run.first_agent_message_ms,
    text_chars: text.length,
    zeros,
    usage_events: usage.length,
    output_tokens: outputTokens || null,
    output_tps_total_wall: outputTps,
    stdout_path: path.join(runRoot, run.name, "stdout.jsonl"),
    stderr_path: path.join(runRoot, run.name, "stderr.log"),
  };
}

await fs.mkdir(runRoot, { recursive: true });

if (!existsSync(turaBin)) throw new Error(`missing tura binary: ${turaBin}`);
if (!existsSync(codexBin)) throw new Error(`missing codex binary: ${codexBin}`);

const commonCodexArgs = [
  "exec",
  "--skip-git-repo-check",
  "--json",
  "-C",
  repoRoot,
  "-m",
  "gpt-5.5",
  "--dangerously-bypass-approvals-and-sandbox",
  "-c",
  configValue('model_reasoning_effort="low"'),
];
const commonTuraArgs = [
  "exec",
  "--skip-git-repo-check",
  "--json",
  "-C",
  repoRoot,
  "-m",
  "openai/gpt-5.5",
  "--dangerously-bypass-approvals-and-sandbox",
  "-c",
  configValue('model_reasoning_effort="low"'),
];

const runs = await Promise.all([
  spawnLogged("tura-priority", turaBin, [...commonTuraArgs, "-c", 'service_tier="priority"']),
  spawnLogged("tura-default", turaBin, [...commonTuraArgs, "-c", 'service_tier="auto"']),
  spawnLogged("codex-main-priority", codexBin, [...commonCodexArgs, "-c", 'service_tier="priority"']),
  spawnLogged("codex-main-default", codexBin, [...commonCodexArgs, "-c", 'service_tier="default"']),
]);

for (const run of runs) await writeRun(run);

const summary = runs.map(summarize);
await fs.writeFile(path.join(runRoot, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`);
console.table(summary);
console.log(`run_root=${runRoot}`);
