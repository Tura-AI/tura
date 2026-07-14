import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { render, renderFrame } from "../../../src/tui/render.js";
import { richCapabilities } from "../../../src/tui/capabilities.js";
import { stripAnsi, textSecondary } from "../../../src/tui/render-terminal.js";
import { providerEnums, withTerminalSize, assertLineWidths } from "./helpers/render-harness.js";

process.env.TURA_LANG = "en";

test("render places composer at the bottom and reports its terminal cursor", () => {
  const session = { id: "sess-bottom-input", title: "Bottom Input", status: "idle" as const };
  const state = reducer(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session,
      messages: [
        {
          id: "msg-bottom-input",
          sessionID: "sess-bottom-input",
          role: "assistant",
          parts: [{ id: "part-bottom-input", type: "text", text: "Ready." }],
        },
      ],
      permissions: [],
      providers: { all: [], default: {}, connected: [], enums: providerEnums },
      sessions: [session],
    }),
    { type: "composer", value: "hello" },
  );

  const rendered = withTerminalSize(80, 18, () => renderFrame(state, richCapabilities()));
  assertLineWidths(rendered.frame, 80);
  const lines = rendered.frame.split("\n");
  const composerIndex = lines.findIndex((line) => stripAnsi(line).includes("> hello"));
  const metaIndex = lines.findIndex((line) => stripAnsi(line).trim() === "tura");
  assert.ok(metaIndex >= 0);
  assert.ok(composerIndex > metaIndex, "composer should be below the meta/status line");
  assert.equal(composerIndex, lines.length - 2, "composer body should sit at the bottom edge");
  assert.deepEqual(rendered.cursor, { row: composerIndex + 1, column: 10 });
  assert.doesNotMatch(rendered.frame, /TURA_COMPOSER_CURSOR/);
});

test("render reports the composer cursor on the final visible input line", () => {
  const session = { id: "sess-full-composer", title: "Full Composer", status: "idle" as const };
  const composer = Array.from({ length: 9 }, (_item, index) => `input-line-${index + 1}`).join(
    "\n",
  );
  const state = reducer(
    reducer(initialState("C:/repo"), {
      type: "hydrate",
      session,
      messages: [],
      permissions: [],
      providers: { all: [], default: {}, connected: [], enums: providerEnums },
      sessions: [session],
    }),
    { type: "composer", value: composer },
  );

  const rendered = withTerminalSize(72, 10, () => renderFrame(state, richCapabilities()));
  const plain = stripAnsi(rendered.frame);
  assert.match(plain, /input-line-1/u);
  assert.match(plain, /input-line-9/u);
  assert.equal(rendered.cursor?.row, rendered.frame.split("\n").length - 1);
});

test("render keeps long multiline user text instead of truncating to the first line", () => {
  const session = { id: "sess-long-user", title: "Long User", status: "idle" as const };
  const tail = "user tail must stay visible";
  const text = `${"first paragraph text ".repeat(12)}\nsecond line continues ${tail}`;
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-long-user",
        sessionID: session.id,
        role: "user",
        parts: [{ id: "part-long-user", type: "text", text }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const output = withTerminalSize(72, 30, () => render(state, richCapabilities()));
  assert.match(stripAnsi(output), new RegExp(tail, "u"));
  const secondLine = output
    .split("\n")
    .find((line) => stripAnsi(line).includes("second line continues"));
  assert.ok(secondLine);
  assert.ok(
    secondLine.includes(textSecondary),
    `second user line should keep secondary color: ${JSON.stringify(secondLine)}`,
  );
  assert.doesNotMatch(secondLine, /\x1b\[38;2;244;247;235msecond line/u);
});

// ─── Transcript viewport ────────────────────────────────────────────────────

test("transcript default view shows bottom content", () => {
  const session = { id: "sess-scroll-default", title: "Scroll Default", status: "idle" as const };
  const messages = Array.from({ length: 6 }, (_, i) => ({
    id: `msg-scroll-${i}`,
    sessionID: "sess-scroll-default",
    role: i % 2 === 0 ? ("user" as const) : ("assistant" as const),
    parts: [{ id: `part-scroll-${i}`, type: "text", text: `Message ${i + 1}` }],
  }));
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages,
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });
  const output = withTerminalSize(80, 18, () => stripAnsi(render(state, richCapabilities())));
  assert.match(output, /Message 6/, "latest message must be visible by default");
});

test("transcript render keeps full history and leaves viewport ownership to the terminal", () => {
  const session = { id: "sess-full-history", title: "Full History", status: "idle" as const };
  const messages = Array.from({ length: 130 }, (_, i) => ({
    id: `msg-full-${i}`,
    sessionID: "sess-full-history",
    role: i % 2 === 0 ? ("user" as const) : ("assistant" as const),
    parts: [
      {
        id: `part-full-${i}`,
        type: "text",
        text: `History marker ${String(i + 1).padStart(3, "0")}`,
      },
    ],
  }));
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages,
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });
  const output = withTerminalSize(80, 18, () => render(state, richCapabilities()));
  const plain = stripAnsi(output);
  assertLineWidths(output, 80);
  assert.match(plain, /History marker 001/);
  assert.match(plain, /History marker 130/);
  assert.ok(
    plain.indexOf("History marker 001") < plain.indexOf("History marker 130"),
    "history should remain in event order",
  );
  assert.ok(
    output.split("\n").length > 18,
    "render must not trim transcript history to the current terminal viewport",
  );
});

test("transcript render preserves terminal-owned history without app scroll state", () => {
  const session = { id: "sess-full-scroll", title: "Full Scroll", status: "idle" as const };
  const messages = Array.from({ length: 80 }, (_, index) => ({
    id: `msg-scroll-${index}`,
    sessionID: session.id,
    role: index % 2 === 0 ? ("user" as const) : ("assistant" as const),
    parts: [
      {
        id: `part-scroll-${index}`,
        type: "text",
        text: `Full scroll marker ${String(index + 1).padStart(2, "0")}`,
      },
    ],
  }));
  const state = {
    ...initialState("C:/repo"),
    session,
    sessions: [session],
    messages,
  };

  const output = withTerminalSize(80, 18, () => stripAnsi(render(state, richCapabilities())));

  assert.match(output, /Full scroll marker 01/);
  assert.match(output, /Full scroll marker 80/);
  assert.ok(
    output.indexOf("Full scroll marker 01") < output.indexOf("Full scroll marker 80"),
    "transcript history must stay in terminal-owned event order",
  );
});

// ─── Differential rendering (no full-screen clear in frame string) ─────────

test("renderFrame does not embed a screen-clear escape sequence", () => {
  const session = { id: "sess-no-clear", title: "No Clear", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-nc-1",
        sessionID: "sess-no-clear",
        role: "user" as const,
        parts: [{ id: "p-nc-1", type: "text", text: "test" }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });
  const { frame } = withTerminalSize(80, 20, () => renderFrame(state, richCapabilities()));
  // The frame must NOT start with or contain the full-screen-clear sequence
  // (\x1b[3J\x1b[2J\x1b[H). The draw() function handles the initial clear
  // separately; embedding it inside the frame string would cause every
  // differential repaint to flash.
  assert.doesNotMatch(
    frame,
    /\x1b\[2J/,
    "frame must not contain full-screen clear (causes flicker)",
  );
  assert.doesNotMatch(frame, /\x1b\[3J/, "frame must not contain scrollback-clear");
});
