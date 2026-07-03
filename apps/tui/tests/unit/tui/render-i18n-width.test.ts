import assert from "node:assert/strict";
import test from "node:test";
import { assertDictionaryParity, setLanguage, t } from "../../../src/i18n.js";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { render } from "../../../src/tui/render.js";
import { richCapabilities } from "../../../src/tui/capabilities.js";
import {
  stripAnsi,
  reset,
  textAgentRich,
  textAuxiliary,
  textBackground,
  textPrimary,
  textSecondary,
  truncate,
  truncateAnsi,
  visibleTextWidth,
  wrap,
  wrapAnsi,
} from "../../../src/tui/render-terminal.js";
import {
  providerEnums,
  withTerminalSize,
  assertFitsTerminal,
  assertLineWidths,
} from "./helpers/render-harness.js";

process.env.TURA_LANG = "en";

test("TUI i18n dictionaries keep zh-CN and en keys in sync", () => {
  assert.doesNotThrow(() => assertDictionaryParity());
});

test("TUI language selection reads external locale files", () => {
  setLanguage("zh-CN");
  assert.equal(t("assistant"), "助手");
  assert.equal(t("runtimeStopped"), "Runtime 已停止。");
  setLanguage("en");
  assert.equal(t("assistant"), "assistant");
  assert.equal(t("runtimeStopped"), "Runtime stopped.");
  setLanguage(undefined);
});

test("terminal width helpers count CJK and emoji as double-width", () => {
  assert.equal(visibleTextWidth("空闲"), 4);
  assert.equal(visibleTextWidth("𠀀𠀁𠀂"), 6);
  assert.equal(visibleTextWidth("ok👍"), 4);
  assert.equal(visibleTextWidth("🇨🇳"), 2);
  assert.equal(visibleTextWidth("👨‍💻"), 2);
  assert.equal(visibleTextWidth("a\u0301"), 1);
  assert.equal(truncate("空闲 ready", 8), "空闲 ...");
  assert.equal(stripAnsi(truncateAnsi("\x1b[90m空闲 ready\x1b[0m", 8)), "空闲 ...");
  assert.deepEqual(wrap("空闲".repeat(12), 22), [
    "空闲".repeat(5),
    "空闲".repeat(5),
    "空闲".repeat(2),
  ]);
  assert.deepEqual(wrap("𠀀".repeat(12), 22), ["𠀀".repeat(10), "𠀀".repeat(2)]);
});

test("render localizes structured runtime stopped assistant messages", () => {
  const session = { id: "sess-runtime-stopped", title: "Runtime", status: "idle" as const };
  setLanguage("zh-CN");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-runtime-stopped",
        sessionID: "sess-runtime-stopped",
        role: "assistant",
        parts: [
          {
            id: "part-runtime-stopped",
            type: "text",
            text: "MANO failed while processing this prompt: router execution enqueue failed: runtime worker invocation failed: one-shot worker cancelled",
            metadata: { kind: "runtime_status", code: "runtime_stopped" },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const output = stripAnsi(withTerminalSize(72, 24, () => render(state, richCapabilities())));
  assert.match(output, /Runtime 已停止。/);
  assert.doesNotMatch(output, /MANO failed|one-shot worker cancelled/);
  setLanguage(undefined);
});

test("terminal semantic text colors expose five intensity levels", () => {
  assert.equal(textPrimary, "\x1b[38;2;244;247;235m");
  assert.equal(textAgentRich, "\x1b[38;2;217;222;205m");
  assert.equal(textSecondary, "\x1b[38;2;151;160;153m");
  assert.equal(textAuxiliary, "\x1b[38;2;103;116;111m");
  assert.equal(textBackground, "\x1b[38;2;54;63;61m");
});

test("wrapAnsi preserves dimmed CJK color across wrapped lines", () => {
  const lines = wrapAnsi(`${textSecondary}${"中文滚动".repeat(8)}${textSecondary}尾部${reset}`, 20);
  assert.ok(lines.length > 2);
  for (const line of lines.slice(1, -1)) {
    assert.ok(
      line.startsWith(textSecondary),
      `wrapped Chinese line should keep secondary color: ${JSON.stringify(line)}`,
    );
  }
});

test("render wraps long CJK assistant lines without terminal-spawned blank rows", () => {
  const session = { id: "sess-cjk-width", title: "CJK Width", status: "idle" as const };
  const text = "𠀀𠀁𠀂𠀃𠀄𠀅𠀆𠀇𠀈𠀉".repeat(4);
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-cjk-width",
        sessionID: "sess-cjk-width",
        role: "assistant",
        parts: [{ id: "part-cjk-width", type: "text", text }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const output = withTerminalSize(48, 24, () => render(state, richCapabilities()));
  assertFitsTerminal(output, 48, 24);
  const contentLines = output
    .split("\n")
    .filter((line) => stripAnsi(line).includes("𠀀") || stripAnsi(line).includes("𠀁"));
  assert.ok(contentLines.length > 1);
  for (const line of contentLines) assert.ok(visibleTextWidth(line) < 48);
});

test("render keeps assistant message panel right margin tight", () => {
  const session = { id: "sess-tight-panel", title: "Tight Panel", status: "idle" as const };
  const text = "x".repeat(44);
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-tight-panel",
        sessionID: "sess-tight-panel",
        role: "assistant",
        parts: [{ id: "part-tight-panel", type: "text", text }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const output = withTerminalSize(48, 24, () => render(state, richCapabilities()));
  const contentLines = output.split("\n").filter((line) => stripAnsi(line).includes("xxxxx"));

  assert.equal(contentLines.length, 1);
  assert.match(stripAnsi(contentLines[0] ?? ""), new RegExp(text));
  assertLineWidths(output, 48);
});
