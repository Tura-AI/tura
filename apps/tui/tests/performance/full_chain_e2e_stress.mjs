#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import fsp from "node:fs/promises";
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import process from "node:process";
import { performance } from "node:perf_hooks";
import { Readable } from "node:stream";
import {
  delay,
  freePort,
  intEnv,
  marker,
  nonNegativeIntEnv,
  round,
  startBackendStressEnvironment,
} from "../../../../tests/performance/full_chain_backend_fixture.mjs";

const appRoot = path.resolve(import.meta.dirname, "..", "..");

const config = {
  openBudgetMs: intEnv("TURA_FULL_CHAIN_TUI_OPEN_BUDGET_MS", 6_000),
  readBudgetMs: intEnv("TURA_FULL_CHAIN_TUI_READ_BUDGET_MS", 10_000),
  renderBudgetMs: intEnv("TURA_FULL_CHAIN_TUI_RENDER_BUDGET_MS", 5_000),
  minAvgFps: Number.parseFloat(process.env.TURA_FULL_CHAIN_TUI_MIN_AVG_FPS || "30"),
  maxFrameGapMs: Number.parseFloat(process.env.TURA_FULL_CHAIN_TUI_MAX_FRAME_GAP_MS || "1200"),
  maxLongFrames: intEnv("TURA_FULL_CHAIN_TUI_MAX_LONG_FRAMES", 240),
  measuredSessionCount: intEnv("TURA_FULL_CHAIN_FRONTEND_MEASURED_SESSIONS", 2),
  backgroundReadConcurrency: nonNegativeIntEnv("TURA_FULL_CHAIN_FRONTEND_READ_CONCURRENCY", 8),
  backgroundReadRequests: nonNegativeIntEnv("TURA_FULL_CHAIN_FRONTEND_READ_REQUESTS", 120),
};

async function main() {
  const backend = await startBackendStressEnvironment({ runIdPrefix: "tui-full-chain" });
  let proxy;
  let webTerminal;
  let completedOk = false;
  try {
    requireTuiArtifacts();
    const targets = selectTargetSessions(backend);
    for (const target of targets) await backend.verifyTargetSession(target);
    proxy = await startRecordingProxy(backend.gateway.url);
    const webPort = await freePort();
    webTerminal = await startWebTerminal(webPort, proxy.url, targets[0].workspace, backend.logsDir);
    const pressure = startGatewayReadPressure(backend, targets);
    const replays = [];
    for (const [index, target] of targets.entries()) {
      const targetMarker = marker(
        target.workspaceIndex,
        target.taskIndex,
        backend.config.turnsPerSession - 1,
      );
      replays.push(
        await measureTuiOpen({
          webTerminalUrl: webTerminal.url,
          target,
          targetMarker,
          outDir: backend.runRoot,
          index,
        }),
      );
    }
    const pressureSummary = await pressure.stop();
    const frontendChecks = checkFrontendReplays(replays, proxy);
    const summary = backend.summaryBase({
      ok: frontendChecks.ok,
      owner: "tui",
      frontend: {
        config,
        measuredSessionCount: targets.length,
        replays,
        average: frontendChecks.average,
        assertions: frontendChecks.assertions,
        proxy: proxy.summary(),
        pressure: pressureSummary,
      },
      budget: {
        totalTimeoutMs: backend.config.totalTimeoutMs,
        remainingMs: Math.max(0, backend.stressDeadline - Date.now()),
      },
    });
    await fsp.writeFile(backend.summaryPath, JSON.stringify(summary, null, 2));
    console.log(JSON.stringify(summary, null, 2));
    assert.equal(frontendChecks.ok, true, frontendChecks.message);
    completedOk = true;
  } catch (error) {
    const failureDiagnostics = await backend
      .collectFailureDiagnostics()
      .catch((diagnosticError) => ({
        error: String(diagnosticError?.stack || diagnosticError?.message || diagnosticError),
      }));
    const summary = backend.summaryBase({
      ok: false,
      owner: "tui",
      frontend: {
        config,
        proxy: proxy?.summary(),
      },
      failureDiagnostics,
      error: error instanceof Error ? error.stack || error.message : String(error),
    });
    await fsp
      .writeFile(backend.summaryPath, JSON.stringify(summary, null, 2))
      .catch(() => undefined);
    console.error(JSON.stringify(summary, null, 2));
    process.exitCode = 1;
  } finally {
    await stopProcess(webTerminal?.child);
    await proxy?.close();
    await backend.cleanup();
    if (completedOk) process.exitCode = 0;
  }
}

function selectTargetSessions(backend) {
  const targets = backend.sessions.slice(-config.measuredSessionCount);
  assert.equal(
    targets.length,
    config.measuredSessionCount,
    `TUI full-chain performance requires ${config.measuredSessionCount} target sessions`,
  );
  return targets;
}

function requireTuiArtifacts() {
  const entry = path.join(appRoot, "dist", "index.js");
  if (!fs.existsSync(entry)) {
    throw new Error(`TUI build missing at ${entry}; run npm --prefix apps/tui run build`);
  }
}

async function startWebTerminal(port, gatewayUrl, workspace, logsDir) {
  const child = spawn(process.execPath, [path.join(appRoot, "scripts", "web-terminal.mjs")], {
    cwd: appRoot,
    env: {
      ...process.env,
      PORT: String(port),
      TURA_GATEWAY_URL: gatewayUrl,
      TURA_CWD: workspace,
      FORCE_COLOR: "1",
      TURA_LANG: "en",
    },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  child.stdout.pipe(
    fs.createWriteStream(path.join(logsDir, "tui-web-terminal.stdout.log"), { flags: "a" }),
  );
  child.stderr.pipe(
    fs.createWriteStream(path.join(logsDir, "tui-web-terminal.stderr.log"), { flags: "a" }),
  );
  const url = `http://127.0.0.1:${port}`;
  await waitForHtml(url, child, "Tura TUI", 10_000);
  return { child, url };
}

async function waitForHtml(url, child, text, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    if (child.exitCode !== null)
      throw new Error(`${url} exited before readiness: ${child.exitCode}`);
    try {
      const response = await fetch(url);
      const body = await response.text();
      if (response.ok && body.includes(text)) return;
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  throw lastError || new Error(`timed out waiting for ${url}`);
}

async function startRecordingProxy(upstreamUrl) {
  const port = await freePort();
  const records = [];
  const server = http.createServer(async (req, res) => {
    const startedAt = Date.now();
    const parsed = new URL(req.url || "/", "http://127.0.0.1");
    const record = {
      method: req.method,
      path: parsed.pathname,
      query: parsed.search,
      startedAt,
      status: undefined,
      headerMs: undefined,
      elapsedMs: undefined,
      endedAt: undefined,
      streamed: false,
      error: undefined,
    };
    records.push(record);
    try {
      const body = await requestBody(req);
      const target = new URL(req.url || "/", upstreamUrl);
      const headers = { ...req.headers, host: target.host };
      delete headers.connection;
      delete headers["content-length"];
      const upstream = await fetch(target, {
        method: req.method,
        headers,
        body: body.length && req.method !== "GET" && req.method !== "HEAD" ? body : undefined,
      });
      record.status = upstream.status;
      record.headerMs = Date.now() - startedAt;
      const responseHeaders = responseHeaderObject(upstream.headers);
      responseHeaders["access-control-allow-origin"] ||= "*";
      if (upstream.headers.get("content-type")?.includes("text/event-stream")) {
        record.streamed = true;
        record.endedAt = Date.now();
        record.elapsedMs = record.endedAt - startedAt;
        res.writeHead(upstream.status, responseHeaders);
        if (upstream.body) {
          const stream = Readable.fromWeb(upstream.body);
          stream.on("error", (error) => {
            record.streamError = String(error?.message || error);
            if (!res.destroyed) res.end();
          });
          res.on("error", (error) => {
            record.responseError = String(error?.message || error);
            stream.destroy();
          });
          stream.pipe(res);
        } else {
          res.end();
        }
        return;
      }
      const bytes = Buffer.from(await upstream.arrayBuffer());
      record.endedAt = Date.now();
      record.elapsedMs = record.endedAt - startedAt;
      responseHeaders["content-length"] = String(bytes.length);
      res.writeHead(upstream.status, responseHeaders);
      res.end(bytes);
    } catch (error) {
      record.error = String(error?.stack || error?.message || error);
      record.endedAt = Date.now();
      record.elapsedMs = record.endedAt - startedAt;
      res.writeHead(502, {
        "content-type": "application/json",
        "access-control-allow-origin": "*",
      });
      res.end(JSON.stringify({ error: record.error }));
    }
  });
  await new Promise((resolve, reject) => {
    server.listen(port, "127.0.0.1", resolve);
    server.on("error", reject);
  });
  return {
    url: `http://127.0.0.1:${port}`,
    records,
    messageReads(sessionId) {
      return records.filter(
        (record) => decodeURIComponent(record.path) === `/session/${sessionId}/message`,
      );
    },
    summary() {
      return {
        url: `http://127.0.0.1:${port}`,
        totalRequests: records.length,
        messageReads: records.filter((record) =>
          /\/session\/[^/]+\/message$/u.test(decodeURIComponent(record.path)),
        ).length,
        errors: records.filter((record) => record.error).slice(0, 10),
      };
    },
    close: async () => {
      const closed = new Promise((resolve) => server.close(resolve));
      const result = await Promise.race([closed, delay(1_000).then(() => "timeout")]);
      if (result !== "timeout") return;
      server.closeIdleConnections?.();
      server.closeAllConnections?.();
      await Promise.race([closed, delay(1_000)]);
    },
  };
}

function requestBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => resolve(Buffer.concat(chunks)));
    req.on("error", reject);
  });
}

function responseHeaderObject(headers) {
  const out = {};
  for (const [key, value] of headers.entries()) {
    if (["connection", "keep-alive", "transfer-encoding"].includes(key.toLowerCase())) continue;
    out[key] = value;
  }
  return out;
}

function startGatewayReadPressure(backend, targets) {
  const targetIds = new Set(targets.map((target) => target.sessionId));
  const sessions = backend.sessions.filter((session) => !targetIds.has(session.sessionId));
  let stopped = false;
  let completed = 0;
  const errors = [];
  const samples = [];
  const maxRequests = config.backgroundReadRequests;
  const concurrency = Math.min(config.backgroundReadConcurrency, sessions.length);
  async function worker(offset) {
    let index = offset;
    while (!stopped && completed < maxRequests) {
      const session = sessions[index % sessions.length];
      index += concurrency || 1;
      const started = performance.now();
      try {
        await backend.requestJson(
          backend.gateway.url,
          "GET",
          `/session/${encodeURIComponent(session.sessionId)}/message?limit=200`,
          undefined,
          session.workspace,
          10_000,
          true,
        );
        completed += 1;
        if (samples.length < 20) samples.push(round(performance.now() - started));
      } catch (error) {
        errors.push(String(error?.message || error));
        completed += 1;
      }
    }
  }
  const done =
    concurrency > 0 && maxRequests > 0
      ? Promise.all(Array.from({ length: concurrency }, (_, index) => worker(index)))
      : Promise.resolve();
  return {
    async stop() {
      stopped = true;
      await Promise.race([done, delay(5_000)]);
      return {
        concurrency,
        maxRequests,
        completed,
        sampleMs: samples,
        errors: errors.slice(0, 10),
      };
    },
  };
}

async function measureTuiOpen({ webTerminalUrl, target, targetMarker, outDir, index }) {
  const { chromium } = playwright();
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  const browserErrors = [];
  page.on("pageerror", (error) => browserErrors.push(String(error.stack || error.message)));
  page.on("console", (message) => {
    if (message.type() === "error") browserErrors.push(message.text());
  });
  try {
    await installFrameProbe(page, "tui-open-session");
    const openStartedAt = Date.now();
    const params = new URLSearchParams({
      instance: `full-chain-performance-${index}`,
      sessionId: target.sessionId,
    });
    await page.goto(`${webTerminalUrl}/rich?${params}`, {
      waitUntil: "domcontentloaded",
      timeout: config.openBudgetMs,
    });
    await page.waitForFunction(() => window.__turaTerminal, undefined, {
      timeout: config.openBudgetMs,
    });
    await page.evaluate(() => window.__turaFit?.());
    await page.waitForFunction(
      (source) => {
        const buffer = window.__turaTerminal?.buffer.active;
        if (!buffer) return false;
        for (let index = 0; index < buffer.length; index += 1) {
          const text = buffer.getLine(index)?.translateToString(true) ?? "";
          if (text.includes(source)) return true;
        }
        return false;
      },
      targetMarker,
      { timeout: config.openBudgetMs },
    );
    const markerVisibleAt = Date.now();
    await page.waitForTimeout(100);
    const frame = await stopFrameProbe(page);
    const buffer = await terminalBufferSnapshot(page);
    const screenshotPath = path.join(outDir, `tui-open-session-${index + 1}.png`);
    await page.screenshot({ path: screenshotPath, fullPage: false });
    return {
      sessionId: target.sessionId,
      marker: targetMarker,
      totalOpenMs: markerVisibleAt - openStartedAt,
      markerVisibleAt,
      frame,
      bufferLines: buffer.length,
      textLength: buffer.text.length,
      browserErrors,
      screenshotPath,
    };
  } finally {
    await browser.close();
  }
}

async function terminalBufferSnapshot(page) {
  return page.evaluate(() => {
    const buffer = window.__turaTerminal?.buffer.active;
    if (!buffer) return { length: 0, text: "" };
    const lines = [];
    for (let index = 0; index < buffer.length; index += 1) {
      lines.push(buffer.getLine(index)?.translateToString(true) ?? "");
    }
    return { length: buffer.length, text: lines.join("\n") };
  });
}

async function installFrameProbe(page, label) {
  await page.addInitScript(`
    (() => {
      const probe = {
        label: ${JSON.stringify(label)},
        frames: [],
        running: true,
        startedAt: performance.now(),
        previous: undefined,
        tick(now) {
          if (!this.running) return;
          if (this.previous !== undefined) this.frames.push(Math.max(0, now - this.previous));
          this.previous = now;
          this.raf = requestAnimationFrame((next) => this.tick(next));
        },
        stop() {
          this.running = false;
          if (this.raf) cancelAnimationFrame(this.raf);
          const elapsedMs = Math.max(0, performance.now() - this.startedAt);
          const frames = this.frames || [];
          const sorted = [...frames].sort((a, b) => a - b);
          const avgGapMs = frames.length ? frames.reduce((sum, value) => sum + value, 0) / frames.length : 0;
          const maxFrameGapMs = frames.length ? Math.max(...frames) : 0;
          return {
            label: this.label,
            elapsedMs,
            frameCount: frames.length,
            avgFps: elapsedMs ? frames.length * 1000 / elapsedMs : 0,
            minFps: maxFrameGapMs ? 1000 / maxFrameGapMs : 0,
            avgGapMs,
            p95GapMs: sorted.length ? sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * 0.95) - 1)] : 0,
            maxFrameGapMs,
            longFrameCount: frames.filter((value) => value > 50).length,
          };
        },
      };
      window.__turaFullChainFrameProbe = probe;
      probe.raf = requestAnimationFrame((now) => probe.tick(now));
    })();
  `);
}

async function stopFrameProbe(page) {
  return page.evaluate(() => window.__turaFullChainFrameProbe?.stop?.() ?? {});
}

function checkFrontendReplays(replays, proxy) {
  const failures = [];
  for (const replay of replays) {
    attachReadMetrics(replay, proxy);
    if (!replay.read)
      failures.push(`TUI did not record a session message read for ${replay.sessionId}`);
    if (replay.browserErrors.length > 0)
      failures.push(
        `TUI browser errors for ${replay.sessionId}: ${JSON.stringify(replay.browserErrors)}`,
      );
    if (replay.textLength <= 0 || replay.bufferLines <= 0) {
      failures.push(
        `TUI terminal buffer did not hydrate for ${replay.sessionId}: ${JSON.stringify({ bufferLines: replay.bufferLines, textLength: replay.textLength })}`,
      );
    }
  }
  const average = averageReplayMetrics(replays);
  const assertions = [
    {
      name: "tui-average-open-ms",
      ok: average.totalOpenMs <= config.openBudgetMs,
      actual: average.totalOpenMs,
      budget: config.openBudgetMs,
    },
    {
      name: "tui-average-read-ms",
      ok: average.readMs <= config.readBudgetMs,
      actual: average.readMs,
      budget: config.readBudgetMs,
    },
    {
      name: "tui-average-render-ms",
      ok: average.renderMs <= config.renderBudgetMs,
      actual: average.renderMs,
      budget: config.renderBudgetMs,
    },
    {
      name: "tui-average-fps",
      ok: average.avgFps >= config.minAvgFps,
      actual: average.avgFps,
      budget: config.minAvgFps,
    },
    {
      name: "tui-max-frame-gap-ms",
      ok: average.maxFrameGapMs <= config.maxFrameGapMs,
      actual: average.maxFrameGapMs,
      budget: config.maxFrameGapMs,
    },
    {
      name: "tui-long-frames",
      ok: average.longFrameCount <= config.maxLongFrames,
      actual: average.longFrameCount,
      budget: config.maxLongFrames,
    },
  ];
  failures.push(
    ...assertions
      .filter((item) => !item.ok)
      .map((item) => `${item.name} failed: ${item.actual} outside budget ${item.budget}`),
  );
  return { ok: failures.length === 0, message: failures.join("\n"), average, assertions };
}

function attachReadMetrics(replay, proxy) {
  const readRecord = proxy.messageReads(replay.sessionId).at(-1);
  if (readRecord) {
    replay.read = {
      elapsedMs: readRecord.elapsedMs,
      startedAt: readRecord.startedAt,
      endedAt: readRecord.endedAt,
      status: readRecord.status,
    };
    replay.renderMs = Math.max(0, replay.markerVisibleAt - readRecord.endedAt);
  } else {
    replay.read = null;
    replay.renderMs = null;
  }
}

function averageReplayMetrics(replays) {
  return {
    totalOpenMs: round(mean(replays.map((replay) => replay.totalOpenMs))),
    readMs: round(
      mean(replays.map((replay) => replay.read?.elapsedMs ?? Number.NaN).filter(Number.isFinite)),
    ),
    renderMs: round(
      mean(replays.map((replay) => replay.renderMs ?? Number.NaN).filter(Number.isFinite)),
    ),
    frameCount: round(mean(replays.map((replay) => replay.frame?.frameCount ?? 0))),
    avgFps: round(mean(replays.map((replay) => replay.frame?.avgFps ?? 0))),
    minFps: round(Math.min(...replays.map((replay) => replay.frame?.minFps ?? 0))),
    maxFrameGapMs: round(Math.max(...replays.map((replay) => replay.frame?.maxFrameGapMs ?? 0))),
    longFrameCount: replays.reduce(
      (total, replay) => total + (replay.frame?.longFrameCount ?? 0),
      0,
    ),
  };
}

function mean(values) {
  if (!values.length) return Number.POSITIVE_INFINITY;
  return values.reduce((total, value) => total + value, 0) / values.length;
}

function playwright() {
  return createRequire(path.join(appRoot, "package.json"))("playwright");
}

async function stopProcess(child) {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  child.kill("SIGTERM");
  await Promise.race([new Promise((resolve) => child.once("exit", resolve)), delay(2_000)]);
  if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
}

await main();
