import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const conversationViewSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/conversation/conversation-view.tsx"),
  "utf8",
);

const animationCss = readFileSync(
  resolve(import.meta.dir, "../../../app/src/styles/parts/conversation/animations-schedule.css"),
  "utf8",
);

describe("assistant thinking text animation", () => {
  test("renders the thinking icon as part of the animated text part", () => {
    expect(conversationViewSource).toContain("ASSISTANT_THINKING_TEXT_ICON");
    expect(conversationViewSource).toContain('class="assistant-thinking-glyph"');
    expect(conversationViewSource).toContain(
      "const assistantThinkingPart = createMemo<MessagePart>",
    );
    expect(conversationViewSource).toContain(
      'class="assistant-text-block assistant-thinking-text"',
    );
    expect(conversationViewSource).toContain("part={assistantThinkingPart()}");
    expect(conversationViewSource).not.toContain("AssistantThinkingIndicator");
    expect(conversationViewSource).not.toContain("assistant-thinking-icon");
    expect(conversationViewSource).not.toContain("TUI_THINKING_ICON_FRAMES");
  });

  test("animates the full rich text instead of a separate icon element", () => {
    expect(animationCss).toContain(".assistant-thinking-text .rich-text");
    expect(animationCss).toContain("animation: assistant-thinking-drift 5s ease-in-out infinite;");
    expect(animationCss).toContain(".assistant-thinking-text .assistant-thinking-glyph");
    expect(animationCss).toContain(
      "animation: assistant-thinking-glyph-pulse 1.2s ease-in-out infinite;",
    );
    expect(animationCss).not.toContain(".assistant-thinking-icon");
  });
});
