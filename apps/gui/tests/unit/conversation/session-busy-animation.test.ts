import type { Message, Session } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import {
  messagesWithSessionThinking,
  sessionShowsBusyAnimation,
} from "../../../app/src/conversation/session-animation";

function session(status: Session["status"]): Session {
  return {
    id: `session-${status}`,
    status,
    updated_at: 10,
  };
}

function userMessage(): Message {
  return {
    id: "user-1",
    sessionID: "s1",
    role: "user",
    created_at: 1,
    updated_at: 1,
    time: { created: 1, updated: 1 },
    parts: [
      {
        id: "user-1:text",
        sessionID: "s1",
        messageID: "user-1",
        type: "text",
        text: "run it",
      },
    ],
  };
}

function assistantRuntimeRunningMessage(): Message {
  return {
    id: "assistant-runtime",
    sessionID: "s1",
    role: "assistant",
    created_at: 2,
    updated_at: 2,
    time: { created: 2, updated: 2 },
    parts: [
      {
        id: "runtime",
        sessionID: "s1",
        messageID: "assistant-runtime",
        type: "tool",
        tool: "runtime",
        state: { status: "running" },
      },
    ],
  };
}

describe("session busy animation state", () => {
  test("shows status animation only while the corresponding session is busy", () => {
    expect(sessionShowsBusyAnimation("busy")).toBe(true);
    expect(sessionShowsBusyAnimation("idle")).toBe(false);
    expect(sessionShowsBusyAnimation("error")).toBe(false);
    expect(sessionShowsBusyAnimation(undefined)).toBe(false);
  });

  test("adds the thinking animation message for busy sessions only", () => {
    const messages = [userMessage()];

    expect(messagesWithSessionThinking(messages, session("busy")).map((item) => item.id)).toEqual([
      "user-1",
      "session-thinking:session-busy",
    ]);
    expect(messagesWithSessionThinking(messages, session("idle")).map((item) => item.id)).toEqual([
      "user-1",
    ]);
    expect(messagesWithSessionThinking(messages, session("error")).map((item) => item.id)).toEqual([
      "user-1",
    ]);
  });

  test("does not start the status animation from runtime tool state", () => {
    const messages = [userMessage(), assistantRuntimeRunningMessage()];

    expect(messagesWithSessionThinking(messages, session("idle")).map((item) => item.id)).toEqual([
      "user-1",
      "assistant-runtime",
    ]);
  });

  test("does not start thinking from a stale doing task on an idle session", () => {
    const idleWithStaleTask: Session = {
      ...session("idle"),
      task_management: {
        status: "doing",
        task_summary: "Already summarized",
      },
    };

    expect(messagesWithSessionThinking([userMessage()], idleWithStaleTask).map((item) => item.id)).toEqual([
      "user-1",
    ]);
  });
});
