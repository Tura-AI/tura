import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const appSource = readFileSync(resolve(import.meta.dir, "../../../app/src/app.tsx"), "utf8");
const outletSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/app/conversation-page-outlet.tsx"),
  "utf8",
);
const conversationSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/conversation/conversation-view.tsx"),
  "utf8",
);

function asyncFunctionBlock(source: string, name: string): string {
  const start = source.indexOf(`async function ${name}`);
  expect(start).toBeGreaterThanOrEqual(0);
  const next = source.indexOf("\n  async function", start + 1);
  return source.slice(start, next > start ? next : undefined);
}

describe("conversation submit scroll behavior", () => {
  test("direct prompt submission requests a bottom scroll with the optimistic user message", () => {
    const submitDirectPrompt = asyncFunctionBlock(appSource, "submitDirectPrompt");
    const messageInsert = submitDirectPrompt.indexOf("messagesBySession");
    const scrollRequest = submitDirectPrompt.indexOf("transcriptScrollToBottomRequest");
    const composerClear = submitDirectPrompt.indexOf('composerText: ""');

    expect(scrollRequest).toBeGreaterThan(messageInsert);
    expect(scrollRequest).toBeLessThan(composerClear);
    expect(submitDirectPrompt).toContain("sessionId: session.id");
    expect(submitDirectPrompt).toContain(
      "token: (previous.transcriptScrollToBottomRequest?.token ?? 0) + 1",
    );
  });

  test("conversation view consumes the submit scroll request without changing pinned-only live follow", () => {
    expect(outletSource).toContain("scrollToBottomToken={");
    expect(outletSource).toContain("onScrollToBottomRequestConsumed");
    expect(outletSource).toContain("transcriptScrollToBottomRequest: undefined");
    expect(conversationSource).toContain("scrollToBottomToken?: number");
    expect(conversationSource).toContain("props.scrollToBottomToken ?? 0");
    expect(conversationSource).toContain('scrollTranscriptToBottom("auto")');
    expect(conversationSource).toContain("props.onScrollToBottomRequestConsumed?.(token)");
    expect(conversationSource).toContain("if (transcriptPinned())");
  });
});
