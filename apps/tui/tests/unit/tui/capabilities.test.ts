import assert from "node:assert/strict";
import test from "node:test";
import {
  ansiCapabilities,
  detectTerminalCapabilities,
  plainCapabilities,
  richCapabilities,
} from "../../../src/tui/capabilities.js";

test("capability presets keep the documented L1/L2/L3 contract", () => {
  assert.deepEqual(plainCapabilities(), {
    level: "plain",
    color: "none",
    cursorControl: false,
    unicode: false,
    osc8: false,
    richText: "none",
    mediaOpen: false,
    interactive: false,
  });
  assert.equal(ansiCapabilities().level, "ansi");
  assert.equal(ansiCapabilities().color, "ansi");
  assert.equal(ansiCapabilities().interactive, true);
  assert.equal(richCapabilities().level, "rich");
  assert.equal(richCapabilities().color, "truecolor");
  assert.equal(richCapabilities().mediaOpen, true);
});

test("explicit display flags override terminal auto detection", () => {
  withEnv({ CI: "1", TERM: "dumb", TERM_PROGRAM: "" }, () => {
    withTty(true, () => {
      assert.equal(detectTerminalCapabilities("plain").level, "plain");
      assert.equal(detectTerminalCapabilities("rich").level, "rich");
    });
  });
});

test("auto detection falls back to plain for CI, non-tty, dumb, and unknown terminals", () => {
  withEnv({ CI: "1", TERM: "xterm-256color", TERM_PROGRAM: "vscode" }, () => {
    withTty(true, () => assert.equal(detectTerminalCapabilities("auto").level, "plain"));
  });
  withEnv({ CI: undefined, TERM: "xterm-256color", TERM_PROGRAM: "vscode" }, () => {
    withTty(false, () => assert.equal(detectTerminalCapabilities("auto").level, "plain"));
  });
  for (const term of ["dumb", "unknown"]) {
    withEnv({ CI: undefined, TERM: term, TERM_PROGRAM: "vscode" }, () => {
      withTty(true, () => assert.equal(detectTerminalCapabilities("auto").level, "plain"));
    });
  }
});

test("auto detection treats modern terminal user-agent signals as rich", () => {
  const cases: Array<Record<string, string | undefined>> = [
    { TERM: "vt100", TERM_PROGRAM: "WezTerm" },
    { TERM: "vt100", TERM_PROGRAM: "Apple_Terminal", WEZTERM_EXECUTABLE: "wezterm" },
    { TERM: "vt100", TERM_PROGRAM: "", KITTY_WINDOW_ID: "7" },
    { TERM: "vt100", TERM_PROGRAM: "", GHOSTTY_RESOURCES_DIR: "/tmp/ghostty" },
    { TERM: "vt100", TERM_PROGRAM: "", WT_SESSION: "windows-terminal" },
    { TERM: "xterm-256color", TERM_PROGRAM: "" },
  ];
  for (const env of cases) {
    withEnv({ CI: undefined, ...env }, () => {
      withTty(true, () => assert.equal(detectTerminalCapabilities("auto").level, "rich"));
    });
  }
});

test("auto detection uses ANSI for ordinary interactive terminals", () => {
  withEnv(
    {
      CI: undefined,
      TERM: "vt100",
      TERM_PROGRAM: "screen",
      WEZTERM_EXECUTABLE: undefined,
      KITTY_WINDOW_ID: undefined,
      GHOSTTY_RESOURCES_DIR: undefined,
      WT_SESSION: undefined,
    },
    () => {
      withTty(true, () => assert.equal(detectTerminalCapabilities("auto").level, "ansi"));
    },
  );
});

function withEnv(values: Record<string, string | undefined>, callback: () => void) {
  const previous = new Map<string, string | undefined>();
  for (const [key, value] of Object.entries(values)) {
    previous.set(key, process.env[key]);
    if (value === undefined) delete process.env[key];
    else process.env[key] = value;
  }
  try {
    callback();
  } finally {
    for (const [key, value] of previous) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
  }
}

function withTty(value: boolean, callback: () => void) {
  const stdin = descriptor(process.stdin, "isTTY");
  const stdout = descriptor(process.stdout, "isTTY");
  Object.defineProperty(process.stdin, "isTTY", { configurable: true, value });
  Object.defineProperty(process.stdout, "isTTY", { configurable: true, value });
  try {
    callback();
  } finally {
    restore(process.stdin, "isTTY", stdin);
    restore(process.stdout, "isTTY", stdout);
  }
}

function descriptor(object: object, key: PropertyKey): PropertyDescriptor | undefined {
  return Object.getOwnPropertyDescriptor(object, key);
}

function restore(object: object, key: PropertyKey, previous: PropertyDescriptor | undefined) {
  if (previous) Object.defineProperty(object, key, previous);
  else delete (object as Record<PropertyKey, unknown>)[key];
}
