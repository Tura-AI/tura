#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { initialState, reducer } from "../../../apps/tui/dist/tui/reducer.js";
import { renderFrame } from "../../../apps/tui/dist/tui/render.js";
import {
  plainCapabilities,
  richCapabilities,
} from "../../../apps/tui/dist/tui/capabilities.js";

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..");
const cwd = process.cwd();
const runId =
  process.env.TURA_TUI_STRESS_RUN_ID || `memory-stress-${timestamp()}`;
const runRoot = path.join(
  repoRoot,
  "apps",
  "tui",
  "test-results",
  "benchmark",
  "memory",
  runId,
);
const summaryPath = path.join(runRoot, "summary.json");
const sessionID = "stress-session";
const messageID = "stress-assistant-message";
const partID = "stress-assistant-part";

const config = {
  rows: intEnv("TURA_TUI_STRESS_ROWS", 32),
  cols: intEnv("TURA_TUI_STRESS_COLS", 120),
  historyMessages: intEnv("TURA_TUI_STRESS_HISTORY_MESSAGES", 80),
  historyBytes: intEnv("TURA_TUI_STRESS_HISTORY_BYTES", 256),
  streamChunks: intEnv("TURA_TUI_STRESS_CHUNKS", 300),
  streamChunkBytes: intEnv("TURA_TUI_STRESS_CHUNK_BYTES", 80),
  hydrateIterations: intEnv("TURA_TUI_STRESS_HYDRATE_ITERATIONS", 40),
  refreshIterations: intEnv("TURA_TUI_STRESS_REFRESH_ITERATIONS", 40),
  refreshMessages: intEnv("TURA_TUI_STRESS_REFRESH_MESSAGES", 250),
  refreshPartBytes: intEnv("TURA_TUI_STRESS_REFRESH_PART_BYTES", 512),
  renderMode:
    process.env.TURA_TUI_STRESS_RENDER_MODE === "rich" ? "rich" : "plain",
};

setTerminalSize(config.rows, config.cols);

const capabilities =
  config.renderMode === "rich" ? richCapabilities() : plainCapabilities();

const results = [];
results.push(
  await measure("stream-delta-render", (sample) => {
    let state = baseState(config.historyMessages, config.historyBytes);
    renderFrame(state, capabilities);
    for (let index = 0; index < config.streamChunks; index += 1) {
      state = reducer(state, {
        type: "event",
        event: {
          directory: cwd,
          payload: {
            type: "message.part.delta",
            properties: {
              sessionID,
              message_id: messageID,
              part_id: partID,
              field: "text",
              delta: chunkText(index, config.streamChunkBytes),
            },
          },
        },
      });
      renderFrame(state, capabilities);
      if (index % 25 === 0) sample();
    }
    return {
      messages: state.messages.length,
      streamedBytes: config.streamChunks * config.streamChunkBytes,
      finalFrameBytes: renderFrame(state, capabilities).frame.length,
    };
  }),
);

results.push(
  await measure("full-hydrate-snapshots", (sample) => {
    let state = baseState(1, 32);
    for (let index = 0; index < config.hydrateIterations; index += 1) {
      const messages = snapshotMessages(
        config.refreshMessages,
        config.refreshPartBytes,
        index,
      );
      state = reducer(state, {
        type: "hydrate",
        session: stressSession("busy"),
        messages,
        permissions: [],
        sessions: [stressSession("busy")],
      });
      if (index % 5 === 0) sample();
    }
    return {
      messages: state.messages.length,
      snapshotBytes:
        config.hydrateIterations *
        config.refreshMessages *
        config.refreshPartBytes,
    };
  }),
);

results.push(
  await measure("refresh-signature-compare", (sample) => {
    let current = snapshotMessages(
      config.refreshMessages,
      config.refreshPartBytes,
      0,
    );
    let equalWindows = 0;
    for (let index = 0; index < config.refreshIterations; index += 1) {
      const incoming = snapshotMessages(
        config.refreshMessages,
        config.refreshPartBytes,
        index,
      );
      if (sameMessageWindow(incoming, current)) equalWindows += 1;
      current = incoming;
      if (index % 5 === 0) sample();
    }
    return {
      messages: current.length,
      equalWindows,
      comparedBytes:
        config.refreshIterations *
        config.refreshMessages *
        config.refreshPartBytes,
    };
  }),
);

const summary = { config, results, runRoot, summaryPath };
await fs.mkdir(runRoot, { recursive: true });
await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
console.log(JSON.stringify(summary, null, 2));

function baseState(messageCount, bytesPerMessage) {
  let state = initialState(cwd);
  state = reducer(state, {
    type: "hydrate",
    session: stressSession("busy"),
    messages: snapshotMessages(messageCount, bytesPerMessage, 0),
    permissions: [],
    sessions: [stressSession("busy")],
  });
  return state;
}

function stressSession(status) {
  return {
    id: sessionID,
    name: "TUI memory stress",
    directory: cwd,
    status,
    updated_at: Date.now(),
    message_count: config.refreshMessages,
  };
}

function snapshotMessages(count, bytesPerMessage, version) {
  const messages = [];
  for (let index = 0; index < count; index += 1) {
    const role = index % 3 === 0 ? "user" : "assistant";
    messages.push({
      id: `message-${index}`,
      sessionID,
      role,
      created_at: index + 1,
      updated_at: version,
      parts: [
        {
          id: `part-${index}`,
          sessionID,
          messageID: `message-${index}`,
          type: "text",
          text: payloadText(index, bytesPerMessage, version),
        },
      ],
    });
  }
  return messages;
}

function sameMessageWindow(left, right) {
  if (left.length !== right.length) return false;
  for (const [index, message] of left.entries()) {
    if (messageSignature(message) !== messageSignature(right[index]))
      return false;
  }
  return true;
}

function messageSignature(message) {
  if (!message) return "";
  return JSON.stringify({
    id: message.id,
    created_at: message.created_at ?? message.time?.created,
    updated_at: message.updated_at ?? message.time?.updated,
    parts: message.parts.map((part) => ({
      id: part.id,
      type: part.type,
      text: part.text,
      content: part.content,
      tool: part.tool,
      state: part.state,
    })),
  });
}

async function measure(name, run) {
  forceGc();
  const before = memorySnapshot();
  let peak = before;
  const sample = () => {
    const current = memorySnapshot();
    if (current.rssMb > peak.rssMb) peak = current;
  };
  const started = performance.now();
  const detail = run(sample);
  sample();
  forceGc();
  const after = memorySnapshot();
  return {
    name,
    elapsedMs: Math.round(performance.now() - started),
    before,
    peak,
    after,
    retainedHeapMb: round(after.heapUsedMb - before.heapUsedMb),
    detail,
  };
}

function memorySnapshot() {
  const memory = process.memoryUsage();
  return {
    rssMb: bytesToMb(memory.rss),
    heapUsedMb: bytesToMb(memory.heapUsed),
    heapTotalMb: bytesToMb(memory.heapTotal),
    externalMb: bytesToMb(memory.external),
  };
}

function forceGc() {
  if (global.gc) {
    global.gc();
    global.gc();
  }
}

function payloadText(index, bytes, version) {
  const prefix = `message ${index} version ${version}: `;
  return (prefix + "x".repeat(Math.max(0, bytes - prefix.length))).slice(
    0,
    bytes,
  );
}

function chunkText(index, bytes) {
  const prefix = `chunk ${index} `;
  return (prefix + "y".repeat(Math.max(0, bytes - prefix.length))).slice(
    0,
    bytes,
  );
}

function bytesToMb(bytes) {
  return round(bytes / 1024 / 1024);
}

function round(value) {
  return Math.round(value * 100) / 100;
}

function intEnv(name, fallback) {
  const value = Number.parseInt(process.env[name] ?? "", 10);
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function timestamp() {
  return new Date()
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\..+$/u, "")
    .replace("T", "-");
}

function setTerminalSize(rows, columns) {
  for (const [key, value] of [
    ["rows", rows],
    ["columns", columns],
  ]) {
    try {
      Object.defineProperty(process.stdout, key, {
        configurable: true,
        value,
      });
    } catch {
      process.stdout[key] = value;
    }
  }
}
