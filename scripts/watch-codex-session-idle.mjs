#!/usr/bin/env node

/**
 * Watch a Codex Desktop thread and continue it after its current turn becomes idle.
 *
 * The watcher uses `codex app-server` to resolve the Desktop thread/session. On
 * Windows, a separate app-server process reports Desktop-owned threads as
 * `notLoaded`; in that case the persisted rollout lifecycle is used as the
 * status source. Sending is always done through `codex exec resume`.
 */

import { spawn, spawnSync } from "node:child_process";
import { createReadStream, existsSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";

const DEFAULTS = Object.freeze({
  title: "发布 Reddit 软文 session-1",
  message: "同意请继续发送",
  intervalMs: 300_000,
  maxSends: 1,
  once: false,
  dryRun: false,
  sessionId: null,
  rolloutPath: null,
  codexPath: process.env.CODEX_CLI || null,
});

function usage() {
  return `Usage:
  node scripts/watch-codex-session-idle.mjs [options]

Options:
  --title <name>          Exact Codex Desktop task title
                          (default: ${DEFAULTS.title})
  --session-id <id>       Use a known session ID instead of title lookup
  --message <text>        Message sent when idle
                          (default: ${DEFAULTS.message})
  --interval-ms <number>  Poll interval in milliseconds (default: 300000 / 5 min)
  --max-sends <number>    Stop after N sends; 0 means keep watching (default: 1)
  --rollout <path>        Override the rollout JSONL path
  --codex <path>          Override the Codex CLI executable
  --once                  Check once and exit
  --dry-run               Report what would be sent without sending
  -h, --help              Show this help

Environment:
  CODEX_CLI               Codex CLI executable path (same as --codex)
`;
}

function requireValue(argv, index, option) {
  const value = argv[index + 1];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${option} requires a value`);
  }
  return value;
}

function parseNonNegativeInteger(value, option) {
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number < 0) {
    throw new Error(`${option} must be a non-negative integer`);
  }
  return number;
}

function parseArgs(argv) {
  const options = { ...DEFAULTS };

  for (let index = 0; index < argv.length; index += 1) {
    const option = argv[index];
    switch (option) {
      case "--title":
        options.title = requireValue(argv, index, option);
        index += 1;
        break;
      case "--session-id":
        options.sessionId = requireValue(argv, index, option);
        index += 1;
        break;
      case "--message":
        options.message = requireValue(argv, index, option);
        index += 1;
        break;
      case "--interval-ms":
        options.intervalMs = parseNonNegativeInteger(
          requireValue(argv, index, option),
          option,
        );
        if (options.intervalMs < 250) {
          throw new Error("--interval-ms must be at least 250");
        }
        index += 1;
        break;
      case "--max-sends":
        options.maxSends = parseNonNegativeInteger(
          requireValue(argv, index, option),
          option,
        );
        index += 1;
        break;
      case "--rollout":
        options.rolloutPath = requireValue(argv, index, option);
        index += 1;
        break;
      case "--codex":
        options.codexPath = requireValue(argv, index, option);
        index += 1;
        break;
      case "--once":
        options.once = true;
        break;
      case "--dry-run":
        options.dryRun = true;
        break;
      case "-h":
      case "--help":
        options.help = true;
        break;
      default:
        throw new Error(`Unknown option: ${option}`);
    }
  }

  if (!options.title && !options.sessionId) {
    throw new Error("Provide --title or --session-id");
  }
  if (!options.message) {
    throw new Error("--message cannot be empty");
  }

  return options;
}

function npmShimCommand(shimPath) {
  const cliScript = join(
    dirname(shimPath),
    "node_modules",
    "@openai",
    "codex",
    "bin",
    "codex.js",
  );
  if (!existsSync(cliScript)) return null;
  return { executable: process.execPath, prefixArgs: [cliScript] };
}

function resolveCodexCommand(explicitPath) {
  if (explicitPath) {
    if (!existsSync(explicitPath) && /[\\/]/u.test(explicitPath)) {
      throw new Error(`Codex executable not found: ${explicitPath}`);
    }
    if (process.platform === "win32" && explicitPath.toLowerCase().endsWith(".cmd")) {
      const command = npmShimCommand(explicitPath);
      if (!command) {
        throw new Error(`Cannot safely resolve the npm CLI behind ${explicitPath}`);
      }
      return command;
    }
    if (explicitPath.toLowerCase().endsWith(".js")) {
      return { executable: process.execPath, prefixArgs: [explicitPath] };
    }
    return { executable: explicitPath, prefixArgs: [] };
  }

  if (process.platform === "win32") {
    // The Desktop-bundled exe lives under WindowsApps and can reject direct
    // child-process launches with EPERM. Prefer the regular npm CLI shim and
    // invoke its JS entry point without a shell.
    const found = spawnSync("where.exe", ["codex.cmd"], {
      encoding: "utf8",
      windowsHide: true,
    });
    for (const candidate of found.stdout
      ?.split(/\r?\n/u)
      .map((line) => line.trim())
      .filter(Boolean) || []) {
      const command = npmShimCommand(candidate);
      if (command) return command;
    }
  }

  return { executable: "codex", prefixArgs: [] };
}

function timestamp() {
  return new Date().toISOString();
}

function log(message) {
  process.stdout.write(`[${timestamp()}] ${message}\n`);
}

function sleep(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

class AppServerClient {
  constructor(codexCommand) {
    this.codexCommand = codexCommand;
    this.nextId = 1;
    this.pending = new Map();
    this.stderr = "";
    this.process = null;
    this.lines = null;
  }

  async start() {
    this.process = spawn(this.codexCommand.executable, [
      ...this.codexCommand.prefixArgs,
      "app-server",
      "--listen",
      "stdio://",
    ], {
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
      shell: false,
    });

    this.process.stderr.setEncoding("utf8");
    this.process.stderr.on("data", (chunk) => {
      this.stderr = `${this.stderr}${chunk}`.slice(-8_000);
    });
    this.process.on("error", (error) => this.rejectAll(error));
    this.process.on("exit", (code, signal) => {
      if (this.pending.size > 0) {
        this.rejectAll(
          new Error(
            `codex app-server exited (${signal || code})${
              this.stderr.trim() ? `: ${this.stderr.trim()}` : ""
            }`,
          ),
        );
      }
    });

    this.lines = createInterface({ input: this.process.stdout });
    this.lines.on("line", (line) => this.handleLine(line));

    await this.request("initialize", {
      clientInfo: {
        name: "tura_codex_idle_watcher",
        title: "Tura Codex Idle Watcher",
        version: "1.0.0",
      },
      capabilities: { experimentalApi: true },
    });
    this.notify("initialized", {});
  }

  handleLine(line) {
    let message;
    try {
      message = JSON.parse(line);
    } catch {
      return;
    }

    if (message.id === undefined || message.id === null) return;
    const pending = this.pending.get(message.id);
    if (!pending) return;
    this.pending.delete(message.id);
    clearTimeout(pending.timeout);

    if (message.error) {
      pending.reject(
        new Error(
          `${pending.method} failed: ${
            message.error.message || JSON.stringify(message.error)
          }`,
        ),
      );
    } else {
      pending.resolve(message.result);
    }
  }

  request(method, params, timeoutMs = 15_000) {
    if (!this.process?.stdin?.writable) {
      return Promise.reject(new Error("codex app-server stdin is not writable"));
    }

    const id = this.nextId;
    this.nextId += 1;
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`${method} timed out after ${timeoutMs}ms`));
      }, timeoutMs);
      this.pending.set(id, { method, resolve, reject, timeout });
      this.process.stdin.write(`${JSON.stringify({ method, id, params })}\n`);
    });
  }

  notify(method, params) {
    this.process.stdin.write(`${JSON.stringify({ method, params })}\n`);
  }

  rejectAll(error) {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timeout);
      pending.reject(error);
    }
    this.pending.clear();
  }

  close() {
    this.lines?.close();
    this.process?.stdin?.end();
    this.process?.kill();
  }
}

function numericRecency(thread) {
  return Number(thread.recencyAt ?? thread.updatedAt ?? thread.createdAt ?? 0);
}

async function findThreadByExactTitle(client, title) {
  let rows = [];
  try {
    const result = await client.request("thread/search", {
      searchTerm: title,
      limit: 100,
    });
    rows = (result?.data || []).map((entry) => entry.thread || entry);
  } catch (searchError) {
    log(`thread/search unavailable; falling back to thread/list (${searchError.message})`);
  }

  let exact = rows.filter((thread) => thread?.name === title);
  if (exact.length === 0) {
    const result = await client.request("thread/list", {
      searchTerm: title,
      limit: 100,
      useStateDbOnly: true,
    });
    rows = result?.data || [];
    exact = rows.filter((thread) => thread?.name === title);
  }

  exact.sort((left, right) => numericRecency(right) - numericRecency(left));

  if (exact.length === 0) {
    throw new Error(
      `No task with the exact title "${title}" was found. Pass --session-id if you already know it.`,
    );
  }
  if (exact.length > 1) {
    log(`Found ${exact.length} exact title matches; using the most recent one.`);
  }
  return exact[0];
}

function lifecycleFromObject(object) {
  if (object?.type !== "event_msg") return null;
  const payload = object.payload;
  const turnId = payload?.turn_id || payload?.turnId || null;
  if (payload?.type === "task_started") {
    return { state: "active", turnId, event: payload.type };
  }
  if (payload?.type === "task_complete" || payload?.type === "turn_aborted") {
    return { state: "idle", turnId, event: payload.type };
  }
  return null;
}

function consumeLifecycleLine(line, current) {
  if (!line.includes('"type":"event_msg"')) return current;
  try {
    return lifecycleFromObject(JSON.parse(line)) || current;
  } catch {
    return current;
  }
}

async function scanRollout(path) {
  let lifecycle = null;
  const lines = createInterface({
    input: createReadStream(path, { encoding: "utf8" }),
    crlfDelay: Infinity,
  });
  for await (const line of lines) {
    lifecycle = consumeLifecycleLine(line, lifecycle);
  }
  return {
    lifecycle,
    offset: statSync(path).size,
    partial: "",
  };
}

async function readRange(path, start, end) {
  let content = "";
  const stream = createReadStream(path, {
    encoding: "utf8",
    start,
    end,
  });
  for await (const chunk of stream) content += chunk;
  return content;
}

async function refreshRollout(path, tracker) {
  const size = statSync(path).size;
  if (size < tracker.offset) return scanRollout(path);
  if (size === tracker.offset) return tracker;

  const added = await readRange(path, tracker.offset, size - 1);
  const lines = `${tracker.partial}${added}`.split(/\r?\n/u);
  const partial = lines.pop() || "";
  let lifecycle = tracker.lifecycle;
  for (const line of lines) {
    lifecycle = consumeLifecycleLine(line, lifecycle);
  }

  // Rollout writes are normally newline terminated. Parse a complete-looking
  // trailing JSON object immediately so idle detection is not delayed.
  if (partial.trimEnd().endsWith("}")) {
    lifecycle = consumeLifecycleLine(partial, lifecycle);
    return { lifecycle, offset: size, partial: "" };
  }
  return { lifecycle, offset: size, partial };
}

function normalizeStatus(status) {
  if (typeof status === "string") return status;
  return status?.type || "unknown";
}

async function readThread(client, threadId) {
  const result = await client.request("thread/read", {
    threadId,
    includeTurns: false,
  });
  return result?.thread || result;
}

async function sendResume(codexCommand, sessionId, message, dryRun) {
  if (dryRun) {
    log(`[dry-run] Would send to ${sessionId}: ${message}`);
    return;
  }

  log(`Sending to ${sessionId}: ${message}`);
  await new Promise((resolve, reject) => {
    const child = spawn(
      codexCommand.executable,
      [
        ...codexCommand.prefixArgs,
        "exec",
        "resume",
        "--json",
        sessionId,
        message,
      ],
      {
        stdio: "inherit",
        windowsHide: true,
        shell: false,
      },
    );
    child.on("error", reject);
    child.on("exit", (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`codex exec resume exited with ${signal || code}`));
    });
  });
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    process.stdout.write(usage());
    return;
  }

  const codexCommand = resolveCodexCommand(options.codexPath);
  const client = new AppServerClient(codexCommand);
  let stopping = false;
  const stop = () => {
    stopping = true;
  };
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);

  try {
    await client.start();

    let thread;
    if (options.sessionId) {
      thread = await readThread(client, options.sessionId);
    } else {
      thread = await findThreadByExactTitle(client, options.title);
    }

    const threadId = thread.id || options.sessionId;
    const sessionId = thread.sessionId || thread.id || options.sessionId;
    const rolloutPath = options.rolloutPath || thread.path;
    if (!threadId || !sessionId) {
      throw new Error("The selected task did not contain a usable thread/session ID");
    }

    log(`Watching "${thread.name || options.title || threadId}"`);
    log(`threadId=${threadId} sessionId=${sessionId}`);

    let tracker = null;
    if (rolloutPath && existsSync(rolloutPath)) {
      tracker = await scanRollout(rolloutPath);
      log(`Rollout fallback: ${rolloutPath}`);
    }

    let lastState = null;
    let lastTriggeredKey = null;
    let sends = 0;

    while (!stopping) {
      let latestThread = thread;
      try {
        latestThread = await readThread(client, threadId);
      } catch (error) {
        log(`Status query warning: ${error.message}`);
      }

      const appStatus = normalizeStatus(latestThread?.status);
      if (tracker) tracker = await refreshRollout(rolloutPath, tracker);

      let state = appStatus;
      let triggerKey = `app-server:${appStatus}`;
      let source = "app-server";
      if (appStatus === "notLoaded" || appStatus === "unknown") {
        if (!tracker?.lifecycle) {
          throw new Error(
            `Status is ${appStatus}, and no rollout lifecycle is available. Pass --rollout <path>.`,
          );
        }
        state = tracker.lifecycle.state;
        triggerKey = `${tracker.lifecycle.event}:${tracker.lifecycle.turnId || "unknown"}`;
        source = "rollout";
      }
      if (state === "idle" && tracker?.lifecycle?.state === "idle") {
        triggerKey = `${tracker.lifecycle.event}:${
          tracker.lifecycle.turnId || "unknown"
        }`;
      }
      if (appStatus === "systemError") {
        throw new Error("Codex reports systemError for the selected task");
      }

      if (state !== lastState) {
        log(`Status: ${state} (source=${source}, app-server=${appStatus})`);
        lastState = state;
      }

      if (state === "idle" && triggerKey !== lastTriggeredKey) {
        await sendResume(codexCommand, sessionId, options.message, options.dryRun);
        lastTriggeredKey = triggerKey;
        sends += 1;
        if (options.maxSends > 0 && sends >= options.maxSends) break;
      }

      if (options.once) break;
      await sleep(options.intervalMs);
    }

    log(`Stopped after ${sends} send${sends === 1 ? "" : "s"}.`);
  } finally {
    client.close();
  }
}

main().catch((error) => {
  process.stderr.write(`Error: ${error.message}\n`);
  process.exitCode = 1;
});
