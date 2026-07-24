import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { initialState, reducer } from "../../../src/tui/reducer.js";
import { render } from "../../../src/tui/render.js";
import { renderRichText } from "../../../src/tui/render-rich-text.js";
import {
  ansiCapabilities,
  plainCapabilities,
  richCapabilities,
} from "../../../src/tui/capabilities.js";
import { stripAnsi } from "../../../src/tui/render-terminal.js";
import {
  providerEnums,
  withTerminalSize,
  assertFitsTerminal,
  assertLineWidths,
  assertWideMenuGap,
} from "./helpers/render-harness.js";
import type {
  Message,
  MessagePart,
  Session,
  SessionStatusValue,
} from "../../../src/types/session.js";
import { visibleTextWidth } from "../../../src/tui/render-terminal.js";

process.env.TURA_LANG = "en";

type TestSession = Session & { title: string };

function sessionFixture(
  id: string,
  title: string,
  status: SessionStatusValue = "idle",
  overrides: Partial<Session> = {},
): TestSession {
  return {
    id,
    title,
    name: title,
    parent_id: null,
    created_at: 1_000,
    updated_at: 1_000,
    directory: "C:/repo",
    model: null,
    agent: null,
    session_type: null,
    auto_session_name: true,
    kill_processes_on_start: false,
    validator_enabled: false,
    force_planning: false,
    model_variant: null,
    model_acceleration_enabled: false,
    disable_permission_restrictions: false,
    status,
    message_count: 0,
    task_management: null,
    context_tokens: null,
    plan_summary: null,
    session_display_name: title,
    ...overrides,
  };
}

function textPart(sessionID: string, messageID: string, id: string, text: string): MessagePart {
  return { id, sessionID, messageID, type: "text", text };
}

function textMessage(id: string, sessionID: string, text: string): Message {
  return {
    id,
    sessionID,
    role: "assistant",
    created_at: 1_000,
    updated_at: 1_000,
    time: { created: 1_000, updated: 1_000 },
    parts: [textPart(sessionID, id, `${id}-part`, text)],
  };
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

test("render applies communication style rich text without leaking protocol markup", () => {
  const session = sessionFixture("sess-rich", "Rich");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      textMessage(
        "msg-rich",
        "sess-rich",
        "<b>Bold</b> <i>Italic</i> <u>Under</u> <s>Gone</s> <code>src/App.tsx:12</code>\n<a href='https://example.com'>Example</a>\n<span class='tg-spoiler'>secret</span>\n<blockquote>quoted</blockquote>\n<pre><code class='language-python'>print('hello')</code></pre>\n[MEDIA:C:/tmp/shot.png:MEDIA]\n[MEDIA:https://example.com/shot.png:MEDIA]\n[EMOJI:react:👍:EMOJI]",
      ),
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const transcript = render(state, richCapabilities());
  assert.match(transcript, /\x1b\[1mBold\x1b\[0m/);
  assert.match(transcript, /\x1b\[3mItalic\x1b\[0m/);
  assert.match(transcript, /\x1b\[4mUnder\x1b\[0m/);
  assert.match(transcript, /\x1b\[9mGone\x1b\[0m/);
  assert.match(
    transcript,
    /Gone\x1b\[0m\x1b\[48;2;20;23;24m\x1b\[38;2;244;247;235m \x1b\[48;5;236m\x1b\[38;2;217;222;205m src\/App\.tsx:12 \x1b\[0m\x1b\[48;2;20;23;24m/,
  );
  assert.doesNotMatch(transcript, /\x1b\[36msrc\/App\.tsx:12\x1b\[0m/);
  assert.match(transcript, /Example/);
  assert.match(transcript, /https:\/\/example\.com/);
  assert.doesNotMatch(stripAnsi(transcript), /Example \(https:\/\/example\.com\)/);
  assert.match(transcript, /\x1b\]8;;https:\/\/example\.com\x1b\\/);
  assert.doesNotMatch(transcript, /\[MEDIA:C:\/tmp\/shot\.png:MEDIA\]/);
  assert.match(transcript, /\x1b\[48;5;236m\x1b\[38;2;217;222;205m C:\/tmp\/shot\.png \x1b\[0m/u);
  assert.match(transcript, /https:\/\/example\.com\/shot\.png/);
  assert.match(
    transcript,
    /\x1b\]8;;https:\/\/example\.com\/shot\.png\x1b\\\x1b\[48;5;236m\x1b\[38;2;217;222;205m https:\/\/example\.com\/shot\.png \x1b\[0m/u,
  );
  assert.match(transcript, /👍/u);
  assert.doesNotMatch(transcript, /\[EMOJI:/);
  assert.match(transcript, /\x1b\[48;5;234m\x1b\[38;2;217;222;205mquoted/);
  assert.doesNotMatch(stripAnsi(transcript), /│ quoted/);
  assert.match(transcript, /\x1b\[48;5;234m\x1b\[38;2;217;222;205mprint\('hello'\)/);
  assert.doesNotMatch(stripAnsi(transcript), /```/);
  const htmlCodeLines = transcript.split("\n");
  const htmlCodeLineIndex = htmlCodeLines.findIndex((line) =>
    stripAnsi(line).includes("print('hello')"),
  );
  assert.ok(htmlCodeLineIndex > 0);
  const htmlCodeTop = htmlCodeLines[htmlCodeLineIndex - 1] ?? "";
  const htmlCodeBottom = htmlCodeLines[htmlCodeLineIndex + 1] ?? "";
  assert.match(stripAnsi(htmlCodeTop), /^▏\s*$/u);
  assert.match(stripAnsi(htmlCodeBottom), /^▏\s*$/u);
  assert.ok(htmlCodeTop.includes("\x1b[48;5;234m"));
  assert.ok(htmlCodeBottom.includes("\x1b[48;5;234m"));
  assert.doesNotMatch(transcript, /<b>|<\/code>/);
});

test("render gracefully downgrades rich text across display levels", () => {
  const session = sessionFixture("sess-rich-levels", "Rich Levels");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      textMessage(
        "msg-rich-levels",
        "sess-rich-levels",
        "<b>Bold</b> <code>src/App.tsx:12</code>\n<a href='https://example.com'>Example</a>\n<blockquote>quoted</blockquote>\n[MEDIA:https://example.com/shot.png:MEDIA]\n[EMOJI:react:👍:EMOJI]",
      ),
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const plain = render(state, plainCapabilities());
  assert.match(plain, /Bold/);
  assert.match(plain, /src\/App\.tsx:12/);
  assert.match(plain, /Example/);
  assert.doesNotMatch(plain, /Example \(https:\/\/example\.com\)/);
  assert.match(plain, /https:\/\/example\.com\/shot\.png/);
  assert.doesNotMatch(plain, /\[MEDIA:https:\/\/example\.com\/shot\.png:MEDIA\]/);
  assert.match(plain, /👍/u);
  assert.doesNotMatch(plain, /\[EMOJI:/);
  assert.doesNotMatch(plain, /\x1b|<b>|<\/code>|\x1b\]8|▏/u);

  const ansi = render(state, ansiCapabilities());
  assert.match(ansi, /Bold/);
  assert.match(ansi, /Example/);
  assert.match(ansi, /https:\/\/example\.com/);
  assert.match(ansi, /https:\/\/example\.com\/shot\.png/);
  assert.doesNotMatch(ansi, /\[MEDIA:https:\/\/example\.com\/shot\.png:MEDIA\]/);
  assert.match(
    ansi,
    /\x1b\[48;5;236m\x1b\[38;2;217;222;205m https:\/\/example\.com\/shot\.png \x1b\[0m/u,
  );
  assert.match(ansi, /👍/u);
  assert.match(ansi, /\x1b\[[0-9;]*m/);
  assert.doesNotMatch(ansi, /<b>|<\/code>/u);
  assert.match(ansi, /\x1b\]8;;https:\/\/example\.com\/shot\.png\x1b\\/);
  assert.match(ansi, /\x1b\[48;2;20;23;24m\x1b\[38;2;103;116;111m▏\x1b\[0m\x1b\[48;2;20;23;24m/);

  const rich = render(state, richCapabilities());
  assert.match(rich, /\x1b\[1mBold\x1b\[0m/);
  assert.match(rich, /\x1b\]8;;https:\/\/example\.com\x1b\\/);
  assert.doesNotMatch(stripAnsi(rich), /Example \(https:\/\/example\.com\)/);
  assert.match(rich, /quoted/);
  assert.doesNotMatch(stripAnsi(rich), /│ quoted/);
  assert.match(rich, /👍/u);
  assert.doesNotMatch(rich, /\[EMOJI:/);
  assert.doesNotMatch(rich, /<b>|<\/code>/);
});

test("load session rich text degradation preserves html block newlines", () => {
  const session = sessionFixture("sess-load-rich-html", "Loaded Rich HTML");
  const loadedHtml =
    "<p>BEFORE_BLOCK</p><ul><li>FIRST_ITEM</li><li>SECOND_ITEM</li></ul>" +
    "<pre><code class='language-ts'>const a = 1;\nconst b = 2;</code></pre>" +
    "<p>AFTER_BLOCK</p>";
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [textMessage("msg-load-rich-html", session.id, loadedHtml)],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const transcript = withTerminalSize(100, 28, () => render(state, richCapabilities()));
  const plainTranscript = stripAnsi(transcript);
  assert.match(
    plainTranscript,
    /BEFORE_BLOCK[\s\S]*FIRST_ITEM[\s\S]*SECOND_ITEM[\s\S]*const a = 1;[\s\S]*const b = 2;[\s\S]*AFTER_BLOCK/u,
  );
  assert.doesNotMatch(plainTranscript, /<p>|<\/p>|<ul>|<\/ul>|<li>|<\/li>|<\/code>|<\/pre>/u);

  const transcriptLines = plainTranscript
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  const beforeLine = transcriptLines.find((line) => line.includes("BEFORE_BLOCK")) ?? "";
  const firstLine = transcriptLines.find((line) => line.includes("FIRST_ITEM")) ?? "";
  const secondLine = transcriptLines.find((line) => line.includes("SECOND_ITEM")) ?? "";

  assert.ok(beforeLine, plainTranscript);
  assert.ok(firstLine, plainTranscript);
  assert.ok(secondLine, plainTranscript);
  assert.doesNotMatch(beforeLine, /FIRST_ITEM/u);
  assert.doesNotMatch(firstLine, /SECOND_ITEM/u);
  assert.doesNotMatch(secondLine, /const a = 1;/u);
});

test("render recognizes common code tag attributes and markdown fence info strings", () => {
  const session = sessionFixture("sess-rich-code-attrs", "Rich Code Attributes");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      textMessage(
        "msg-rich-code-attrs",
        session.id,
        [
          "Inline <code class='language-ts'>const answer = 42;</code>",
          "<pre><code data-lang='ts' class='language-ts'>const htmlBlock = true;</code></pre>",
          "```tsx filename=src/App.tsx",
          "export function App() {",
          "  return <main />;",
          "}",
          "```",
          "~~~c++ title=engine.cpp",
          "int main() { return 0; }",
          "~~~",
        ].join("\n"),
      ),
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const transcript = withTerminalSize(110, 34, () => render(state, richCapabilities()));
  const plain = stripAnsi(transcript);

  assert.match(transcript, /\x1b\[48;5;236m\x1b\[38;2;217;222;205m const answer = 42; \x1b\[0m/);
  assert.match(transcript, /\x1b\[48;5;234m\x1b\[38;2;217;222;205mconst htmlBlock = true;/);
  assert.match(transcript, /\x1b\[48;5;234m\x1b\[38;2;217;222;205mexport function App\(\) \{/);
  assert.match(transcript, /\x1b\[48;5;234m\x1b\[38;2;217;222;205mint main\(\) \{ return 0; \}/);
  assert.doesNotMatch(plain, /```|~~~|<code|<\/code>|<pre|<\/pre>/u);
  assert.doesNotMatch(plain, /filename=src\/App\.tsx|title=engine\.cpp/u);
});

test("render keeps compaction threshold rich text visible around angle bracket formulas", () => {
  const session = sessionFixture("sess-compact-rich", "Compact Rich");
  const compactAnswer = [
    "按现在代码逻辑：",
    "触发注入条件是：<context_tokens >= min(60% * model_context_limit, 200k hard cap)>",
    "所以：<100万模型 -> 200k；16万模型 -> 96k>",
    "| 模型上下文上限 | 60% 阈值 | 200k hard cap 后 | 会在多少 context token 注入 compact 要求 |",
    "|---:|---:|---:|---:|",
    "| 1,000,000 | 600,000 | 200,000 | <b>200,000</b> |",
    "| 160,000 | 96,000 | 96,000 | <b>96,000</b> |",
    "也就是说：",
    "- <b>100 万上下文模型</b>：到 <code>200k input tokens</code> 左右。",
    "- <b>16 万上下文模型</b>：到 <code>96k input tokens</code> 左右。",
    "补一句边界：<code>COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS</code> 会覆盖这个计算。",
  ].join("\n");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [textMessage("msg-compact-rich", session.id, compactAnswer)],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  for (const capabilities of [plainCapabilities(), ansiCapabilities(), richCapabilities()]) {
    const output = withTerminalSize(120, 34, () => render(state, capabilities));
    const plain = stripAnsi(output);
    assert.match(
      plain,
      /触发注入条件是：<context_tokens >= min\(60% \* model_context_limit, 200k hard cap\)>/u,
    );
    assert.match(plain, /所以：<100万模型 -> 200k；16万模型 -> 96k>/u);
    assert.match(plain, /1,000,000\s+(?:│\s+)?600,000\s+(?:│\s+)?200,000\s+(?:│\s+)?200,000/u);
    assert.match(plain, /160,000\s+(?:│\s+)?96,000\s+(?:│\s+)?96,000\s+(?:│\s+)?96,000/u);
    assert.match(plain, /100 万上下文模型/u);
    assert.match(plain, /COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS/u);
    assert.doesNotMatch(plain, /<b>|<\/b>|<code>|<\/code>/u);
  }
});

test("render supports markdown tables while keeping markdown and local paths non-clickable", () => {
  const session = sessionFixture("sess-md", "Markdown");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      textMessage(
        "msg-md",
        "sess-md",
        "*Italic* _Em_ ~~Gone~~ ==Mark== [Site](https://example.com) [Local Doc](C:/repo/docs/readme.md)\n| Item | Path |\n| --- | --- |\n| Source | C:/repo/apps/tui |\n| Docs | [README](https://example.com/readme) |",
      ),
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const plain = render(state, plainCapabilities());
  assert.match(plain, /Item\s+Path/);
  assert.match(plain, /README/);
  assert.doesNotMatch(plain, /README \(https:\/\/example\.com\/readme\)/);
  assert.doesNotMatch(plain, /Site \(https:\/\/example\.com\)/);
  assert.doesNotMatch(plain, /Local Doc \(C:\/repo\/docs\/readme\.md\)/);
  assert.doesNotMatch(plain, /\x1b\]8/);

  const rich = render(state, richCapabilities());
  const richText = stripAnsi(rich);
  assert.doesNotMatch(rich, /[┬┼┴]/u);
  assert.match(rich, /\x1b\[3mItalic\x1b\[0m/);
  assert.match(rich, /\x1b\[3mEm\x1b\[0m/);
  assert.match(rich, /\x1b\[9mGone\x1b\[0m/);
  assert.match(rich, /\x1b\[7mMark\x1b\[0m/);
  assert.match(richText, /Site/);
  assert.match(richText, /Local Doc/);
  assert.doesNotMatch(richText, /Site \(https:\/\/example\.com\)/);
  assert.doesNotMatch(richText, /Local Doc \(C:\/repo\/docs\/readme\.md\)/);
  assert.match(richText, /Item\s+│\s+Path/);
  assert.match(richText, /Source\s+│\s+C:\/repo\/apps\/tui/);
  assert.match(richText, /Item/);
  assert.match(richText, /Path/);
  assert.match(rich, /\x1b\[38;2;103;116;111m│\x1b\[38;2;217;222;205m/);
  assert.match(richText, /Source\s+│\s+C:\/repo\/apps\/tui/);
  assert.match(rich, /C:\/repo\/apps\/tui/);
  assert.doesNotMatch(rich, /\x1b\]8/u);

  const narrowRich = withTerminalSize(42, 24, () => render(state, richCapabilities()));
  assertFitsTerminal(narrowRich, 42, 24);
  assert.match(stripAnsi(narrowRich), /Source\s+│\s+C:\/repo\/apps\/tui/);
  assert.doesNotMatch(narrowRich, /[┬┼┴]/u);
  assert.doesNotMatch(narrowRich, /\x1b\]8/u);
  assert.doesNotMatch(narrowRich, /\x1b\[4m/u);
});

test("render resolves relative paths with spaces and links missing local media", () => {
  const workspace = mkdtempSync(path.join(tmpdir(), "tura tui paths "));
  try {
    mkdirSync(path.join(workspace, "shots"), { recursive: true });
    writeFileSync(path.join(workspace, "shots", "final image.png"), "not really an image");
    const finalMediaPath = path.join(workspace, "shots", "final image.png").replace(/\\/g, "/");
    const missingMediaPath = path.join(workspace, "shots", "missing image.png").replace(/\\/g, "/");
    const session = sessionFixture("sess-local-media", "Local Media", "idle", {
      directory: workspace,
    });
    const state = reducer(initialState(workspace), {
      type: "hydrate",
      session,
      messages: [
        textMessage(
          "msg-local-media",
          session.id,
          [
            "[Local Shot](shots/final image.png)",
            "[MEDIA:shots/final image.png:MEDIA]",
            "[MEDIA:shots/missing image.png:MEDIA]",
          ].join("\n"),
        ),
      ],
      permissions: [],
      providers: { all: [], default: {}, connected: [], enums: providerEnums },
      sessions: [session],
    });

    const transcript = render(state, richCapabilities());
    assert.match(transcript, /Local Shot/);
    assert.match(stripAnsi(transcript), new RegExp(escapeRegExp(finalMediaPath), "u"));
    assert.match(stripAnsi(transcript), new RegExp(escapeRegExp(missingMediaPath), "u"));
    assert.match(
      transcript,
      /\x1b\[48;5;236m\x1b\[38;2;217;222;205m [^\x1b]*shots\/final image\.png \x1b\[0m/u,
    );
    assert.doesNotMatch(transcript, /\x1b\]8/u);
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});

test("render keeps spaces inside local media and directory text without creating links", () => {
  const workspace = mkdtempSync(path.join(tmpdir(), "tura tui local links "));
  try {
    const mediaDir = path.join(workspace, "Project Files", "Agent Media");
    const rawDir = path.join(workspace, "Project Files", "Raw Directory");
    mkdirSync(mediaDir, { recursive: true });
    mkdirSync(rawDir, { recursive: true });
    writeFileSync(path.join(mediaDir, "shot final.png"), "not really an image");

    const absoluteRawDir = rawDir.replace(/\\/g, "/");
    const absoluteMediaPath = path.join(mediaDir, "shot final.png").replace(/\\/g, "/");
    const absoluteMissingMediaPath = path.join(mediaDir, "missing final.png").replace(/\\/g, "/");
    const session = sessionFixture("sess-local-link-boundaries", "Local Link Boundaries", "idle", {
      directory: workspace,
    });
    const state = reducer(initialState(workspace), {
      type: "hydrate",
      session,
      messages: [
        textMessage(
          "msg-local-link-boundaries",
          session.id,
          [
            "Agent local links:",
            `raw directory ${absoluteRawDir} and then plain words`,
            "relative directory Project Files/Docs (review), then a comma clause",
            "wrapped directory (Project Files/Docs (review)) after wrapper",
            "markdown directory [Review Folder](Project Files/Docs (review))",
            "[MEDIA:Project Files/Agent Media/shot final.png:MEDIA]",
            "[MEDIA:Project Files/Agent Media/missing final.png:MEDIA]",
          ].join("\n"),
        ),
      ],
      permissions: [],
      providers: { all: [], default: {}, connected: [], enums: providerEnums },
      sessions: [session],
    });

    const transcript = withTerminalSize(140, 32, () => render(state, richCapabilities()));
    const text = stripAnsi(transcript);

    assert.match(text, /Project Files\/Docs \(review\)/u);
    assert.match(text, new RegExp(escapeRegExp(absoluteMediaPath), "u"));
    assert.match(text, new RegExp(escapeRegExp(absoluteMissingMediaPath), "u"));
    assert.match(text.replace(/[▏\s]+/gu, ""), /RawDirectoryandthenplainwords/u);
    assert.match(text, /Review Folder/u);
    assert.doesNotMatch(transcript, /\x1b\]8/u);
    assert.match(
      transcript,
      /\x1b\[48;5;236m\x1b\[38;2;217;222;205m [^\x1b]*Project Files\/Agent Media\/shot final\.png \x1b\[0m/u,
    );
    assert.doesNotMatch(transcript, /\[MEDIA:/u);
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});

test("render wraps wide markdown table cells without dropping data or breaking columns", () => {
  const session = sessionFixture("sess-wide-table", "Wide Table");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      textMessage(
        "msg-wide-table",
        session.id,
        [
          "BEFORE_TABLE_MARKER",
          "| 项目名称 | 负责人 | 当前进展 | 主要风险 | 下一步行动 |",
          "| --- | --- | --- | --- | --- |",
          "| 智能客服系统升级项目 | 李晶 | 已完成意图识别模块重构，正在灰度发布新的多轮对话能力并持续观测线上指标 | 历史会话数据格式不统一，可能导致部分老用户上下文恢复异常 | 继续观察灰度指标，收集异常日志，并准备回滚脚本和补偿数据校验流程 |",
          "| 跨境订单履约优化项目 | 王芳 | 仓储路由策略已经接入测试环境，目前正在验证不同国家地区的拆单策略 | 第三方物流接口响应时间波动较大，高峰期可能影响履约时效 | 增加接口超时监控，完善重试机制，并与物流供应商确认扩容窗口 |",
          "| 数据看板性能治理项目 | 张伟 | 核心报表查询从分钟级优化到秒级，但部分自定义筛选仍然存在慢查询 | 复杂筛选组合会触发全表扫描，如果用户频繁刷新会造成数据库压力 | 为高频筛选字段补充索引，限制过重查询，并设计异步导出兜底 |",
          "AFTER_TABLE_MARKER",
        ].join("\n"),
      ),
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const output = withTerminalSize(88, 80, () => render(state, richCapabilities()));
  const plainLines = stripAnsi(output).split("\n");
  const headerIndex = plainLines.findIndex((line) => /项目名称\s+│\s+负责人/u.test(line));
  const afterIndex = plainLines.findIndex((line) => line.includes("AFTER_TABLE_MARKER"));
  assert.ok(headerIndex >= 0);
  assert.ok(afterIndex > headerIndex);
  const tableRegion = plainLines.slice(headerIndex, afterIndex);
  const tableRows = tableRegion.filter((line) => line.includes("│"));

  assert.ok(tableRows.length > 4, tableRegion.join("\n"));
  assert.equal(
    tableRegion.filter((line) => line.trim() && !line.includes("│") && !/^▏\s*$/u.test(line))
      .length,
    0,
    tableRegion.join("\n"),
  );
  assert.ok(
    tableRows.every((line) => (line.match(/│/gu) ?? []).length === 4),
    `wrapped table lines should keep five columns:\n${tableRows.join("\n")}`,
  );
  const tableInterior = tableRegion.filter((line) => !/^▏\s*$/u.test(line));
  assert.ok(
    tableInterior.every((line) => (line.match(/│/gu) ?? []).length === 4),
    `wrapped table lines should preserve column separators:\n${tableRegion.join("\n")}`,
  );
  const spacerRows = tableInterior.filter(
    (line) => line.includes("│") && !/[\p{L}\p{N}]/u.test(line.replace(/[│▏]/gu, "")),
  );
  assert.equal(
    spacerRows.length,
    3,
    `table renderer must insert one separator-preserving blank row between table rows:\n${tableRegion.join("\n")}`,
  );
  const nextActionColumn = tableRows
    .map((line) => line.split("│")[4] ?? "")
    .join("")
    .replace(/[^\p{Script=Han}]/gu, "");
  assert.doesNotMatch(tableRegion.join("\n"), /\.\.\.|…/u);
  assert.match(nextActionColumn, /补偿数据校验流程/u);
  assert.match(nextActionColumn, /确认扩容窗口/u);
  assert.match(nextActionColumn, /异步导出兜底/u);
  assertLineWidths(output, 88);
});

test("render keeps inline rich text scoped to wrapped markdown table cells", () => {
  const output = renderRichText(
    [
      "| 命令 | 结果 |",
      "| --- | --- |",
      "| `very-long-command-with-inline-rich-style-that-wraps` | 通过 |",
    ].join("\n"),
    { tableWidth: 34 },
  );
  const lines = output.split("\n").filter((line) => stripAnsi(line).includes("│"));
  const dataLines = lines.filter(
    (line) => stripAnsi(line).includes("通过") || stripAnsi(line).includes("rich-style"),
  );

  assert.ok(dataLines.length >= 2, output);

  const firstDataLine = dataLines.find((line) => stripAnsi(line).includes("通过"));
  assert.ok(firstDataLine, output);
  const [firstCell = "", secondCell = ""] = firstDataLine.split("│");
  assert.match(firstCell, /\x1b\[48;5;236m/u);
  assert.doesNotMatch(secondCell, /\x1b\[48;5;236m/u);

  const wrappedCommandLine = dataLines.find((line) => stripAnsi(line).includes("rich-style"));
  assert.ok(wrappedCommandLine, output);
  const [wrappedFirstCell = "", wrappedSecondCell = ""] = wrappedCommandLine.split("│");
  assert.match(wrappedFirstCell, /\x1b\[48;5;236m/u);
  assert.doesNotMatch(wrappedSecondCell, /\x1b\[48;5;236m/u);
});

test("render preserves rich text blank paragraphs and full-width code block backgrounds", () => {
  const session = sessionFixture("sess-rich-blocks", "Rich Blocks");
  const state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [
      textMessage(
        "msg-rich-blocks",
        session.id,
        "Paragraph before blank.\n\nParagraph after blank.\n> Quoted without a rail\n```ts\nconst width = 'full message text area';\n```\n| Kind | Result |\n| --- | --- |\n| Table | compact row |",
      ),
    ],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    sessions: [session],
  });

  const rich = withTerminalSize(96, 28, () => render(state, richCapabilities()));
  const stripped = stripAnsi(rich);
  const lines = stripped.split("\n");
  const before = lines.findIndex((line) => line.includes("Paragraph before blank."));
  const after = lines.findIndex((line) => line.includes("Paragraph after blank."));
  assert.ok(before >= 0 && after > before + 1, "blank paragraph line should remain in-message");
  assert.ok(lines.slice(before + 1, after).some((line) => /^▏\s*$/u.test(line)));
  assert.match(stripped, /Quoted without a rail/);
  assert.doesNotMatch(stripped, /│ Quoted without a rail/);
  assert.match(stripped, /Kind\s+│\s+Result/);
  assert.match(stripped, /Table\s+│\s+compact row/);
  const quoteIndex = lines.findIndex((line) => line.includes("Quoted without a rail"));
  const codeLineIndex = lines.findIndex((line) => line.includes("const width"));
  const tableHeaderIndex = lines.findIndex((line) => /Kind\s+│\s+Result/u.test(line));
  const tableRowIndex = lines.findIndex((line) => /Table\s+│\s+compact row/u.test(line));
  assert.ok(quoteIndex >= 0 && codeLineIndex > quoteIndex);
  assert.ok(tableHeaderIndex > codeLineIndex);
  assert.ok(tableRowIndex > tableHeaderIndex);
  assert.equal(tableRowIndex, tableHeaderIndex + 2);
  assert.match(lines[tableHeaderIndex + 1] ?? "", /│/u);
  assert.doesNotMatch(lines[tableHeaderIndex + 1] ?? "", /[\p{L}\p{N}]/u);
  assert.ok(lines.slice(quoteIndex + 1, codeLineIndex).some((line) => /^▏\s*$/u.test(line)));
  assert.ok(lines.slice(codeLineIndex + 1, tableHeaderIndex).some((line) => /^▏\s*$/u.test(line)));
  assert.doesNotMatch(stripped, /```(?:ts)?/);

  const rawCodeLines = rich.split("\n");
  const codeLine = rawCodeLines.find((line) => stripAnsi(line).includes("const width"));
  assert.ok(codeLine);
  assert.ok(codeLine.includes("\x1b[48;5;234m"));
  assert.ok(
    visibleTextWidth(codeLine) >= 94,
    `code block background should fill the rich message text area: ${visibleTextWidth(codeLine)}`,
  );
  const rawCodeLineIndex = rawCodeLines.findIndex((line) =>
    stripAnsi(line).includes("const width"),
  );
  const codeTopBlank = rawCodeLines[rawCodeLineIndex - 1] ?? "";
  const codeBottomBlank = rawCodeLines[rawCodeLineIndex + 1] ?? "";
  assert.match(stripAnsi(codeTopBlank), /^▏\s*$/u);
  assert.match(stripAnsi(codeBottomBlank), /^▏\s*$/u);
  assert.ok(codeTopBlank.includes("\x1b[48;5;234m"));
  assert.ok(codeBottomBlank.includes("\x1b[48;5;234m"));
  assert.ok(visibleTextWidth(codeTopBlank) >= 94);
  assert.ok(visibleTextWidth(codeBottomBlank) >= 94);
});

test("render shows agent persona summary and persona panel", () => {
  const session = sessionFixture("sess-persona", "Persona", "idle", { agent: "fast" });
  let state = reducer(initialState("C:/repo"), {
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: { all: [], default: {}, connected: [], enums: providerEnums },
    agents: [
      {
        summary: {
          id: "fast",
          name: "Fast",
          description: "fast agent",
          source: "static",
          path: "agents/src/fast",
          aliases: [],
          capabilities: ["chat"],
          hidden: false,
        },
        config: {
          agent_name: "fast",
        },
        prompt: "Fast prompt",
      },
    ],
    personas: [
      {
        summary: {
          id: "tura",
          source: "static",
          description: "calm technical collaborator",
          path: "personas/src/tura",
        },
        config: { persona_name: "tura" },
        communication_style: "concise, direct, friendly",
      },
      {
        summary: {
          id: "reviewer",
          source: "dynamic",
          description: "review-first mode",
          path: "personas/src/reviewer",
        },
        config: { persona_name: "reviewer" },
      },
    ],
    sessions: [session],
    sessionConfig: { active_agent: "fast" },
  });
  const top = render(state, richCapabilities());
  assert.doesNotMatch(top, /Agent:.*fast/);
  assert.doesNotMatch(top, /persona:.*tura/);
  assert.match(top, /Tab: sessions/);
  assert.match(top, /\/stop: cancel/);
  assert.doesNotMatch(top, /↑\/↓ view sessions/);
  assert.doesNotMatch(top, /[┌┐└┘]/u);
  assert.match(top, /^\x1b\[48;2;20;23;24m\x1b\[38;2;244;247;235m▏\x1b\[0m/m);
  assert.match(top, /^\x1b\[48;2;20;23;24m\x1b\[38;2;244;247;235m▏\x1b\[0m.*Enter: send/m);
  assert.doesNotMatch(top, /\x1b\[38;2;64;224;208m█\x1b\[0m/);
  assert.match(stripAnsi(top), /fast\s+│\s+tura/);

  state = reducer(state, { type: "toggle-personas" });
  const panel = render(state, richCapabilities());
  assert.match(panel, /Personas/);
  assert.match(panel, /> tura/);
  assert.match(panel, /\x1b\[48;2;20;23;24m/);
  assert.match(panel, /tura/);
  assert.match(panel, /Balanced and energetic/);
  assert.match(stripAnsi(panel), /concise, direct, fri/u);
  assert.match(stripAnsi(panel), /concise, direct, friendly/u);
  const personaLine = panel.split("\n").find((line) => stripAnsi(line).includes("> tura"));
  assert.ok(personaLine);
  assertWideMenuGap(personaLine, "tura", "current");
});
