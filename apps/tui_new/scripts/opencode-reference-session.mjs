#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const referenceSessionID = "ses_tura_reference";

const baseTime = Date.parse("2026-06-07T10:00:00.000Z");
const userID = "msg_tura_reference_001_user";
const assistantID = "msg_tura_reference_002_assistant";

function textPart(id, messageID, text) {
  return {
    id,
    sessionID: referenceSessionID,
    messageID,
    type: "text",
    text,
    time: { start: baseTime + 100, end: baseTime + 200 },
  };
}

function completedTool(id, callID, tool, input, metadata = {}) {
  return {
    id,
    sessionID: referenceSessionID,
    messageID: assistantID,
    type: "tool",
    callID,
    tool,
    state: {
      status: "completed",
      input,
      output: "",
      title: "",
      metadata,
      time: { start: baseTime + 300, end: baseTime + 450 },
    },
  };
}

function runningTool(id, callID, tool, input = {}) {
  return {
    id,
    sessionID: referenceSessionID,
    messageID: assistantID,
    type: "tool",
    callID,
    tool,
    state: {
      status: "running",
      input,
      title: "Asking questions...",
      metadata: {},
      time: { start: baseTime + 900 },
    },
  };
}

export function makeReferenceSession(directory = "C:\\Users\\liuliu\\Documents\\opencode-dev") {
  const model = { providerID: "opencode", modelID: "claude-opus-4-5" };
  return {
    info: {
      id: referenceSessionID,
      slug: "homepage-button-color-change-in-repo-workflow",
      projectID: "global",
      directory,
      path: ".",
      title: "Homepage button color change in repo workflow",
      agent: "build",
      model: { providerID: model.providerID, id: model.modelID },
      version: "0.0.0-tui-new-reference",
      cost: 0.29,
      tokens: {
        input: 39413,
        output: 2100,
        reasoning: 0,
        cache: { read: 0, write: 0 },
      },
      time: { created: baseTime, updated: baseTime + 1200 },
    },
    messages: [
      {
        info: {
          id: userID,
          sessionID: referenceSessionID,
          role: "user",
          time: { created: baseTime },
          agent: "build",
          model,
        },
        parts: [textPart("prt_tura_reference_001_user_text", userID, "Find the homepage button and make it blue")],
      },
      {
        info: {
          id: assistantID,
          sessionID: referenceSessionID,
          role: "assistant",
          time: { created: baseTime + 100, completed: baseTime + 1100 },
          parentID: userID,
          modelID: model.modelID,
          providerID: model.providerID,
          mode: "build",
          agent: "build",
          path: { cwd: directory, root: directory },
          cost: 0.29,
          tokens: {
            input: 39413,
            output: 2100,
            reasoning: 0,
            cache: { read: 0, write: 0 },
          },
        },
        parts: [
          textPart("prt_tura_reference_002_assistant_intro", assistantID, "I'll search for the homepage button in the codebase."),
          completedTool("prt_tura_reference_003_grep_homepage", "call_tura_reference_grep_homepage", "grep", {
            pattern: "homepage|home.*button|Home.*button",
          }),
          completedTool("prt_tura_reference_004_grep_title", "call_tura_reference_grep_title", "grep", {
            pattern: "Homepage",
          }),
          textPart(
            "prt_tura_reference_005_assistant_broad",
            assistantID,
            "Let me search more broadly for buttons and home-related elements:",
          ),
          completedTool("prt_tura_reference_006_grep_home", "call_tura_reference_grep_home", "grep", {
            pattern: "Home",
          }),
          completedTool("prt_tura_reference_007_grep_tsx", "call_tura_reference_grep_tsx", "grep", {
            pattern: "button",
            include: "*.tsx",
          }),
          textPart(
            "prt_tura_reference_008_assistant_candidates",
            assistantID,
            'I found several "Home" links. Let me look at the most likely candidates - the 404 pages and the header component where the "Home" links:',
          ),
          completedTool(
            "prt_tura_reference_009_read_console_404",
            "call_tura_reference_read_console_404",
            "read",
            { filePath: "packages/console/app/src/routes/[...404].tsx" },
            { loaded: ["packages/console/app/src/routes/[...404].tsx"] },
          ),
          completedTool(
            "prt_tura_reference_010_read_console_header",
            "call_tura_reference_read_console_header",
            "read",
            { filePath: "packages/console/app/src/component/header.tsx" },
            { loaded: ["packages/console/app/src/component/header.tsx"] },
          ),
          completedTool(
            "prt_tura_reference_011_read_enterprise_404",
            "call_tura_reference_read_enterprise_404",
            "read",
            { filePath: "packages/enterprise/src/routes/[...404].tsx" },
            { loaded: ["packages/enterprise/src/routes/[...404].tsx"] },
          ),
          textPart(
            "prt_tura_reference_012_assistant_question",
            assistantID,
            'I found "Home" buttons/links in multiple locations. Let me ask which one you want to modify:',
          ),
          runningTool("prt_tura_reference_013_question", "call_tura_reference_question", "question"),
        ],
      },
    ],
  };
}

export async function writeReferenceSession(file, directory) {
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, JSON.stringify(makeReferenceSession(directory), null, 2));
  return file;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  const output = process.argv[2] || path.resolve("target", "tui-new-reference-session.json");
  await writeReferenceSession(output, process.argv[3]);
  console.log(output);
}
