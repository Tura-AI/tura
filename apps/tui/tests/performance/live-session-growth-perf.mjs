#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { richCapabilities } from "../../dist/tui/capabilities.js";
import { draw, resetDrawState } from "../../dist/tui/draw.js";
import { initialState, reducer } from "../../dist/tui/reducer.js";

const appRoot = path.resolve(import.meta.dirname, "..", "..");
const runId = process.env.TURA_TUI_LIVE_GROWTH_RUN_ID || `live-growth-${timestamp()}`;
const runRoot = path.join(appRoot, "test-results", "performance", "live-growth", runId);
const summaryPath = path.join(runRoot, "summary.json");
const config = {
  cols: intEnv("TURA_TUI_LIVE_GROWTH_COLS", 120),
  rows: intEnv("TURA_TUI_LIVE_GROWTH_ROWS", 32),
  messageCount: intEnv("TURA_TUI_LIVE_GROWTH_MESSAGES", 500),
  textBytes: intEnv("TURA_TUI_LIVE_GROWTH_TEXT_BYTES", 240),
  steadyFrames: intEnv("TURA_TUI_LIVE_GROWTH_STEADY_FRAMES", 30),
  frameBudgetMs: numberEnv("TURA_TUI_LIVE_GROWTH_FRAME_BUDGET_MS", 1000 / 30),
};
const session = makeSession("busy");
const capabilities = richCapabilities();
let terminalWriteBytes = 0;

const result = withCapturedTerminal(() => {
  const live = measureLiveGrowth();
  const resumed = measureResumedSession();
  const liveP95 = live.steady.drawMs.p95;
  const resumedP95 = resumed.steady.drawMs.p95;
  const improvementRatio = round(liveP95 / Math.max(0.01, resumedP95));
  return {
    live,
    resumed,
    comparison: {
      liveSteadyDrawP95Ms: liveP95,
      resumedSteadyDrawP95Ms: resumedP95,
      reentryImprovementRatio: improvementRatio,
      frameBudgetMs: round(config.frameBudgetMs),
      liveMissesFrameBudget: liveP95 > config.frameBudgetMs,
      reproduced: liveP95 > config.frameBudgetMs && improvementRatio >= 2,
    },
  };
});

const summary = {
  config,
  generatedAt: new Date().toISOString(),
  node: process.version,
  platform: process.platform,
  summaryPath,
  ...result,
  notes: [
    "The live phase adds independent message.part.delta streams to one session and draws after every event.",
    "The resumed phase hydrates the same 500 messages as stable history, modeling leaving and reopening the session.",
    "Steady probes advance the busy animation without changing transcript content, isolating retained live-state rendering cost.",
    "Terminal writes are intercepted; writeBytes estimates xterm/terminal scroll and repaint pressure without measuring terminal paint time.",
  ],
};

await fs.mkdir(runRoot, { recursive: true });
await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
printSummary(summary);

function measureLiveGrowth() {
  resetDrawState();
  let state = hydrate([], session);
  let previousFrame = draw(state, capabilities, "", { forceReset: true });
  const reducerSamples = [];
  const drawSamples = [];
  const writeBytes = [];
  const checkpoints = [];
  const checkpointCounts = new Set(
    [1, 50, 100, 250, config.messageCount].filter((count) => count <= config.messageCount),
  );

  for (let index = 0; index < config.messageCount; index += 1) {
    let started = performance.now();
    state = reducer(state, liveDeltaAction(index));
    reducerSamples.push(performance.now() - started);

    const beforeBytes = terminalWriteBytes;
    started = performance.now();
    previousFrame = draw(state, capabilities, previousFrame);
    drawSamples.push(performance.now() - started);
    writeBytes.push(terminalWriteBytes - beforeBytes);

    const count = index + 1;
    if (checkpointCounts.has(count)) {
      checkpoints.push({
        messages: count,
        liveStreams: Object.keys(state.liveStreams).length,
        drawMs: round(drawSamples.at(-1) ?? 0),
        reducerMs: round(reducerSamples.at(-1) ?? 0),
        writeBytes: writeBytes.at(-1) ?? 0,
        frameBytes: Buffer.byteLength(previousFrame),
        heapUsedBytes: process.memoryUsage().heapUsed,
      });
    }
  }

  return {
    checkpoints,
    growth: {
      reducerMs: stats(reducerSamples),
      drawMs: stats(drawSamples),
      writeBytes: stats(writeBytes),
      firstWindowDrawMs: stats(drawSamples.slice(0, Math.min(30, drawSamples.length))),
      lastWindowDrawMs: stats(drawSamples.slice(-Math.min(30, drawSamples.length))),
    },
    steady: measureSteadyFrames(state, previousFrame),
    finalLiveStreams: Object.keys(state.liveStreams).length,
  };
}

function measureResumedSession() {
  resetDrawState();
  const started = performance.now();
  const state = hydrate(stableMessages(), session);
  const hydrateMs = performance.now() - started;
  const beforeBytes = terminalWriteBytes;
  const drawStarted = performance.now();
  const previousFrame = draw(state, capabilities, "", { forceReset: true });
  const coldDrawMs = performance.now() - drawStarted;
  return {
    hydrateMs: round(hydrateMs),
    coldDrawMs: round(coldDrawMs),
    coldWriteBytes: terminalWriteBytes - beforeBytes,
    frameBytes: Buffer.byteLength(previousFrame),
    liveStreams: Object.keys(state.liveStreams).length,
    steady: measureSteadyFrames(state, previousFrame),
  };
}

function measureSteadyFrames(baseState, baseFrame) {
  let state = baseState;
  let previousFrame = baseFrame;
  const reducerSamples = [];
  const drawSamples = [];
  const writeBytes = [];
  for (let index = 0; index < config.steadyFrames; index += 1) {
    let started = performance.now();
    state = reducer(state, { type: "tick" });
    reducerSamples.push(performance.now() - started);
    const beforeBytes = terminalWriteBytes;
    started = performance.now();
    previousFrame = draw(state, capabilities, previousFrame);
    drawSamples.push(performance.now() - started);
    writeBytes.push(terminalWriteBytes - beforeBytes);
  }
  return {
    frames: config.steadyFrames,
    reducerMs: stats(reducerSamples),
    drawMs: stats(drawSamples),
    writeBytes: stats(writeBytes),
    finalFrameBytes: Buffer.byteLength(previousFrame),
  };
}

function hydrate(messages, activeSession) {
  return reducer(initialState(process.cwd()), {
    type: "hydrate",
    session: activeSession,
    sessions: [activeSession],
    messages,
    permissions: [],
    closePanels: true,
    sessionConfig: { show_command_instructions: true },
  });
}

function liveDeltaAction(index) {
  const messageID = `live-growth-message-${index}`;
  const now = 1_700_000_000_000 + index;
  return {
    type: "event",
    event: {
      directory: process.cwd(),
      payload: {
        type: "message.part.delta",
        properties: {
          sessionID: session.id,
          messageID,
          partID: `${messageID}-part`,
          createdAt: now,
          updatedAt: now,
          field: "text",
          delta: messageText(index),
        },
      },
    },
  };
}

function stableMessages() {
  return Array.from({ length: config.messageCount }, (_, index) => {
    const messageID = `live-growth-message-${index}`;
    const now = 1_700_000_000_000 + index;
    return {
      id: messageID,
      sessionID: session.id,
      role: "assistant",
      parts: [
        {
          id: `${messageID}-part`,
          sessionID: session.id,
          messageID,
          type: "text",
          text: messageText(index),
        },
      ],
      created_at: now,
      updated_at: now,
      time: { created: now, updated: now },
    };
  });
}

function messageText(index) {
  const seed = `Live response ${index + 1}: streaming output retained in the active session. `;
  return `${seed}${"x".repeat(Math.max(0, config.textBytes - seed.length))}`;
}

function makeSession(status) {
  return {
    id: "perf-live-growth-session",
    name: "Live growth performance session",
    directory: process.cwd(),
    created_at: 1,
    updated_at: 2,
    status,
    message_count: config.messageCount,
  };
}

function withCapturedTerminal(fn) {
  const descriptors = {
    isTTY: Object.getOwnPropertyDescriptor(process.stdout, "isTTY"),
    columns: Object.getOwnPropertyDescriptor(process.stdout, "columns"),
    rows: Object.getOwnPropertyDescriptor(process.stdout, "rows"),
    write: Object.getOwnPropertyDescriptor(process.stdout, "write"),
  };
  Object.defineProperty(process.stdout, "isTTY", { configurable: true, value: true });
  Object.defineProperty(process.stdout, "columns", { configurable: true, value: config.cols });
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: config.rows });
  Object.defineProperty(process.stdout, "write", {
    configurable: true,
    value: (chunk) => {
      terminalWriteBytes += Buffer.byteLength(typeof chunk === "string" ? chunk : chunk.toString());
      return true;
    },
  });
  try {
    return fn();
  } finally {
    resetDrawState();
    restoreProperty(process.stdout, "isTTY", descriptors.isTTY);
    restoreProperty(process.stdout, "columns", descriptors.columns);
    restoreProperty(process.stdout, "rows", descriptors.rows);
    restoreProperty(process.stdout, "write", descriptors.write);
  }
}

function stats(samples) {
  const sorted = [...samples].sort((left, right) => left - right);
  const total = sorted.reduce((sum, value) => sum + value, 0);
  return {
    min: round(sorted[0] ?? 0),
    p50: round(percentile(sorted, 0.5)),
    p90: round(percentile(sorted, 0.9)),
    p95: round(percentile(sorted, 0.95)),
    max: round(sorted.at(-1) ?? 0),
    avg: round(total / Math.max(1, sorted.length)),
    samples: sorted.length,
  };
}

function percentile(sorted, ratio) {
  if (!sorted.length) return 0;
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * ratio) - 1)];
}

function printSummary(value) {
  const live = value.live;
  const resumed = value.resumed;
  const comparison = value.comparison;
  console.log(`live session growth perf summary: ${value.summaryPath}`);
  for (const checkpoint of live.checkpoints) {
    console.log(
      `[live ${checkpoint.messages}] streams=${checkpoint.liveStreams} draw=${checkpoint.drawMs}ms write=${checkpoint.writeBytes}B frame=${checkpoint.frameBytes}B`,
    );
  }
  console.log(
    `[steady live] drawP95=${live.steady.drawMs.p95}ms writeP95=${Math.round(live.steady.writeBytes.p95)}B`,
  );
  console.log(
    `[after reentry] coldDraw=${resumed.coldDrawMs}ms steadyDrawP95=${resumed.steady.drawMs.p95}ms writeP95=${Math.round(resumed.steady.writeBytes.p95)}B`,
  );
  console.log(
    `[result] reproduced=${comparison.reproduced} frameBudget=${comparison.frameBudgetMs}ms reentryImprovement=${comparison.reentryImprovementRatio}x`,
  );
}

function intEnv(key, fallback) {
  const value = Number.parseInt(process.env[key] ?? "", 10);
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function numberEnv(key, fallback) {
  const value = Number.parseFloat(process.env[key] ?? "");
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function restoreProperty(target, key, descriptor) {
  if (descriptor) Object.defineProperty(target, key, descriptor);
  else Reflect.deleteProperty(target, key);
}

function round(value) {
  return Math.round(value * 100) / 100;
}

function timestamp() {
  return new Date().toISOString().replace(/[:.]/g, "-");
}
