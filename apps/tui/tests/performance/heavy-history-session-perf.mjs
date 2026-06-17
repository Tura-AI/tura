#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { initialState, reducer } from "../../dist/tui/reducer.js";
import { draw, resetDrawState } from "../../dist/tui/draw.js";
import { richCapabilities, plainCapabilities } from "../../dist/tui/capabilities.js";
import { renderChatFrameParts } from "../../dist/tui/render.js";
import { transcriptRenderLines } from "../../dist/tui/render/transcript.js";
import { setActiveCapabilities } from "../../dist/tui/render-terminal.js";

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..", "..");
const runId = process.env.TURA_TUI_HISTORY_PERF_RUN_ID || `heavy-history-${timestamp()}`;
const runRoot = path.join(
  repoRoot,
  "apps",
  "tui",
  "test-results",
  "performance",
  "heavy-history",
  runId,
);
const summaryPath = path.join(runRoot, "summary.json");

const config = {
  cols: intEnv("TURA_TUI_HISTORY_PERF_COLS", 120),
  rows: intEnv("TURA_TUI_HISTORY_PERF_ROWS", 32),
  messageCount: intEnv("TURA_TUI_HISTORY_PERF_MESSAGES", 1000),
  commandCount: intEnv("TURA_TUI_HISTORY_PERF_COMMANDS", 1200),
  textBytes: intEnv("TURA_TUI_HISTORY_PERF_TEXT_BYTES", 900),
  inputChars: intEnv("TURA_TUI_HISTORY_PERF_INPUT_CHARS", 3),
  iterations: intEnv("TURA_TUI_HISTORY_PERF_ITERATIONS", 1),
  modes: (process.env.TURA_TUI_HISTORY_PERF_MODES || "rich")
    .split(",")
    .map((mode) => mode.trim())
    .filter((mode) => mode === "rich" || mode === "plain"),
};

if (!config.modes.length) config.modes.push("rich");
setTerminal(config.rows, config.cols);

let capturedWrites = [];
let capturedBytes = 0;

const session = makeSession(config.messageCount);
const heavyMessages = makeHistoryMessages(
  config.messageCount,
  config.commandCount,
  config.textBytes,
);
const lightMessages = makeHistoryMessages(20, 0, 120);
const heavyBase = hydrateState(heavyMessages);
const lightBase = hydrateState(lightMessages);
const commandParts = heavyMessages.flatMap((message) =>
  message.parts.filter((part) => part.tool === "command_run"),
);

const results = [];
for (const mode of config.modes) {
  const capabilities = mode === "rich" ? richCapabilities() : plainCapabilities();
  results.push({
    mode,
    dataset: {
      messages: heavyMessages.length,
      commandParts: commandParts.length,
      approximateTextBytes: heavyMessages.reduce(
        (sum, message) =>
          sum +
          message.parts.reduce(
            (partSum, part) => partSum + (part.text?.length ?? part.content?.length ?? 0),
            0,
          ),
        0,
      ),
    },
    transcriptHeavy: measureMany(config.iterations, () => {
      setActiveCapabilities(capabilities);
      return transcriptRenderLines(heavyBase, config.cols);
    }),
    renderHeavy: measureMany(config.iterations, () =>
      renderChatFrameParts(heavyBase, capabilities),
    ),
    drawColdHeavy: measureDrawCold(heavyBase, capabilities),
    composerHeavy: measureComposer(heavyBase, capabilities),
    composerLight: measureComposer(lightBase, capabilities),
  });
}

const summary = {
  config,
  runRoot,
  summaryPath,
  generatedAt: new Date().toISOString(),
  node: process.version,
  platform: process.platform,
  results,
  notes: [
    "composerHeavy measures one renderChatFrameParts and one draw per typed character after a heavy history hydrate.",
    "process.stdout.write is intercepted, so draw timings mostly measure TUI render/diff CPU; writeBytes estimates terminal output pressure.",
    "transcriptHeavy isolates transcriptRenderLines for the historical cache path.",
  ],
};

await fs.mkdir(runRoot, { recursive: true });
await fs.writeFile(summaryPath, JSON.stringify(summary, null, 2));
printSummary(summary);

function hydrateState(messages) {
  return reducer(initialState(process.cwd()), {
    type: "hydrate",
    session,
    sessions: [session],
    messages,
    permissions: [],
    closePanels: true,
    sessionConfig: { show_command_instructions: true },
  });
}

function measureComposer(baseState, capabilities) {
  return withCapturedStdout(() => {
    resetDrawState();
    let state = baseState;
    let renderedParts = renderChatFrameParts(state, capabilities);
    let renderedCache = renderedParts.cache;
    let previousFrame = draw(state, capabilities, "", { forceReset: true });
    const chars = inputPayload(config.inputChars);
    const renderSamples = [];
    const drawSamples = [];
    const reducerSamples = [];
    const writeBytes = [];
    const writeCounts = [];

    for (const char of chars) {
      let started = performance.now();
      state = reducer(state, { type: "composer", value: state.composer + char });
      reducerSamples.push(performance.now() - started);

      started = performance.now();
      renderedParts = renderChatFrameParts(state, capabilities, { cache: renderedCache });
      renderedCache = renderedParts.cache;
      renderSamples.push(performance.now() - started);

      const beforeWrites = capturedWrites.length;
      const beforeBytes = capturedBytes;
      started = performance.now();
      previousFrame = draw(state, capabilities, previousFrame);
      drawSamples.push(performance.now() - started);
      writeCounts.push(capturedWrites.length - beforeWrites);
      writeBytes.push(capturedBytes - beforeBytes);
    }

    return {
      chars: chars.length,
      reducerMs: stats(reducerSamples),
      renderMs: stats(renderSamples),
      drawMs: stats(drawSamples),
      writeBytes: stats(writeBytes),
      writeCount: stats(writeCounts),
      finalFrameBytes: previousFrame.length,
    };
  });
}

function measureDrawCold(state, capabilities) {
  return withCapturedStdout(() => {
    resetDrawState();
    const beforeBytes = capturedBytes;
    const beforeWrites = capturedWrites.length;
    const started = performance.now();
    const frame = draw(state, capabilities, "", { forceReset: true });
    return {
      elapsedMs: round(performance.now() - started),
      frameBytes: frame.length,
      writeBytes: capturedBytes - beforeBytes,
      writeCount: capturedWrites.length - beforeWrites,
    };
  });
}

function measureMany(iterations, fn) {
  const samples = [];
  let detail;
  for (let index = 0; index < iterations; index += 1) {
    const started = performance.now();
    detail = fn();
    samples.push(performance.now() - started);
  }
  const lineCount = Array.isArray(detail)
    ? detail.length
    : typeof detail?.frame === "string"
      ? detail.frame.split("\n").length
      : undefined;
  const frameBytes = typeof detail?.frame === "string" ? detail.frame.length : undefined;
  return { ...stats(samples), lineCount, frameBytes };
}

function withCapturedStdout(fn) {
  capturedWrites = [];
  capturedBytes = 0;
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
      const text = typeof chunk === "string" ? chunk : chunk.toString();
      capturedWrites.push(text);
      capturedBytes += Buffer.byteLength(text);
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

function makeSession(messageCount) {
  return {
    id: "perf-heavy-history-session",
    name: "Heavy history performance session",
    parent_id: null,
    created_at: 1,
    updated_at: 2,
    directory: process.cwd(),
    model: "openai/gpt-5",
    agent: "coding",
    session_type: "coding",
    auto_session_name: true,
    kill_processes_on_start: false,
    validator_enabled: false,
    force_planning: false,
    model_variant: null,
    model_acceleration_enabled: false,
    disable_permission_restrictions: false,
    status: "idle",
    message_count: messageCount,
  };
}

function makeHistoryMessages(messageCount, commandCount, textBytes) {
  const messages = [];
  let remainingCommands = commandCount;
  const assistantSlots = Math.max(1, Math.floor(messageCount / 2));
  for (let index = 0; index < messageCount; index += 1) {
    const role = index % 2 === 0 ? "user" : "assistant";
    const messageID = `perf-message-${index}`;
    const parts = [
      textPart(
        messageID,
        `perf-part-${index}-text`,
        role === "user" ? userText(index, textBytes) : assistantText(index, textBytes),
      ),
    ];
    if (role === "assistant" && remainingCommands > 0) {
      const slotsLeft = Math.max(1, assistantSlots - Math.floor(index / 2));
      const commandsForMessage = Math.max(1, Math.ceil(remainingCommands / slotsLeft));
      for (let offset = 0; offset < commandsForMessage && remainingCommands > 0; offset += 1) {
        const commandIndex = commandCount - remainingCommands;
        parts.push(commandPart(messageID, commandIndex));
        remainingCommands -= 1;
      }
    }
    messages.push({
      id: messageID,
      sessionID: session.id,
      parentID: index > 0 ? `perf-message-${index - 1}` : null,
      role,
      parts,
      created_at: 1_700_000_000_000 + index,
      updated_at: 1_700_000_000_000 + index,
      time: {
        created: 1_700_000_000_000 + index,
        updated: 1_700_000_000_000 + index,
      },
    });
  }
  return messages;
}

function textPart(messageID, partID, text) {
  return {
    id: partID,
    sessionID: session.id,
    messageID,
    type: "text",
    text,
    content: text,
  };
}

function commandPart(messageID, index) {
  const command = `node scripts/heavy-task-${index % 17}.mjs --batch ${index} --workspace "${process.cwd()}"`;
  return {
    id: `perf-command-part-${index}`,
    sessionID: session.id,
    messageID,
    type: "tool",
    tool: "command_run",
    state: {
      status: index % 13 === 0 ? "failed" : "completed",
      input: {
        commands: [
          {
            step: index + 1,
            command_type: "shell_command",
            command_line: command,
          },
        ],
      },
      output: {
        streamed_command_run_result: {
          results: [
            {
              step: index + 1,
              command_type: "shell_command",
              command_line: command,
              success: index % 13 !== 0,
              status: index % 13 === 0 ? "failed" : "completed",
              output: `command ${index} output ${"x".repeat(80)}`,
            },
          ],
        },
      },
    },
  };
}

function userText(index, targetBytes) {
  return fillText(
    `用户历史消息 ${index}: 这里是一段比较长的输入，用来模拟关闭后重开读取过去 session 的场景。\n`,
    targetBytes,
  );
}

function assistantText(index, targetBytes) {
  const seed = [
    `Assistant response ${index}`,
    "",
    "```ts",
    `const value${index} = ${index};`,
    "console.log(value);",
    "```",
    "",
    "| file | status | note |",
    "| --- | --- | --- |",
    `| src/${index}.ts | changed | generated history row |`,
    "",
  ].join("\n");
  return fillText(seed, targetBytes);
}

function fillText(seed, targetBytes) {
  let text = seed;
  const tail = " long transcript payload with markdown, paths, numbers, and repeated content.";
  while (text.length < targetBytes) text += tail;
  return text.slice(0, targetBytes);
}

function inputPayload(length) {
  const text = "typing latency probe 输入延迟测试 0123456789 ";
  let value = "";
  while (value.length < length) value += text;
  return Array.from(value.slice(0, length));
}

function stats(samples) {
  const sorted = [...samples].sort((left, right) => left - right);
  const sum = sorted.reduce((total, item) => total + item, 0);
  return {
    min: round(sorted[0] ?? 0),
    p50: round(percentile(sorted, 0.5)),
    p90: round(percentile(sorted, 0.9)),
    p95: round(percentile(sorted, 0.95)),
    max: round(sorted.at(-1) ?? 0),
    avg: round(sum / Math.max(1, sorted.length)),
    samples: sorted.length,
  };
}

function percentile(sorted, ratio) {
  if (!sorted.length) return 0;
  const index = Math.min(sorted.length - 1, Math.ceil(sorted.length * ratio) - 1);
  return sorted[index];
}

function printSummary(summary) {
  console.log(`heavy history perf summary: ${summary.summaryPath}`);
  for (const result of summary.results) {
    const heavy = result.composerHeavy;
    const light = result.composerLight;
    console.log(
      [
        `[${result.mode}]`,
        `transcript=${result.transcriptHeavy.avg}ms`,
        `render=${result.renderHeavy.avg}ms`,
        `coldDraw=${result.drawColdHeavy.elapsedMs}ms`,
        `typeRenderP95=${heavy.renderMs.p95}ms`,
        `typeDrawP95=${heavy.drawMs.p95}ms`,
        `lightTypeDrawP95=${light.drawMs.p95}ms`,
        `writeP95=${Math.round(heavy.writeBytes.p95)}B`,
      ].join(" "),
    );
  }
}

function intEnv(key, fallback) {
  const parsed = Number.parseInt(process.env[key] ?? "", 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function setTerminal(rows, cols) {
  Object.defineProperty(process.stdout, "rows", { configurable: true, value: rows });
  Object.defineProperty(process.stdout, "columns", { configurable: true, value: cols });
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
