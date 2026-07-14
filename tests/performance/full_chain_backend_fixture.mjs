#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import fsp from "node:fs/promises";
import http from "node:http";
import net from "node:net";
import path from "node:path";
import process from "node:process";
import { performance } from "node:perf_hooks";

export const repoRoot = path.resolve(import.meta.dirname, "..", "..");

export function marker(workspaceIndex, taskIndex, turn) {
  return `E2E-STRESS-w${workspaceIndex}-t${taskIndex}-r${turn}`;
}

export function round(value) {
  return Math.round(value * 100) / 100;
}

export async function delay(ms) {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

export async function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => resolve(address.port));
    });
    server.on("error", reject);
  });
}

export function normalizePath(value) {
  return String(value || "").replace(/\\/g, "/").replace(/\/+$/u, "").toLowerCase();
}

export function samePath(left, right) {
  return normalizePath(left) === normalizePath(right);
}

export function intEnv(name, fallback, env = process.env) {
  const parsed = Number.parseInt(env[name] || "", 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

export function nonNegativeIntEnv(name, fallback, env = process.env) {
  const parsed = Number.parseInt(env[name] || "", 10);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : fallback;
}

export function boolEnv(name, fallback, env = process.env) {
  const raw = env[name];
  if (raw === undefined || raw === "") return fallback;
  return !["0", "false", "no", "off"].includes(raw.toLowerCase());
}

export function defaultBackendStressConfig(env = process.env, overrides = {}) {
  const config = {
    workspaces: intEnv("TURA_FULL_CHAIN_WORKSPACES", 10, env),
    tasksPerWorkspace: intEnv("TURA_FULL_CHAIN_TASKS_PER_WORKSPACE", 20, env),
    turnsPerSession: intEnv("TURA_FULL_CHAIN_TURNS_PER_SESSION", 5, env),
    liveSessionTarget: nonNegativeIntEnv("TURA_FULL_CHAIN_LIVE_SESSIONS", 20, env),
    turnTimeoutMs: intEnv("TURA_FULL_CHAIN_TURN_TIMEOUT_MS", 30_000, env),
    totalTimeoutMs: intEnv("TURA_FULL_CHAIN_TOTAL_TIMEOUT_MS", 120_000, env),
    createSessionConcurrency: intEnv("TURA_FULL_CHAIN_CREATE_SESSION_CONCURRENCY", 20, env),
    ensureBuilds: boolEnv("TURA_FULL_CHAIN_ENSURE_BUILDS", false, env),
    forceKillTrackedChildren: boolEnv("TURA_FULL_CHAIN_FORCE_KILL_TRACKED_CHILDREN", false, env),
    gatewayVerifyConcurrency: intEnv("TURA_FULL_CHAIN_GATEWAY_VERIFY_CONCURRENCY", 20, env),
  };
  Object.assign(config, overrides);
  config.sessionCount = config.workspaces * config.tasksPerWorkspace;
  config.liveSessionCount = Math.min(config.liveSessionTarget, config.sessionCount);
  config.historicalSessionCount = config.sessionCount - config.liveSessionCount;
  config.expectedRichRecords = config.sessionCount * config.turnsPerSession * 2;
  config.expectedProviderCalls = config.liveSessionCount * config.turnsPerSession;
  return config;
}

export async function startBackendStressEnvironment(options = {}) {
  const harness = new BackendStressHarness(options);
  await harness.start();
  return harness;
}

export class BackendStressHarness {
  constructor(options = {}) {
    this.config = defaultBackendStressConfig(process.env, options.config);
    this.runId =
      options.runId ||
      process.env.TURA_FULL_CHAIN_E2E_RUN_ID ||
      `${options.runIdPrefix || "full-chain-backend"}-${Date.now()}`;
    this.runRoot = path.join(repoRoot, "target", "full-chain-e2e-stress", this.runId);
    this.logsDir = path.join(this.runRoot, "logs");
    this.summaryPath = path.join(this.runRoot, "summary.json");
    this.turaHome = path.join(this.runRoot, "tura-home");
    this.debugDir = path.join(repoRoot, "target", "debug");
    this.exeSuffix = process.platform === "win32" ? ".exe" : "";
    this.timings = [];
    this.checks = [];
    this.processes = [];
    this.cleanupActions = [];
    this.stoppedPids = new Set();
    this.providerRequests = [];
    this.requestErrors = [];
    this.diagnosticSessions = [];
    this.startedAt = performance.now();
    this.stressDeadline = undefined;
    this.provider = undefined;
    this.providerConfig = undefined;
    this.gateway = undefined;
    this.workspaces = [];
    this.sessions = [];
    this.targetSession = undefined;
  }

  remainingBudget(label, reserveMs = 0) {
    const remaining = this.stressDeadline - Date.now() - reserveMs;
    if (remaining <= 0) {
      throw new Error(`${label} exceeded ${this.config.totalTimeoutMs}ms backend stress budget`);
    }
    return remaining;
  }

  boundedTimeout(desiredMs, label, floorMs = 1_000) {
    return Math.max(floorMs, Math.min(desiredMs, this.remainingBudget(label)));
  }

  recordCheck(name, ok, details = {}) {
    this.checks.push({ name, ok, ...details });
    if (!ok) throw new Error(`${name} failed: ${JSON.stringify(details)}`);
  }

  async timed(name, fn) {
    const started = performance.now();
    try {
      const value = await fn();
      this.timings.push({ name, ok: true, elapsedMs: round(performance.now() - started) });
      return value;
    } catch (error) {
      this.timings.push({
        name,
        ok: false,
        elapsedMs: round(performance.now() - started),
        error: String(error?.message || error),
      });
      throw error;
    }
  }

  async start() {
    this.startedAt = performance.now();
    this.stressDeadline = Date.now() + this.config.totalTimeoutMs;
    await fsp.rm(this.runRoot, { recursive: true, force: true });
    await fsp.mkdir(this.logsDir, { recursive: true });
    if (this.config.ensureBuilds) await this.timed("ensure-backend-builds", () => this.ensureBuilds());
    else this.requireArtifacts();
    this.provider = await this.timed("start-local-provider", () => this.startProvider());
    this.providerConfig = await this.writeProviderConfig(this.provider.url);
    this.workspaces = [];
    for (let index = 0; index < this.config.workspaces; index += 1) {
      const workspace = path.join(this.runRoot, "workspaces", `workspace-${index}`);
      await fsp.mkdir(workspace, { recursive: true });
      this.workspaces.push(workspace);
    }
    const gatewayPort = await freePort();
    this.gateway = await this.timed("start-real-gateway-router-session-db", () =>
      this.startGateway(gatewayPort, this.workspaces[0], this.providerConfig),
    );
    this.sessions = await this.timed("run-full-chain-backend-workload", () =>
      this.runWorkload(this.gateway.url, this.workspaces),
    );
    this.targetSession = this.sessions.at(-1);
    return this;
  }

  requireArtifacts() {
    const required = [
      path.join(this.debugDir, `tura_gateway${this.exeSuffix}`),
      path.join(this.debugDir, `tura_router${this.exeSuffix}`),
      path.join(this.debugDir, `tura_runtime${this.exeSuffix}`),
      path.join(this.debugDir, `tura_session_db${this.exeSuffix}`),
    ];
    const missing = required.filter((file) => !fs.existsSync(file));
    if (missing.length > 0) {
      throw new Error(
        `backend full-chain artifacts missing; run setup first or set TURA_FULL_CHAIN_ENSURE_BUILDS=1: ${missing.join(", ")}`,
      );
    }
  }

  ensureBuilds() {
    runChecked(
      "cargo",
      [
        "build",
        "-p",
        "gateway",
        "--bin",
        "tura_gateway",
        "-p",
        "router",
        "--bin",
        "tura_router",
        "-p",
        "runtime",
        "--bin",
        "tura_runtime",
        "-p",
        "session_log",
        "--bin",
        "tura_session_db",
      ],
      { timeoutMs: 300_000 },
    );
  }

  async startProvider() {
    const port = await freePort();
    const server = http.createServer(async (req, res) => {
      const index = this.providerRequests.length;
      const requestInfo = {
        index,
        method: req.method,
        url: req.url,
        promptText: "",
        receivedAt: Date.now(),
        completedAt: undefined,
        bodyBytes: 0,
      };
      this.providerRequests.push(requestInfo);
      res.writeHead(200, {
        "content-type": "text/event-stream",
        "cache-control": "no-cache",
        connection: "close",
      });
      const initial = `### E2E-STRESS provider ${index}\n\n`;
      writeSse(res, { type: "response.output_text.delta", delta: initial });
      const chunks = [];
      let requestFailed = false;
      req.on("error", (error) => {
        requestFailed = true;
        requestInfo.error = String(error?.message || error);
      });
      res.on("error", (error) => {
        requestInfo.responseError = String(error?.message || error);
      });
      req.on("data", (chunk) => chunks.push(chunk));
      req.on("end", () => {
        if (requestFailed || res.destroyed) return;
        const raw = Buffer.concat(chunks).toString("utf8");
        requestInfo.bodyBytes = Buffer.byteLength(raw);
        let body = {};
        try {
          body = raw ? JSON.parse(raw) : {};
        } catch {
          body = { raw };
        }
        const promptText = requestPromptText(body);
        requestInfo.promptText = promptText;
        requestInfo.completedAt = Date.now();
        const content = richAssistantText(index, promptText);
        writeSse(res, { type: "response.output_text.delta", delta: content });
        writeSse(res, {
          type: "response.completed",
          response: {
            id: `resp_full_chain_${index}`,
            output_text: `${initial}${content}`,
            output: [
              {
                id: `msg_full_chain_${index}`,
                type: "message",
                role: "assistant",
                content: [{ type: "output_text", text: `${initial}${content}` }],
              },
            ],
            usage: { input_tokens: 1, output_tokens: 1, total_tokens: 2 },
          },
        });
        if (!res.destroyed) {
          res.write("data: [DONE]\n\n");
          res.end();
        }
      });
    });
    await new Promise((resolve, reject) => {
      server.listen(port, "127.0.0.1", resolve);
      server.on("error", reject);
    });
    return { server, url: `http://127.0.0.1:${port}` };
  }

  async writeProviderConfig(providerUrl) {
    const routes = {};
    for (const route of ["fast", "thinking", "codex/gpt-5.5", "embedding_high", "embedding_low"]) {
      routes[route] = {
        default_temperature: 0,
        providers: [{ provider: "openai", model: "mock-coder", temperature: 0 }],
      };
    }
    const configPath = path.join(this.runRoot, "provider_config.json");
    await fsp.writeFile(
      configPath,
      JSON.stringify(
        {
          provider_base_url: { openai: providerUrl },
          routes,
          model_catalog: {
            tiers: ["fast", "thinking"],
            providers: {
              openai: {
                display_name: "Local OpenAI-compatible stress provider",
                runtime_provider: "openai",
                api_style: "openai-responses",
                base_url: providerUrl,
                token_env: "OPENAI_API_KEY",
                models: { default: [{ id: "mock-coder", name: "Mock Coder" }] },
              },
            },
          },
        },
        null,
        2,
      ),
    );
    return configPath;
  }

  testEnv(extra = {}) {
    return {
      ...process.env,
      ...extra,
      PATH: `${this.debugDir}${path.delimiter}${process.env.PATH || ""}`,
      TURA_HOME: this.turaHome,
      TURA_PROJECT_ROOT: repoRoot,
      OPENAI_API_KEY: "local-stress-key",
      OPENAI_LOGIN: "api_key",
      TURA_SESSION_MODEL_OVERRIDE: "openai/mock-coder",
      TURA_PROVIDER_TOTAL_TIMEOUT_MS: "60000",
      TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS: "10000",
      TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS: "10000",
      TURA_MANAS_MAX_TURNS: "8",
      TURA_NO_TOOL_RETRY_LIMIT: "0",
      TURA_GATEWAY_CALLBACKS: "1",
      FORCE_COLOR: "0",
    };
  }

  startLoggedProcess(command, args, options) {
    const child = spawn(command, args, {
      cwd: options.cwd || repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      stdio: options.stdio || ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
    const stdoutStream = child.stdout && options.stdout ? fs.createWriteStream(options.stdout) : undefined;
    const stderrStream = child.stderr && options.stderr ? fs.createWriteStream(options.stderr) : undefined;
    if (stdoutStream) {
      stdoutStream.on("error", (error) => {
        this.recordCleanup({
          action: "log-stream-error",
          label: `${options.label}:stdout`,
          error: String(error?.message || error),
        });
      });
    }
    if (stderrStream) {
      stderrStream.on("error", (error) => {
        this.recordCleanup({
          action: "log-stream-error",
          label: `${options.label}:stderr`,
          error: String(error?.message || error),
        });
      });
    }
    const entry = {
      child,
      label: options.label || path.basename(command),
      command,
      args,
      cwd: options.cwd || repoRoot,
      stdoutStream,
      stderrStream,
    };
    this.processes.push(entry);
    if (child.stdout && stdoutStream) child.stdout.pipe(stdoutStream);
    if (child.stderr && stderrStream) child.stderr.pipe(stderrStream);
    return child;
  }

  async startGateway(port, workspace, providerConfig) {
    const child = this.startLoggedProcess(path.join(this.debugDir, `tura_gateway${this.exeSuffix}`), [], {
      cwd: workspace,
      env: this.testEnv({
        PORT: String(port),
        TURA_GATEWAY_PORT: String(port),
        TURA_GATEWAY_URL: `http://127.0.0.1:${port}`,
        TURA_CWD: workspace,
        TURA_PROVIDER_CONFIG: providerConfig,
        TURA_GATEWAY_SHUTDOWN_ON_STDIN_EOF: "1",
      }),
      stdio: ["pipe", "pipe", "pipe"],
      label: "gateway",
      stdout: path.join(this.logsDir, "gateway.stdout.log"),
      stderr: path.join(this.logsDir, "gateway.stderr.log"),
    });
    const url = `http://127.0.0.1:${port}`;
    const response = await this.waitForUrl(
      `${url}/global/health`,
      child,
      this.boundedTimeout(20_000, "gateway readiness"),
    );
    await response.body?.cancel().catch(() => undefined);
    return { child, url };
  }

  async waitForUrl(url, child, timeoutMs) {
    const deadline = Date.now() + timeoutMs;
    let lastError;
    while (Date.now() < deadline) {
      if (child?.exitCode !== null) throw new Error(`${url} exited before readiness: ${child.exitCode}`);
      try {
        const response = await fetch(url);
        if (response.ok) return response;
        await response.body?.cancel().catch(() => undefined);
        lastError = new Error(`${url} returned ${response.status}`);
      } catch (error) {
        lastError = error;
      }
      await delay(250);
    }
    throw lastError || new Error(`timed out waiting for ${url}`);
  }

  async requestJson(gatewayUrl, method, apiPath, payload, workspace, timeoutMs = 30_000, ignoreBudget = false) {
    const bounded =
      this.stressDeadline && !ignoreBudget
        ? this.boundedTimeout(timeoutMs, `${method} ${apiPath}`, 250)
        : timeoutMs;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), bounded);
    try {
      const headers = { "content-type": "application/json" };
      if (workspace) headers["x-opencode-directory"] = encodeURIComponent(workspace);
      const response = await fetch(`${gatewayUrl}${apiPath}`, {
        method,
        headers,
        body: payload === undefined ? undefined : JSON.stringify(payload),
        signal: controller.signal,
      });
      const text = await response.text();
      if (!response.ok) throw new Error(`${method} ${apiPath} returned ${response.status}: ${text}`);
      return text.trim() ? JSON.parse(text) : undefined;
    } catch (error) {
      this.requestErrors.push({
        method,
        apiPath,
        workspace,
        name: error?.name,
        message: error?.message || String(error),
        cause: error?.cause ? String(error.cause?.message || error.cause) : undefined,
        at: Date.now(),
      });
      throw error;
    } finally {
      clearTimeout(timer);
    }
  }

  async callSessionDb(command, timeoutMs = 30_000) {
    const addrPath = path.join(this.turaHome, "db", "session_log", "service.addr");
    const endpoint = JSON.parse(await fsp.readFile(addrPath, "utf8"));
    const [host, portText] = String(endpoint.addr || "").split(":");
    const port = Number(portText);
    if (!host || !Number.isFinite(port)) throw new Error(`invalid session_db addr: ${endpoint.addr}`);
    const requestLine = `${JSON.stringify(command)}\n`;
    return new Promise((resolve, reject) => {
      const socket = net.createConnection({ host, port });
      let buffered = "";
      let settled = false;
      const finish = (fn, value) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        fn(value);
      };
      const timer = setTimeout(() => {
        socket.destroy();
        finish(reject, new Error(`session_db IPC ${command.command} timed out after ${timeoutMs}ms`));
      }, timeoutMs);
      socket.setEncoding("utf8");
      socket.on("connect", () => socket.write(requestLine));
      socket.on("data", (chunk) => {
        if (settled) return;
        buffered += chunk;
        const newline = buffered.indexOf("\n");
        if (newline < 0) return;
        const line = buffered.slice(0, newline).trim();
        socket.end();
        try {
          const response = JSON.parse(line);
          if (response.kind === "error") finish(reject, new Error(response.error || `${command.command} failed`));
          else finish(resolve, response);
        } catch (error) {
          finish(reject, error);
        }
      });
      socket.on("error", (error) => {
        finish(reject, error);
      });
      socket.on("close", () => {
        finish(reject, new Error(`session_db IPC ${command.command} closed before response`));
      });
    });
  }

  async waitForSessionDbReady(timeoutMs) {
    const deadline = Date.now() + timeoutMs;
    let lastError;
    while (Date.now() < deadline) {
      try {
        const response = await this.callSessionDb({ command: "health" }, 2_000);
        if (response.kind === "ok") return;
        lastError = new Error(`unexpected session_db health response: ${JSON.stringify(response)}`);
      } catch (error) {
        lastError = error;
      }
      await delay(250);
    }
    throw lastError || new Error("session_db service did not become ready");
  }

  taskCoordinates() {
    const coordinates = [];
    for (let taskIndex = 0; taskIndex < this.config.tasksPerWorkspace; taskIndex += 1) {
      for (let workspaceIndex = 0; workspaceIndex < this.config.workspaces; workspaceIndex += 1) {
        coordinates.push({ workspaceIndex, taskIndex });
      }
    }
    return coordinates;
  }

  async createHttpSession(gatewayUrl, workspace, name) {
    const session = await this.requestJson(
      gatewayUrl,
      "POST",
      scoped("/session", workspace),
      {
        directory: workspace,
        agent: "direct-text-only",
        model: "openai/mock-coder",
        model_variant: "default",
        model_acceleration_enabled: false,
        disable_permission_restrictions: true,
        auto_session_name: false,
      },
      workspace,
    );
    await this.requestJson(gatewayUrl, "PATCH", `/session/${encodeURIComponent(session.id)}`, { name }, workspace);
    return session.id;
  }

  async submitTurn(gatewayUrl, workspace, sessionId, currentMarker) {
    await this.requestJson(
      gatewayUrl,
      "POST",
      `/session/${encodeURIComponent(sessionId)}/prompt_async`,
      {
        messageID: `msg_${currentMarker}`,
        parts: [{ id: `part_${currentMarker}`, type: "text", text: richUserPrompt(currentMarker) }],
        model: "openai/mock-coder",
        agent: "direct-text-only",
        source: "backend-stress",
      },
      workspace,
    );
  }

  async upsertHistoricalSession(workspace, workspaceIndex, taskIndex) {
    const sessionId = `historical-w${workspaceIndex}-t${taskIndex}-${this.runId}`.replace(/[^a-zA-Z0-9_.:-]/g, "-");
    const sessionName = `E2E-STRESS historical workspace ${workspaceIndex} task ${taskIndex}`;
    const createdAt = Date.now() - 60_000 + workspaceIndex * 1_000 + taskIndex * 10;
    const updatedAt = createdAt + this.config.turnsPerSession * 2_000;
    const management = this.historicalManagement({ sessionId, sessionName, workspace, createdAt, updatedAt });
    const messages = [];
    for (let turn = 0; turn < this.config.turnsPerSession; turn += 1) {
      const currentMarker = marker(workspaceIndex, taskIndex, turn);
      const userCreatedAt = createdAt + turn * 2_000;
      const assistantCreatedAt = userCreatedAt + 1_000;
      messages.push(historicalMessage(sessionId, "user", currentMarker, richUserPrompt(currentMarker), userCreatedAt));
      messages.push(
        historicalMessage(
          sessionId,
          "assistant",
          currentMarker,
          richHistoricalAssistantText(currentMarker),
          assistantCreatedAt,
        ),
      );
    }
    const taskManagement = {
      plan_summary: sessionName,
      tasks: [
        {
          task_id: `task-${workspaceIndex}-${taskIndex}`,
          step: 0,
          status: "done",
          task_summary: sessionName,
          step_task: sessionName,
        },
      ],
    };
    const session = {
      id: sessionId,
      name: sessionName,
      directory: workspace,
      created_at: createdAt,
      updated_at: updatedAt,
      model: "openai/mock-coder",
      agent: "direct-text-only",
      session_type: "coding",
      auto_session_name: false,
      kill_processes_on_start: false,
      validator_enabled: false,
      force_planning: false,
      model_variant: "default",
      model_acceleration_enabled: false,
      disable_permission_restrictions: true,
      status: "idle",
      message_count: messages.length,
      task_management: taskManagement,
      management,
    };
    await this.callSessionDb({
      command: "upsert_session",
      session,
      parent_id: null,
      messages,
      todos: [],
    });
    return { workspaceIndex, taskIndex, workspace, sessionId, mode: "historical" };
  }

  historicalManagement({ sessionId, sessionName, workspace, createdAt, updatedAt }) {
    return {
      session_id: sessionId,
      session_name: sessionName,
      auto_session_name: false,
      session_directory: workspace,
      session_uses_docker: false,
      task_type: [],
      session_capabilities: [],
      session_current_turn: this.config.turnsPerSession,
      session_log: [],
      session_created_at: new Date(createdAt).toISOString(),
      session_last_update_at: new Date(updatedAt).toISOString(),
      session_started_at: new Date(createdAt).toISOString(),
      input: {
        user_input: `Historical pressure seed for ${sessionName}`,
        file_input: [],
        agent: "direct-text-only",
        runtime_context: null,
        planning_mode_override: null,
      },
      user_goal: sessionName,
      current_objective: sessionName,
      task_plan: {
        plan_summary: sessionName,
        detailed_tasks: [],
      },
      state: "completed",
      use_last_tool_call_response: false,
      is_child_session: false,
      disable_permission_restrictions: true,
      planning_enabled: false,
      reflection_enabled: false,
      op_manual_enabled: true,
      no_op_manual: false,
      goal_mode: false,
      last_goal_user_input: "",
      context_tokens: { input: 0, limit: 255000 },
      runtime_usage: { total_tokens: 0 },
    };
  }

  async waitForRecords(gatewayUrl, sessionId, expected, currentMarker, timeoutMs) {
    const deadline = Math.min(Date.now() + timeoutMs, this.stressDeadline ?? Number.POSITIVE_INFINITY);
    let last;
    let lastError;
    while (Date.now() < deadline) {
      try {
        const response = await this.requestJson(
          gatewayUrl,
          "GET",
          `/session-log/${encodeURIComponent(sessionId)}/records?page=0&page_size=200`,
          undefined,
          undefined,
          15_000,
        );
        last = response;
        const records = response.records || [];
        const transcriptRecords = records.filter((record) => record.role === "user" || record.role === "assistant");
        const hasAssistantMarker =
          !currentMarker ||
          transcriptRecords.some((record) => record.role === "assistant" && recordContains(record, currentMarker));
        if (transcriptRecords.length >= expected && hasAssistantMarker) return response;
      } catch (error) {
        lastError = error;
      }
      await delay(500);
    }
    throw new Error(
      `timed out waiting for ${expected} transcript records in ${sessionId}; saw ${last?.records?.length ?? 0} total records; last error: ${lastError?.message || "none"}`,
    );
  }

  async runWorkload(gatewayUrl, workspaces) {
    const sessions = [];
    const coordinates = this.taskCoordinates();
    const liveCoordinates = coordinates.slice(0, this.config.liveSessionCount);
    const historicalCoordinates = coordinates.slice(this.config.liveSessionCount);
    const liveKeys = new Set(liveCoordinates.map(coordinateKey));

    await this.timed("gateway-http-create-live-sessions", async () => {
      await mapLimit(liveCoordinates, this.config.createSessionConcurrency, async ({ workspaceIndex, taskIndex }) => {
        const sessionId = await this.createHttpSession(
          gatewayUrl,
          workspaces[workspaceIndex],
          `E2E-STRESS workspace ${workspaceIndex} task ${taskIndex}`,
        );
        sessions.push(this.trackDiagnosticSession({
          workspaceIndex,
          taskIndex,
          workspace: workspaces[workspaceIndex],
          sessionId,
          mode: "live",
          entrypoint: "gateway-http",
        }));
      });
    });

    await this.timed("session-db-upsert-historical-sessions", async () => {
      if (historicalCoordinates.length === 0) return;
      await this.waitForSessionDbReady(this.boundedTimeout(10_000, "session_db readiness"));
      const historicalSessions = await mapLimit(
        historicalCoordinates,
        this.config.createSessionConcurrency,
        async ({ workspaceIndex, taskIndex }) =>
          this.upsertHistoricalSession(workspaces[workspaceIndex], workspaceIndex, taskIndex),
      );
      for (const session of historicalSessions) sessions.push(this.trackDiagnosticSession(session));
    });

    assert.equal(sessions.length, this.config.sessionCount);

    for (let turn = 0; turn < this.config.turnsPerSession; turn += 1) {
      await this.timed(`turn-${turn}-submit-and-drain`, async () => {
        const liveSessions = sessions.filter((session) => liveKeys.has(coordinateKey(session)));
        await Promise.all(
          liveSessions.map((session) =>
            this.submitTurn(gatewayUrl, session.workspace, session.sessionId, marker(session.workspaceIndex, session.taskIndex, turn)),
          ),
        );
        await Promise.all(
          liveSessions.map((session) =>
            this.waitForRecords(
              gatewayUrl,
              session.sessionId,
              (turn + 1) * 2,
              marker(session.workspaceIndex, session.taskIndex, turn),
              this.config.turnTimeoutMs,
            ),
          ),
        );
      });
    }

    return sessions.sort((left, right) =>
      left.workspaceIndex - right.workspaceIndex || left.taskIndex - right.taskIndex,
    );
  }

  trackDiagnosticSession(session) {
    this.diagnosticSessions.push(session);
    return session;
  }

  async verifyTargetSession(session = this.targetSession) {
    if (!session) throw new Error("backend stress target session is not available");
    const lastMarker = marker(session.workspaceIndex, session.taskIndex, this.config.turnsPerSession - 1);
    const response = await this.waitForRecords(
      this.gateway.url,
      session.sessionId,
      this.config.turnsPerSession * 2,
      lastMarker,
      10_000,
    );
    this.recordCheck("target-session-rich-records-visible", JSON.stringify(response.records || []).includes(lastMarker), {
      sessionId: session.sessionId,
      marker: lastMarker,
    });
    return response;
  }

  async verifySessionLog() {
    return this.timed("verify-session-log-rich-records", async () => {
      const workspaceResponse = await this.requestJson(this.gateway.url, "GET", "/session-log/workspaces");
      for (const workspace of this.workspaces) {
        const summary = (workspaceResponse.workspaces || []).find((item) => samePath(item.directory, workspace));
        this.recordCheck("workspace-summary-present", Boolean(summary), { workspace });
        this.recordCheck("workspace-session-count", summary.session_count === this.config.tasksPerWorkspace, {
          workspace,
          sessionCount: summary.session_count,
        });
      }
      let totalRecords = 0;
      let totalRichRecords = 0;
      const roleTotals = {};
      const samples = [];
      for (const session of this.sessions) {
        const lastMarker = marker(session.workspaceIndex, session.taskIndex, this.config.turnsPerSession - 1);
        const response = await this.waitForRecords(
          this.gateway.url,
          session.sessionId,
          this.config.turnsPerSession * 2,
          lastMarker,
          10_000,
        );
        totalRecords += response.records.length;
        for (const record of response.records) {
          roleTotals[record.role] = (roleTotals[record.role] || 0) + 1;
        }
        let sessionRichRecords = 0;
        for (let turn = 0; turn < this.config.turnsPerSession; turn += 1) {
          const expectedMarker = marker(session.workspaceIndex, session.taskIndex, turn);
          const userRecord = response.records.find(
            (record) => record.role === "user" && recordContains(record, expectedMarker),
          );
          const assistantRecord = response.records.find(
            (record) => record.role === "assistant" && recordContains(record, expectedMarker),
          );
          assert.ok(userRecord, `${session.sessionId} missing user rich record ${expectedMarker}`);
          assert.ok(assistantRecord, `${session.sessionId} missing assistant rich record ${expectedMarker}`);
          sessionRichRecords += 2;
        }
        totalRichRecords += sessionRichRecords;
        if (samples.length < 5) {
          samples.push({
            sessionId: session.sessionId,
            mode: session.mode,
            records: response.records.length,
            richRecords: sessionRichRecords,
            roles: countRoles(response.records),
          });
        }
      }
      this.recordCheck("total-rich-record-count", totalRichRecords === this.config.expectedRichRecords, {
        totalRichRecords,
        totalRecords,
        expected: this.config.expectedRichRecords,
        roleTotals,
        samples,
      });
      const gatewayVisible = await this.verifyGatewayVisibleSessions();
      return { totalRichRecords, totalRecords, roleTotals, samples, gatewayVisible };
    });
  }

  async verifyGatewayVisibleSessions() {
    for (const workspace of [...new Set(this.sessions.map((session) => session.workspace))]) {
      const hydrated = await this.requestJson(
        this.gateway.url,
        "GET",
        `/session?${new URLSearchParams({ directory: workspace, limit: "500" })}`,
        undefined,
        workspace,
      );
      this.recordCheck("gateway-workspace-hydrated", Array.isArray(hydrated), {
        workspace,
        sessionCount: Array.isArray(hydrated) ? hydrated.length : undefined,
      });
    }
    let visibleSessions = 0;
    let visibleMessages = 0;
    const samples = [];
    await mapLimit(this.sessions, this.config.gatewayVerifyConcurrency, async (session) => {
      const detail = await this.requestJson(
        this.gateway.url,
        "GET",
        `/session/${encodeURIComponent(session.sessionId)}`,
        undefined,
        session.workspace,
      );
      this.recordCheck("gateway-session-detail-visible", detail?.id === session.sessionId, {
        sessionId: session.sessionId,
        mode: session.mode,
        returnedId: detail?.id,
      });
      this.recordCheck("gateway-session-detail-idle", detail?.status === "idle", {
        sessionId: session.sessionId,
        mode: session.mode,
        status: detail?.status,
      });
      this.recordCheck("gateway-session-detail-hydrated", samePath(detail?.directory, session.workspace), {
        sessionId: session.sessionId,
        mode: session.mode,
        directory: detail?.directory,
        expectedDirectory: session.workspace,
        messageCount: detail?.message_count,
      });
      const messages = await this.requestJson(
        this.gateway.url,
        "GET",
        `/session/${encodeURIComponent(session.sessionId)}/message?limit=200`,
        undefined,
        session.workspace,
      );
      const messagesText = JSON.stringify(messages);
      const lastMarker = marker(session.workspaceIndex, session.taskIndex, this.config.turnsPerSession - 1);
      this.recordCheck("gateway-session-message-count", Array.isArray(messages) && messages.length >= this.config.turnsPerSession * 2, {
        sessionId: session.sessionId,
        mode: session.mode,
        detailMessageCount: detail?.message_count,
        messageCount: Array.isArray(messages) ? messages.length : undefined,
        expectedAtLeast: this.config.turnsPerSession * 2,
      });
      this.recordCheck("gateway-session-messages-visible", Array.isArray(messages) && messagesText.includes(lastMarker), {
        sessionId: session.sessionId,
        mode: session.mode,
        messageCount: Array.isArray(messages) ? messages.length : undefined,
        marker: lastMarker,
      });
      visibleSessions += 1;
      visibleMessages += Array.isArray(messages) ? messages.length : 0;
      if (samples.length < 5) {
        samples.push({
          sessionId: session.sessionId,
          mode: session.mode,
          status: detail?.status,
          messageCount: Array.isArray(messages) ? messages.length : 0,
        });
      }
    });
    this.recordCheck("gateway-visible-session-count", visibleSessions === this.config.sessionCount, {
      visibleSessions,
      expected: this.config.sessionCount,
    });
    return { visibleSessions, visibleMessages, samples };
  }

  providerMarkerCounts() {
    const counts = {};
    for (const request of this.providerRequests) {
      const providerMarker = markerFromText(request.promptText || "") || "unknown";
      counts[providerMarker] = (counts[providerMarker] || 0) + 1;
    }
    return counts;
  }

  latencyFindings() {
    const sorted = [...this.timings].sort((left, right) => right.elapsedMs - left.elapsedMs);
    const providerCount = this.providerRequests.length;
    const findings = [];
    findings.push({
      area: "provider/runtime turns",
      evidence: `local provider received ${providerCount}/${this.config.expectedProviderCalls} expected calls`,
      status: providerCount === this.config.expectedProviderCalls ? "covered" : "mismatch",
    });
    for (const item of sorted.slice(0, 8)) {
      findings.push({ area: item.name, elapsedMs: item.elapsedMs, status: item.ok ? "slowest-stage" : "failed" });
    }
    findings.push({
      area: "session_db write amplification",
      evidence:
        "session_log upsert rewrites records per session turn; inspect crates/session_log/src/store/write.rs around session_records DELETE/INSERT if this stage dominates.",
      status: "known-risk-to-confirm-with-profile",
    });
    return findings;
  }

  async collectFailureDiagnostics(gatewayUrl = this.gateway?.url, sessions = this.diagnosticSessions) {
    const providerMarkers = this.providerMarkerCounts();
    const sessionSnapshots = await mapLimit(sessions, 16, async (session) => {
      const expectedMarkers = Array.from({ length: this.config.turnsPerSession }, (_, turn) =>
        marker(session.workspaceIndex, session.taskIndex, turn),
      );
      const recordsResponse = gatewayUrl
        ? await this.requestJson(
            gatewayUrl,
            "GET",
            `/session-log/${encodeURIComponent(session.sessionId)}/records?page=0&page_size=200`,
            undefined,
            undefined,
            2_000,
            true,
          ).catch((error) => ({ error: String(error?.message || error), records: [] }))
        : { records: [] };
      const records = recordsResponse.records || [];
      const recordText = JSON.stringify(records);
      const missingMarkers = expectedMarkers.filter((item) => !recordText.includes(item));
      return {
        ...session,
        recordCount: records.length,
        roles: countRoles(records),
        missingMarkers,
        providerCallsForSession: expectedMarkers.reduce((total, item) => total + (providerMarkers[item] || 0), 0),
        recordsError: recordsResponse.error,
      };
    });
    const recordCountBuckets = {};
    for (const snapshot of sessionSnapshots) {
      recordCountBuckets[String(snapshot.recordCount)] =
        (recordCountBuckets[String(snapshot.recordCount)] || 0) + 1;
    }
    const diagnostics = {
      totalSessions: sessions.length,
      providerMarkerCounts: providerMarkers,
      providerMarkerCount: Object.keys(providerMarkers).length,
      recordCountBuckets,
      zeroRecordSessions: sessionSnapshots.filter((snapshot) => snapshot.recordCount === 0).slice(0, 30),
      incompleteSessions: sessionSnapshots.filter((snapshot) => snapshot.missingMarkers.length > 0).slice(0, 30),
    };
    await fsp.writeFile(
      path.join(this.logsDir, "failure-diagnostics.json"),
      JSON.stringify(diagnostics, null, 2),
    ).catch(() => undefined);
    return diagnostics;
  }

  summaryBase(extra = {}) {
    return {
      config: this.config,
      runRoot: this.runRoot,
      summaryPath: this.summaryPath,
      workspaces: this.workspaces,
      gatewayUrl: this.gateway?.url,
      providerCalls: this.providerRequests.length,
      expectedProviderCalls: this.config.expectedProviderCalls,
      providerRequests: this.providerRequests.slice(0, 20),
      requestErrors: this.requestErrors.slice(0, 50),
      targetSession: this.targetSession
        ? {
            ...this.targetSession,
            marker: marker(this.targetSession.workspaceIndex, this.targetSession.taskIndex, this.config.turnsPerSession - 1),
          }
        : undefined,
      timings: this.timings,
      checks: this.checks,
      cleanupActions: this.cleanupActions,
      latencyFindings: this.latencyFindings(),
      elapsedMs: round(performance.now() - this.startedAt),
      ...extra,
    };
  }

  recordCleanup(action) {
    this.cleanupActions.push({ at: new Date().toISOString(), ...action });
  }

  terminateSingleProcess(child, label, reason, signal = "SIGTERM") {
    if (!child) return;
    if (processHasExited(child)) {
      this.recordCleanup({ action: "skip-exited", label, pid: child.pid, reason, ...processExitState(child) });
      return;
    }
    if (child.pid) this.stoppedPids.add(child.pid);
    try {
      const sent = child.kill(signal);
      this.recordCleanup({ action: "signal", label, pid: child.pid, reason, signal, sent, ...processExitState(child) });
    } catch (error) {
      this.recordCleanup({
        action: "signal-error",
        label,
        pid: child.pid,
        reason,
        signal,
        error: String(error?.message || error),
        ...processExitState(child),
      });
    }
  }

  async stopProcess(entry) {
    if (!entry?.child) return;
    const target = entry.child;
    const label = entry.label || "process";
    try {
      if (processHasExited(target)) {
        this.recordCleanup({ action: "already-exited", label, pid: target.pid, ...processExitState(target) });
        return;
      }
      if (target.stdin && !target.stdin.destroyed) {
        try {
          target.stdin.end();
          this.recordCleanup({ action: "stdin-end", label, pid: target.pid, ...processExitState(target) });
        } catch (error) {
          this.recordCleanup({
            action: "stdin-end-error",
            label,
            pid: target.pid,
            error: String(error?.message || error),
            ...processExitState(target),
          });
        }
      }
      await waitForProcessExit(target, 2_000);
      if (processHasExited(target)) {
        this.recordCleanup({ action: "exited-after-stdin", label, pid: target.pid, ...processExitState(target) });
        return;
      }
      this.terminateSingleProcess(target, label, "cleanup");
      await waitForProcessExit(target, 2_000);
      if (processHasExited(target)) {
        this.recordCleanup({ action: "exited-after-signal", label, pid: target.pid, ...processExitState(target) });
        return;
      }
      if (!this.config.forceKillTrackedChildren) {
        this.recordCleanup({
          action: "still-running-after-term-no-force-kill",
          label,
          pid: target.pid,
          reason: "set TURA_FULL_CHAIN_FORCE_KILL_TRACKED_CHILDREN=1 to send SIGKILL to this tracked child pid",
          ...processExitState(target),
        });
        return;
      }
      this.terminateSingleProcess(target, label, "cleanup-timeout", "SIGKILL");
      await waitForProcessExit(target, 1_000);
      this.recordCleanup({
        action: processHasExited(target) ? "exited-after-kill" : "still-running-after-single-pid-kill",
        label,
        pid: target.pid,
        ...processExitState(target),
      });
    } finally {
      await this.waitForEntryStreams(entry, label);
    }
  }

  async waitForEntryStreams(entry, label) {
    await Promise.all([
      this.waitForWritableClosed(entry.stdoutStream, `${label}:stdout`),
      this.waitForWritableClosed(entry.stderrStream, `${label}:stderr`),
    ]);
  }

  waitForWritableClosed(stream, label, timeoutMs = 1_000) {
    if (!stream || stream.closed || stream.destroyed) return Promise.resolve("closed");
    return Promise.race([
      new Promise((resolve) => {
        stream.once("close", () => resolve("close"));
        stream.once("finish", () => resolve("finish"));
        stream.once("error", (error) => resolve(`error:${error?.message || error}`));
      }).then((result) => {
        this.recordCleanup({ action: "log-stream-closed", label, result });
        return result;
      }),
      delay(timeoutMs).then(() => {
        this.recordCleanup({ action: "log-stream-timeout", label });
        stream.destroy();
        return "timeout";
      }),
    ]);
  }

  async closeServer(server) {
    if (!server) return;
    let settled = false;
    const closePromise = new Promise((resolve) => {
      server.close((error) => {
        settled = true;
        resolve(error);
      });
    });
    const result = await Promise.race([closePromise, delay(3_000).then(() => "timeout")]);
    if (result !== "timeout") return;
    server.closeIdleConnections?.();
    server.closeAllConnections?.();
    await Promise.race([closePromise, delay(2_000)]);
    if (!settled) console.warn("provider server close timed out during cleanup");
  }

  activeHandleSummary() {
    const handles = typeof process._getActiveHandles === "function" ? process._getActiveHandles() : [];
    return handles.map((handle) => ({
      type: handle?.constructor?.name || typeof handle,
      fd: typeof handle?.fd === "number" ? handle.fd : undefined,
      pid: typeof handle?.pid === "number" ? handle.pid : undefined,
      destroyed: typeof handle?.destroyed === "boolean" ? handle.destroyed : undefined,
      readableEnded: typeof handle?.readableEnded === "boolean" ? handle.readableEnded : undefined,
      writableEnded: typeof handle?.writableEnded === "boolean" ? handle.writableEnded : undefined,
    }));
  }

  async cleanup() {
    const cleanupErrors = [];
    await this.closeServer(this.provider?.server).catch((error) => {
      cleanupErrors.push(String(error?.stack || error?.message || error));
      this.recordCleanup({ action: "provider-close-error", error: String(error?.message || error) });
    });
    for (const entry of [...this.processes].reverse()) {
      await this.stopProcess(entry).catch((error) => {
        cleanupErrors.push(String(error?.stack || error?.message || error));
        this.recordCleanup({
          action: "tracked-process-cleanup-error",
          label: entry?.label,
          pid: entry?.child?.pid,
          error: String(error?.message || error),
        });
      });
    }
    if (cleanupErrors.length > 0) {
      await fsp.writeFile(path.join(this.logsDir, "cleanup-errors.json"), JSON.stringify(cleanupErrors, null, 2)).catch(
        (error) => console.warn(`failed to write cleanup-errors.json: ${error?.message || error}`),
      );
    }
    await fsp.writeFile(path.join(this.logsDir, "cleanup-actions.json"), JSON.stringify(this.cleanupActions, null, 2)).catch(
      (error) => console.warn(`failed to write cleanup-actions.json: ${error?.message || error}`),
    );
    await fsp.writeFile(
      path.join(this.logsDir, "active-handles-after-cleanup.json"),
      JSON.stringify(this.activeHandleSummary(), null, 2),
    ).catch((error) => console.warn(`failed to write active-handles-after-cleanup.json: ${error?.message || error}`));
  }
}

function runChecked(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || repoRoot,
    env: { ...process.env, ...(options.env || {}) },
    encoding: "utf8",
    text: true,
    timeout: options.timeoutMs || 120_000,
    windowsHide: true,
    maxBuffer: 128 * 1024 * 1024,
    shell: options.shell || false,
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status || result.signal}\nSTDOUT:\n${result.stdout || ""}\nSTDERR:\n${result.stderr || ""}`,
      { cause: result.error },
    );
  }
  return result;
}

function writeSse(res, value) {
  res.write(`data: ${JSON.stringify(value)}\n\n`);
}

function requestPromptText(value) {
  const texts = [];
  function walk(node) {
    if (!node || typeof node !== "object") return;
    if (typeof node.text === "string") texts.push(node.text);
    if (typeof node.content === "string") texts.push(node.content);
    if (Array.isArray(node)) node.forEach(walk);
    else Object.values(node).forEach(walk);
  }
  walk(value.input ?? value.messages ?? value);
  return texts.join("\n").slice(-600);
}

function richAssistantText(index, promptText) {
  const currentMarker = markerFromText(promptText) || `provider-${index}`;
  return [
    `### E2E-STRESS assistant ${currentMarker}`,
    "",
    "The local provider completed this turn through the real backend path.",
    "",
    "| layer | status |",
    "| --- | --- |",
    "| gateway | persisted |",
    "| router | dispatched |",
    "| runtime | completed |",
    "| session_db | queued-write-drained |",
    "",
    "```json",
    JSON.stringify({ marker: currentMarker, provider_request: index }),
    "```",
    "",
    `<b>E2E-STRESS ${currentMarker}</b>`,
  ].join("\n");
}

function richHistoricalAssistantText(currentMarker) {
  return [
    `### E2E-STRESS assistant ${currentMarker}`,
    "",
    "This completed historical turn was written through the session_db IPC path and replayed through gateway APIs.",
    "",
    "| layer | status |",
    "| --- | --- |",
    "| session_db | ipc-upsert |",
    "| gateway | hydrated |",
    "",
    "```json",
    JSON.stringify({ marker: currentMarker, source: "historical-session-db-ipc" }),
    "```",
    "",
    `<b>E2E-STRESS ${currentMarker}</b>`,
  ].join("\n");
}

function markerFromText(text) {
  const matches = [...String(text || "").matchAll(/E2E-STRESS-w\d+-t\d+-r\d+/gu)];
  return matches.at(-1)?.[0];
}

function scoped(pathname, workspace) {
  const separator = pathname.includes("?") ? "&" : "?";
  return `${pathname}${separator}${new URLSearchParams({ directory: workspace })}`;
}

function coordinateKey({ workspaceIndex, taskIndex }) {
  return `${workspaceIndex}:${taskIndex}`;
}

async function mapLimit(items, limit, mapper) {
  const results = new Array(items.length);
  let next = 0;
  const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (next < items.length) {
      const index = next;
      next += 1;
      results[index] = await mapper(items[index], index);
    }
  });
  await Promise.all(workers);
  return results;
}

function historicalMessage(sessionId, role, currentMarker, text, createdAt) {
  const messageId = `hist-${sessionId}-${role}-${currentMarker}`.replace(/[^a-zA-Z0-9_.:-]/g, "-");
  return {
    id: messageId,
    session_id: sessionId,
    role,
    parent_id: null,
    parts: [
      {
        id: `${messageId}-part`,
        type: "text",
        content: text,
        text,
        metadata: { marker: currentMarker, source: "full-chain-historical-session" },
        call_id: null,
        tool: null,
        state: null,
      },
    ],
    created_at: createdAt,
    updated_at: createdAt,
  };
}

function richUserPrompt(currentMarker) {
  return [
    `### E2E-STRESS ${currentMarker}`,
    "",
    "Persist this rich text through gateway, router, runtime, provider, and session_db.",
    "",
    "| component | requirement |",
    "| --- | --- |",
    "| gateway | prompt_async |",
    "| router | runtime dispatch |",
    "| session_db | durable records |",
    "",
    "```ts",
    `const marker = '${currentMarker}';`,
    "```",
    "",
    `<b>${currentMarker}</b> [local](file:///tmp/${currentMarker})`,
  ].join("\n");
}

function recordContains(record, needle) {
  return JSON.stringify(record.record ?? record).includes(needle);
}

function countRoles(records) {
  const counts = {};
  for (const record of records) counts[record.role] = (counts[record.role] || 0) + 1;
  return counts;
}

function processExitState(child) {
  return {
    exitCode: child?.exitCode ?? null,
    signalCode: child?.signalCode ?? null,
    killed: Boolean(child?.killed),
  };
}

function processHasExited(child) {
  return !child || child.exitCode !== null || child.signalCode !== null;
}

function waitForProcessExit(child, timeoutMs) {
  if (processHasExited(child)) return Promise.resolve("exited");
  return Promise.race([
    new Promise((resolve) => child.once("exit", () => resolve("exited"))),
    delay(timeoutMs).then(() => "timeout"),
  ]);
}
