import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { render } from "../../../src/tui/render.js";
import {
  ansiCapabilities,
  plainCapabilities,
  richCapabilities,
} from "../../../src/tui/capabilities.js";
import { stripAnsi } from "../../../src/tui/render-terminal.js";
import {
  providerEnums,
  withTerminalSize,
  assertLineWidths,
  assertOpencodePalette,
} from "./helpers/render-harness.js";

process.env.TURA_LANG = "en";

test("render filters internal task_status command updates from command sections", () => {
  const session = {
    id: "sess-task-status-hidden",
    title: "Task Status Hidden",
    status: "idle" as const,
  };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-agent",
        sessionID: session.id,
        role: "assistant",
        parts: [
          { id: "text", type: "text", text: "Try a bowl of noodles." },
          {
            id: "task-status-json",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output: {
                results: [
                  {
                    command_type: "task_status",
                    success: true,
                    output: {
                      task_status: {
                        status: "done",
                        task_detail: "user asked for a random food suggestion",
                      },
                    },
                  },
                ],
              },
            },
          },
          {
            id: "task-status-summary",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output:
                '[command_run: {\\"task_detail\\":\\"user asked for a random food suggestion\\"}]',
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: { show_command_instructions: true },
  });

  const plain = stripAnsi(render(state, richCapabilities()));

  assert.match(plain, /Try a bowl of noodles./u);
  assert.doesNotMatch(plain, /Commands/u);
  assert.doesNotMatch(plain, /command_run completed/u);
  assert.doesNotMatch(plain, /task_status|user asked for a random food suggestion/u);
});

test("render keeps L1 L2 L3 readable without overflow across terminal sizes", () => {
  const session = { id: "sess-layout", title: "Layout", status: "idle" as const };
  const longPath = "C:/Users/liuliu/Documents/tura/apps/tui/src/tui/render-terminal.ts:123";
  const state = reducer(initialState("C:/Users/liuliu/Documents/tura"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-layout-user",
        sessionID: "sess-layout",
        role: "user",
        parts: [
          {
            id: "part-layout-user",
            type: "text",
            text: `Please inspect ${longPath} and keep the answer compact even on a narrow terminal.`,
          },
        ],
      },
      {
        id: "msg-layout-assistant",
        sessionID: "sess-layout",
        role: "assistant",
        parts: [
          {
            id: "part-layout-assistant",
            type: "text",
            text:
              "**Layout evidence**\n" +
              "Short status first, details hidden by default.\n" +
              `Local path ${longPath}\n` +
              "| Phase | Evidence |\n" +
              "| --- | --- |\n" +
              "| L1 | plain safe text |\n" +
              "| L2 | geometric feedback |\n" +
              "| L3 | Primer-style rich UI |\n" +
              "```text\nnpm run test:e2e\n```\n" +
              "Extra line one\nExtra line two\nExtra line three\nExtra line four",
          },
          {
            id: "tool-layout",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: {
                command_type: "shell_command",
                command_line: "npm run test:e2e -- --layout",
              },
            },
          },
        ],
      },
    ],
    permissions: [{ id: "perm-layout", sessionID: "sess-layout", permission: "shell" }],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  for (const { cols, rows } of [
    { cols: 52, rows: 18 },
    { cols: 80, rows: 24 },
    { cols: 132, rows: 36 },
  ]) {
    for (const capabilities of [plainCapabilities(), ansiCapabilities(), richCapabilities()]) {
      const output = withTerminalSize(cols, rows, () => render(state, capabilities));
      assertLineWidths(output, cols);
      if (capabilities.level === "plain") assert.doesNotMatch(output, /\x1b/u);
      if (capabilities.level === "ansi") {
        assert.match(output, /[◆◇▏]/u);
        assert.doesNotMatch(stripAnsi(output), /^─{8,}$/mu);
        assertOpencodePalette(output);
      }
      if (capabilities.level === "rich") {
        assert.match(output, /\x1b\[38;2;64;224;208m/);
        assert.match(output, /\x1b\[48;2;16;19;20m/);
        assert.doesNotMatch(stripAnsi(output), /─{8,}/u);
        assert.doesNotMatch(output, /\x1b\[38;2;157;124;216m/);
        assert.doesNotMatch(output, /\x1b\[38;2;127;216;143m/);
        assertOpencodePalette(output);
      }
      assert.doesNotMatch(stripAnsi(output), /earlier output hidden|earlier output hidden/u);
    }
  }
});

test("render uses opencode-style turn spacing and configured command disclosure in L1 L2 L3", () => {
  const session = { id: "sess-turns", title: "Turn Layout", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-turn-user",
        sessionID: "sess-turns",
        role: "user",
        parts: [{ id: "part-turn-user", type: "text", text: "Summarize the terminal layout." }],
      },
      {
        id: "msg-turn-assistant",
        sessionID: "sess-turns",
        role: "assistant",
        parts: [
          {
            id: "part-turn-assistant",
            type: "text",
            text: "Feedback first. Command details follow the session setting.",
          },
          {
            id: "tool-turn",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: { command_type: "shell_command", command_line: "npm run test:e2e" },
            },
          },
        ],
      },
      {
        id: "msg-turn-assistant-followup",
        sessionID: "sess-turns",
        role: "assistant",
        parts: [{ id: "part-turn-assistant-followup", type: "text", text: "Follow-up block." }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const plain = render(state, plainCapabilities());
  assert.match(plain, /^\s{2}Summarize/m);
  assert.match(plain, /^\s{2}Feedback first/m);
  assert.doesNotMatch(plain, /(?:^|\n)(?:user|assistant)(?:\n|$)/);
  assert.match(plain, /Commands/);
  const plainLines = plain.split("\n");
  const plainCommandIndex = plainLines.findIndex((line) => line.includes("Commands"));
  assert.ok(plainCommandIndex >= 0);
  assert.equal(plainLines[plainCommandIndex], "* Commands");
  assert.equal(plainLines[plainCommandIndex - 1], "");
  assert.match(plainLines[plainCommandIndex + 1] ?? "", /\$ npm run test:e2e/);
  assert.match(plain, /\|- \+ #1 shell_command completed\s+\$ npm run test:e2e/);
  assert.doesNotMatch(plain, /\x1b|▏|◆|◇/u);

  const ansi = render(state, ansiCapabilities());
  assert.doesNotMatch(ansi, /◇.*user|◆.*assistant/u);
  assert.match(ansi, /^\x1b\[48;2;16;19;20m\x1b\[38;2;103;116;111m▏\x1b\[0m.*Feedback first/m);
  assert.match(ansi, /◇ Commands/u);
  const ansiLines = ansi.split("\n");
  const ansiCommandIndex = ansiLines.findIndex((line) => stripAnsi(line).includes("Commands"));
  assert.ok(ansiCommandIndex >= 0);
  assert.equal(stripAnsi(ansiLines[ansiCommandIndex]), "◇ Commands");
  assert.equal(stripAnsi(ansiLines[ansiCommandIndex - 1] ?? ""), "");
  assert.match(stripAnsi(ansiLines[ansiCommandIndex + 1] ?? ""), /\$ npm run test:e2e/);
  assert.notEqual(stripAnsi(ansiLines[ansiCommandIndex - 2] ?? ""), "");
  assert.doesNotMatch(ansiLines[ansiCommandIndex], /\x1b\[48;2;16;19;20m/);
  assert.match(ansi, /└─ ✓ #1 shell_command completed\s+\$ npm run test:e2e/u);
  assertOpencodePalette(ansi);

  const rich = render(state, richCapabilities());
  assert.match(rich, /^\x1b\[48;2;16;19;20m\x1b\[38;2;244;247;235m▏\x1b\[0m.*Summarize/m);
  assert.doesNotMatch(rich, /^\x1b\[38;2;(?:103;116;111|244;247;235)m▏\x1b\[0m +\x1b\[0m$/m);
  assert.match(rich, /^\x1b\[48;2;16;19;20m\x1b\[38;2;103;116;111m▏\x1b\[0m.*Feedback first/m);
  assert.doesNotMatch(rich, /(?:user|assistant)/);
  assert.doesNotMatch(rich, /[┌├└].*(?:user|assistant)/u);
  assert.match(rich, /\x1b\[48;2;16;19;20m/);
  assert.doesNotMatch(rich, /\x1b\[38;2;157;124;216m/);
  assert.doesNotMatch(rich, /\x1b\[38;2;127;216;143m/);
  assert.match(rich, /◇ Commands/u);
  const richLines = rich.split("\n");
  const richCommandIndex = richLines.findIndex((line) => stripAnsi(line).includes("Commands"));
  assert.ok(richCommandIndex >= 0);
  assert.equal(stripAnsi(richLines[richCommandIndex]), "◇ Commands");
  assert.equal(stripAnsi(richLines[richCommandIndex - 1] ?? ""), "");
  assert.match(stripAnsi(richLines[richCommandIndex + 1] ?? ""), /\$ npm run test:e2e/);
  assert.notEqual(stripAnsi(richLines[richCommandIndex - 2] ?? ""), "");
  assert.doesNotMatch(richLines[richCommandIndex], /\x1b\[48;2;16;19;20m/);
  assert.match(rich, /└─ ✓ #1 shell_command completed\s+\$ npm run test:e2e/u);
  assertOpencodePalette(rich);
});

test("render ignores adjacent command_run summaries without command types", () => {
  const session = { id: "sess-command-group", title: "Command Group", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-group-user",
        sessionID: "sess-command-group",
        role: "user",
        parts: [{ id: "part-group-user", type: "text", text: "hello there" }],
      },
      {
        id: "msg-group-command-1",
        sessionID: "sess-command-group",
        role: "assistant",
        parts: [
          {
            id: "tool-group-command-1",
            type: "tool",
            tool: "command_run",
            state: { status: "completed", output: "Greeted the user" },
          },
        ],
      },
      {
        id: "msg-group-command-2",
        sessionID: "sess-command-group",
        role: "assistant",
        parts: [
          {
            id: "tool-group-command-2",
            type: "tool",
            tool: "command_run",
            state: { status: "completed", output: "Greeted the user again" },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const rich = render(state, richCapabilities());
  const plain = stripAnsi(rich);
  assert.match(plain, /hello there/);
  assert.doesNotMatch(plain, /◆\s+hello there/u);
  assert.doesNotMatch(plain, /Commands/u);
  assert.doesNotMatch(plain, /#1 completed\s+\$ Greeted/u);
  assert.doesNotMatch(plain, /\[command_run:/u);
  const lines = plain.split("\n");
  const userIndex = lines.findIndex((line) => line.includes("hello there"));
  assert.ok(userIndex >= 0);
});

test("render filters command_run summaries and type-only records from command sections", () => {
  const session = { id: "sess-command-filter", title: "Command Filter", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-command-filter",
        sessionID: session.id,
        role: "assistant",
        parts: [
          {
            id: "tool-filter-task-detail",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output: '{"status":"done","task_detail":"large file scan"}',
            },
          },
          {
            id: "tool-filter-summary",
            type: "tool",
            tool: "command_run",
            state: { status: "completed", output: "large file scan" },
          },
          {
            id: "tool-filter-valid-and-type-only",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output: {
                results: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "$ErrorActionPreference='Stop'",
                    status: "completed",
                    success: true,
                  },
                  {
                    step: 1,
                    command_type: "shell_command",
                    status: "completed",
                    success: true,
                  },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: { show_command_instructions: true },
  });

  const plain = stripAnsi(render(state, richCapabilities()));

  assert.equal(plain.match(/Commands/g)?.length, 1);
  assert.match(plain, /\$ \$ErrorActionPreference='Stop'/u);
  assert.doesNotMatch(plain, /\$ large file scan/u);
  assert.doesNotMatch(plain, /\$ \{"status":"done","task_detail":"large file scan"\}/u);
  assert.doesNotMatch(plain, /\$ shell_command/u);
});

test("render keeps command-only updates at their exact message position", () => {
  const session = { id: "sess-command-order", title: "Command Order", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-order-user",
        sessionID: "sess-command-order",
        role: "user",
        parts: [{ id: "part-order-user", type: "text", text: "Fix it" }],
      },
      {
        id: "msg-order-first",
        sessionID: "sess-command-order",
        role: "assistant",
        parts: [{ id: "part-order-first", type: "text", text: "First visible reply." }],
      },
      {
        id: "msg-order-tool",
        sessionID: "sess-command-order",
        role: "assistant",
        parts: [
          {
            id: "tool-order-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: { command_type: "shell_command", command_line: "npm test" },
            },
          },
        ],
      },
      {
        id: "msg-order-final",
        sessionID: "sess-command-order",
        role: "assistant",
        parts: [{ id: "part-order-final", type: "text", text: "Final visible reply." }],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  for (const capabilities of [plainCapabilities(), richCapabilities()]) {
    const output = withTerminalSize(100, 30, () => stripAnsi(render(state, capabilities)));
    const firstIndex = output.indexOf("First visible reply.");
    const finalIndex = output.indexOf("Final visible reply.");
    const commandIndex = output.indexOf("$ npm test");
    assert.ok(firstIndex >= 0);
    assert.ok(commandIndex > firstIndex, output);
    assert.ok(finalIndex > commandIndex, output);
  }
});

test("render keeps assistant text above command parts even when tool part arrives first", () => {
  const session = { id: "sess-part-order", title: "Part Order", status: "idle" as const };
  const state = {
    ...initialState("C:/repo"),
    session,
    messages: [
      {
        id: "msg-part-order",
        sessionID: "sess-part-order",
        role: "assistant" as const,
        parts: [
          {
            id: "tool-part-order",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output: '{"status":"done","task_detail":"Greeting answered"}',
            },
          },
          {
            id: "text-part-order",
            type: "text",
            text: "Hello, greeting answered.",
          },
        ],
      },
    ],
    sessionConfig: { show_command_instructions: true },
  };

  const output = withTerminalSize(100, 30, () => stripAnsi(render(state, plainCapabilities())));
  const textIndex = output.indexOf("Hello, greeting answered.");
  assert.ok(textIndex >= 0, output);
  assert.doesNotMatch(output, /Commands|command_run completed|\$ Greeting answered/u);
});

test("render normalizes command progress carriage returns into new lines", () => {
  const session = {
    id: "sess-command-progress",
    title: "Command Progress",
    status: "idle" as const,
  };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-progress-user",
        sessionID: "sess-command-progress",
        role: "user",
        parts: [{ id: "part-progress-user", type: "text", text: "run progress" }],
      },
      {
        id: "msg-progress-assistant",
        sessionID: "sess-command-progress",
        role: "assistant",
        parts: [
          {
            id: "part-progress-text",
            type: "text",
            text: "started\rstill running\x1b[2K\rfinished",
          },
          {
            id: "tool-progress",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output: {
                results: [
                  {
                    step: 1,
                    command_type: "shell_command",
                    command_line: "Downloading 10%\rDownloading 90%\x1b[1Gdone",
                    status: "completed",
                    success: true,
                  },
                ],
              },
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const plain = stripAnsi(render(state, richCapabilities()));
  assert.doesNotMatch(plain, /\r/u);
  assert.doesNotMatch(plain, /\x1b\[(?:2K|1G)/u);
  assert.match(plain, /started/);
  assert.match(plain, /still running/);
  assert.match(plain, /finished/);
  assert.match(plain, /Commands/);
  assert.match(plain, /#1 shell_command completed\s+\$ Downloading 10%/u);
});

test("render keeps composer and bottom meta visible after large command blocks", () => {
  const session = { id: "sess-command-footer", title: "Command Footer", status: "idle" as const };
  const commandParts = Array.from({ length: 8 }, (_, index) => ({
    id: `tool-footer-${index + 1}`,
    type: "tool",
    tool: "command_run",
    state: {
      status: "completed",
      input: {
        command_type: "shell_command",
        command_line: `Get-Content -Raw apps/tui/test-results/tui-snake-playwright/very-long-run-${index + 1}/summary.json`,
      },
    },
  }));
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-footer-user",
        sessionID: "sess-command-footer",
        role: "user",
        parts: [{ id: "part-footer-user", type: "text", text: "read summaries" }],
      },
      {
        id: "msg-footer-commands",
        sessionID: "sess-command-footer",
        role: "assistant",
        parts: commandParts,
      },
      {
        id: "msg-footer-reply",
        sessionID: "sess-command-footer",
        role: "assistant",
        parts: [
          {
            id: "part-footer-reply",
            type: "text",
            text: "This new reply must render below the command block without covering it.",
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  // Default view shows the most recent content and keeps the footer pinned.
  const output = withTerminalSize(106, 18, () => render(state, richCapabilities()));
  const plain = stripAnsi(output);
  const lines = plain.split("\n");
  assert.ok(lines.some((line) => line.includes("Enter to send")));
  assert.ok(lines.some((line) => line.includes("tokens")));
  assert.ok(lines.some((line) => line.includes("This new reply must render")));
  assertLineWidths(output, 106);
});
