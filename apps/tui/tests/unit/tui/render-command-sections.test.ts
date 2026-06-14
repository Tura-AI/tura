import assert from "node:assert/strict";
import test from "node:test";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { render } from "../../../src/tui/render.js";
import { richCapabilities } from "../../../src/tui/capabilities.js";
import { stripAnsi } from "../../../src/tui/render-terminal.js";
import {
  providerEnums,
  withTerminalSize,
  withNow,
  assertLineWidths,
} from "./helpers/render-harness.js";

process.env.TURA_LANG = "en";

test("render applies rich text cleanup to tool summaries", () => {
  const session = { id: "sess-tool-rich", title: "Tool Rich", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-tool-rich",
        sessionID: "sess-tool-rich",
        role: "assistant",
        parts: [
          {
            id: "tool-rich",
            type: "tool",
            tool: "browser",
            state: {
              status: "completed",
              output: {
                text: "Result: <b>Frontend</b> verified <code>npm run verify:all</code> [MEDIA:C:/tmp/a.png:MEDIA]",
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

  const transcript = render(state, richCapabilities());
  assert.match(transcript, /Frontend/);
  assert.match(transcript, /npm run verify:all/);
  assert.match(transcript, /C:\/tmp\/a\.png/);
  assert.doesNotMatch(transcript, /\[MEDIA:C:\/tmp\/a\.png:MEDIA\]/);
  assert.doesNotMatch(transcript, /<b>|<\/b>|<code>|<\/code>/);
});

test("render shows assistant command summaries, command details setting, and thinking state", () => {
  const session = { id: "sess-commands", title: "Commands", status: "busy" as const };
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-command-user",
        sessionID: "sess-commands",
        role: "user",
        created_at: 1_000_000,
        parts: [{ id: "part-command-user", type: "text", text: "Run checks" }],
      },
      {
        id: "msg-command-summary",
        sessionID: "sess-commands",
        role: "assistant",
        parts: [
          {
            id: "part-command-text",
            type: "text",
            text: "Checking the app before the final answer.",
          },
          {
            id: "part-inline-payload",
            type: "text",
            text: '[command_run: {"task_detail":"inline payload summary should be readable"}]\n[command_run: {"status":"done"}]',
          },
          {
            id: "tool-command-1",
            type: "tool",
            tool: "runtime",
            state: {
              status: "completed",
              input: {
                command_type: "shell_command",
                command_line: "npm test -- --runInBand",
              },
            },
          },
          {
            id: "tool-command-2",
            type: "tool",
            tool: "runtime",
            state: {
              status: "completed",
              input: {
                command_type: "shell_command",
                command_line: "node tools/snake_playwright.mjs",
              },
            },
          },
          {
            id: "tool-powershell-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                step: 7,
                command_type: "shell_command",
                command_line: "Get-ChildItem -Force | Select-Object FullName",
              },
            },
          },
          {
            id: "tool-running-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: { step: 9, command_type: "shell_command", command_line: "pnpm test --watch" },
            },
          },
          {
            id: "tool-task-summary",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              output:
                '[command_run: {\\"task_detail\\":\\"provide concise final verification summary\\"}]',
            },
          },
          {
            id: "tool-status",
            type: "tool",
            tool: "command_run",
            state: { status: "completed", output: '[command_run: {\\"status\\":\\"done\\"}]' },
          },
          {
            id: "tool-input-status",
            type: "tool",
            tool: "command_run",
            state: { status: "completed", input: { status: "done" } },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: { show_command_instructions: false },
  });

  const collapsed = withNow(1_012_300, () => render(state, richCapabilities()));
  assert.match(collapsed, /Checking the app/);
  assert.match(collapsed, /Commands/);
  assert.doesNotMatch(collapsed, /Commands:\s*\d+/);
  assert.match(collapsed, /[◆◇].*Commands/);
  const collapsedCommandLine = collapsed
    .split("\n")
    .find((line) => stripAnsi(line).includes("Commands"));
  assert.ok(collapsedCommandLine);
  assert.match(stripAnsi(collapsedCommandLine), /^[◆◇] Commands$/u);
  assert.doesNotMatch(collapsedCommandLine, /\x1b\[48;2;16;19;20m/);
  assert.match(collapsed, /\x1b\[38;2;103;116;111m/);
  assert.doesNotMatch(collapsed, /last.*Get-ChildItem -Force/);
  assert.doesNotMatch(collapsed, /show commands/);
  assert.doesNotMatch(collapsed, /click \/ Ctrl\+O/);
  const collapsedText = stripAnsi(collapsed).replace(/\s*\n\s*/g, "");
  assert.doesNotMatch(collapsedText, /inline payload summary should be readable/);
  assert.doesNotMatch(collapsed, /\[command_run:/);
  assert.doesNotMatch(collapsed, /bash: npm test -- --runInBand/);
  assert.doesNotMatch(collapsed, /task_detail/);
  assert.doesNotMatch(collapsed, /\{"status"/);
  assert.match(stripAnsi(collapsed), /thinking\s+12s/);
  assert.match(stripAnsi(collapsed), /✦ thinking\s+12s/);
  assert.doesNotMatch(collapsed, /thinking.*Commands/);

  state = reducer(state, {
    type: "session-config",
    value: { show_command_instructions: true },
  });
  const expanded = withNow(1_012_300, () => render(state, richCapabilities()));
  assert.doesNotMatch(expanded, /hide commands/);
  const expandedCommandLine = expanded
    .split("\n")
    .find((line) => stripAnsi(line).includes("Commands"));
  assert.ok(expandedCommandLine);
  assert.match(stripAnsi(expandedCommandLine), /^[◆◇] Commands$/u);
  assert.doesNotMatch(expandedCommandLine, /\x1b\[48;2;16;19;20m/);
  assert.match(expanded, /#1 shell_command completed\s+\$ npm test -- --runInBand/);
  assert.match(expanded, /#1 shell_command completed\s+\$ node tools\/snake_playwright\.mjs/);
  assert.match(expanded, /#7 shell_command completed\s+\$ Get-ChildItem -Force/);
  assert.match(expanded, /#9 shell_command running\s+\$ pnpm test --watch/);
  assert.doesNotMatch(expanded, /provide concise final verification summary/);
  assert.doesNotMatch(expanded, /\$ done/);
  assert.match(expanded, /\x1b\[38;2;103;116;111m.*\$ pnpm test --watch/);
  const npmTestLine = expanded
    .split("\n")
    .find((line) => stripAnsi(line).includes("$ npm test -- --runInBand"));
  assert.ok(npmTestLine);
  assert.doesNotMatch(npmTestLine, /\x1b\[48;2;16;19;20m/);
  assert.doesNotMatch(expanded, /\{"command_line"/);
  assert.equal(
    expanded
      .split("\n")
      .filter((line) =>
        /\$ (?:npm test|node tools\/snake_playwright|Get-ChildItem|pnpm test)/.test(
          stripAnsi(line),
        ),
      ).length,
    4,
  );

  const solid = withNow(1_012_300, () =>
    stripAnsi(render({ ...state, thinkingFrame: 0 }, richCapabilities())),
  );
  const hollow = withNow(1_012_300, () =>
    stripAnsi(render({ ...state, thinkingFrame: 1 }, richCapabilities())),
  );
  const starburst = withNow(1_012_300, () =>
    stripAnsi(render({ ...state, thinkingFrame: 2 }, richCapabilities())),
  );
  assert.match(solid, /^◆ Commands$/mu);
  assert.match(hollow, /^◇ Commands$/mu);
  assert.match(solid, /✦ thinking\s+12s/);
  assert.match(hollow, /✧ thinking\s+12s/);
  assert.match(starburst, /✶ thinking\s+12s/);
  assert.match(solid, /^└─ ■ #9 shell_command running\s+\$ pnpm test --watch$/mu);
  assert.match(hollow, /^└─ □ #9 shell_command running\s+\$ pnpm test --watch$/mu);
  assert.doesNotMatch(solid, /task_status|provide concise final verification summary|\$ done/u);
});

test("render shows streamed command_run results before the whole command batch finishes", () => {
  const session = {
    id: "sess-streamed-commands",
    title: "Streamed Commands",
    status: "busy" as const,
  };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-streamed-command-user",
        sessionID: "sess-streamed-commands",
        role: "user",
        created_at: 1_000,
        parts: [{ id: "part-streamed-command-user", type: "text", text: "Run build and tests" }],
      },
      {
        id: "msg-streamed-command-tool",
        sessionID: "sess-streamed-commands",
        role: "assistant",
        created_at: 2_000,
        parts: [
          {
            id: "tool-streamed-command-run",
            type: "tool",
            tool: "command_run",
            state: {
              status: "running",
              input: {
                commands: [
                  { step: 3, command_type: "shell_command", command_line: "npm run build" },
                  {
                    step: 10,
                    command_type: "shell_command",
                    command_line: "npm test -- --runInBand",
                  },
                ],
              },
              output: {
                streamed_command_run_result: {
                  results: [
                    {
                      status: "completed",
                      success: true,
                      step: 3,
                      command_type: "shell_command",
                      command_line: "npm run build",
                      output: { text: "built" },
                    },
                    {
                      status: "running",
                      success: null,
                      command_type: "shell_command",
                      command_line: "npm test -- --runInBand",
                      output: { stdout: "still testing" },
                    },
                  ],
                },
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

  const solid = withTerminalSize(100, 30, () =>
    stripAnsi(render({ ...state, thinkingFrame: 0 }, richCapabilities())),
  );
  const hollow = withTerminalSize(100, 30, () =>
    stripAnsi(render({ ...state, thinkingFrame: 1 }, richCapabilities())),
  );

  assert.match(solid, /^◆ Commands$/mu);
  assert.match(solid, /^├─ ✓ #3 shell_command completed\s+\$ npm run build$/mu);
  assert.match(solid, /^└─ ■ #10 shell_command running\s+\$ npm test -- --runInBand$/mu);
  assert.match(hollow, /^◇ Commands$/mu);
  assert.match(hollow, /^└─ □ #10 shell_command running\s+\$ npm test -- --runInBand$/mu);
});

test("render uses per-command results from dirty command_run error records", () => {
  const session = {
    id: "sess-dirty-command-error",
    title: "Dirty Command Error",
    status: "idle" as const,
  };
  const commands = [
    {
      step: 1,
      command_type: "shell_command",
      command_line: 'py -3 -c "print(\\"ok\\")"',
    },
    {
      step: 1,
      command_type: "shell_command",
      command_line: 'py -3 -c "raise SystemExit(1)"',
    },
    {
      step: 1,
      command_type: "shell_command",
      command_line: "Get-Content crates/runtime/src/turn_loop/provider_step.rs -TotalCount 120",
    },
    {
      step: 1,
      command_type: "shell_command",
      command_line: "Get-Content crates/session_log/src/store/mod.rs -TotalCount 260",
    },
  ];
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-dirty-command-error",
        sessionID: session.id,
        role: "assistant",
        created_at: 1_000,
        parts: [
          {
            id: "tool-dirty-command-error",
            type: "tool",
            tool: "command_run",
            state: {
              status: "error",
              input: { commands },
              error: [
                "Exit code: 1",
                "Old diagnostic output included unrelated commands:",
                "npm run lint",
                "npm run format:check",
                "npm run typecheck",
              ].join("\n"),
            },
            metadata: {
              success: false,
              error: [
                "Exit code: 1",
                "Old diagnostic output included unrelated commands:",
                "npm run lint",
                "npm run format:check",
                "npm run typecheck",
              ].join("\n"),
              output: {
                commands,
                results: [
                  { success: true, output: "Exit code: 0\nOutput:\nok" },
                  { success: false, output: "Exit code: 1\nOutput:\nfailed" },
                  { success: true, output: "Exit code: 0\nOutput:\nfile contents" },
                  {
                    success: false,
                    output:
                      "Exit code: 1\nStderr:\nCannot find path 'crates/session_log/src/store/mod.rs'",
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

  const transcript = withTerminalSize(140, 30, () => stripAnsi(render(state, richCapabilities())));

  assert.match(transcript, /^◇ Commands$/mu);
  assert.match(transcript, /^├─ ✓ #1 shell_command completed\s+\$ py -3 -c "print/mu);
  assert.match(transcript, /^├─ x #1 shell_command failed\s+\$ py -3 -c "raise SystemExit/mu);
  assert.match(transcript, /^├─ ✓ #1 shell_command completed\s+\$ Get-Content crates\/runtime/mu);
  assert.match(transcript, /^└─ x #1 shell_command failed\s+\$ Get-Content crates\/session_log/mu);
  assert.doesNotMatch(transcript, /\$ npm run (?:lint|format:check|typecheck)/u);
});

test("render uses real command_run step numbers from non-streamed command batches", () => {
  const session = { id: "sess-command-batch-steps", title: "Batch Steps", status: "idle" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-command-batch-steps",
        sessionID: "sess-command-batch-steps",
        role: "assistant",
        created_at: 1_000,
        parts: [
          {
            id: "tool-command-batch-steps",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: {
                commands: [
                  { step: 4, command_type: "shell_command", command_line: "npm run lint" },
                  { step: 12, command_type: "shell_command", command_line: "npm run typecheck" },
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

  const transcript = stripAnsi(render(state, richCapabilities()));
  assert.match(transcript, /^├─ ✓ #4 shell_command completed\s+\$ npm run lint$/mu);
  assert.match(transcript, /^└─ ✓ #12 shell_command completed\s+\$ npm run typecheck$/mu);
  assert.equal(
    transcript.split("\n").filter((line) => /\$ npm run (?:lint|typecheck)/u.test(line)).length,
    2,
  );
});

test("render blinks only the running command block icon", () => {
  const session = { id: "sess-group-running", title: "Group Running", status: "busy" as const };
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-group-running",
        sessionID: session.id,
        role: "assistant",
        parts: [
          {
            id: "tool-group-done",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: { command_type: "shell_command", command_line: "npm run build" },
            },
          },
          {
            id: "tool-group-progress",
            type: "tool",
            tool: "command_run",
            state: {
              status: "in_progress",
              input: { command_type: "shell_command", command_line: "npm test" },
            },
          },
        ],
      },
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
    sessionConfig: { show_command_instructions: false },
  });

  const solid = stripAnsi(render({ ...state, thinkingFrame: 0 }, richCapabilities()));
  const hollow = stripAnsi(render({ ...state, thinkingFrame: 1 }, richCapabilities()));

  assert.match(solid, /^◇ Commands$/mu);
  assert.match(solid, /^◆ Commands$/mu);
  assert.equal(solid.match(/Commands/g)?.length, 2);
  assert.equal(hollow.match(/Commands/g)?.length, 2);
});

test("render keeps each command detail to one visible line", () => {
  const session = { id: "sess-long-command", title: "Long Command", status: "idle" as const };
  const tail = "TAIL_VISIBLE_AFTER_WRAP";
  const secondLine = "echo MULTILINE_COMMAND_SECOND_LINE_VISIBLE";
  const command = `node scripts/check.mjs --with-a-very-long-argument ${"arg".repeat(18)} ${tail}\n${secondLine}`;
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      {
        id: "msg-long-command",
        sessionID: session.id,
        role: "assistant",
        parts: [
          {
            id: "tool-long-command",
            type: "tool",
            tool: "command_run",
            state: {
              status: "completed",
              input: { command_type: "shell_command", command_line: command },
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

  const output = withTerminalSize(58, 30, () => render(state, richCapabilities()));
  const plainLines = stripAnsi(output).split("\n");
  const commandLines = plainLines.filter((line) =>
    /\$ |MULTILINE_COMMAND_SECOND_LINE_VISIBLE/u.test(line),
  );
  assert.equal(commandLines.length, 1, output);
  assert.match(commandLines[0] ?? "", /\$ node scripts\/check/u);
  assert.doesNotMatch(stripAnsi(output), /MULTILINE_COMMAND_SECOND_LINE_VISIBLE/);
  assertLineWidths(output, 58);
});
