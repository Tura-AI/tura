import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  reactionEmojiValues,
  stickerEmojiValues,
  stripEmojiDirectives,
} from "../../../app/src/conversation/message-rich-protocol";
import {
  localPathQueryValue,
  mediaSource,
} from "../../../app/src/conversation/message-rich-text-paths";

const richTextSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/conversation/message-rich-text.tsx"),
  "utf8",
);

describe("message rich text media paths", () => {
  test("normalizes file URLs and includes workspace directory in media requests", () => {
    expect(localPathQueryValue("file:///C:/tmp/My%20File.png")).toBe("C:/tmp/My File.png");
    expect(mediaSource("shots/final image.png", "C:/repo with space")).toBe(
      "/file/media?path=shots%2Ffinal+image.png&directory=C%3A%2Frepo+with+space",
    );
  });
});

describe("message rich text emoji protocol directives", () => {
  test("extracts reactions and stickers without leaving protocol text in display content", () => {
    const source = "hello [EMOJI:react:👍:EMOJI]\n[EMOJI:sticker:😂:EMOJI] done";

    expect(reactionEmojiValues(source)).toEqual(["👍"]);
    expect(stickerEmojiValues(source)).toEqual(["😂"]);
    expect(stripEmojiDirectives(source)).toBe("hello \n done");
  });

  test("routes sticker directives through the rich text parser instead of dropping them", () => {
    expect(richTextSource).toContain(
      '| { kind: "emoji"; variant: "sticker" | "react"; value: string }',
    );
    expect(richTextSource).toContain('match[3] === "sticker" || match[3] === "react"');
    expect(richTextSource).toContain("class={`rich-emoji rich-${props.node.variant}`}");
  });
});

describe("message rich text paragraph layout", () => {
  test("uses TUI-style HTML block normalization instead of paragraph nodes", () => {
    expect(richTextSource).toContain("function normalizeHtmlBlockBreaks");
    expect(richTextSource).toContain('.replace(/<br\\s*\\/?>/giu, "\\n")');
    expect(richTextSource).toContain("address|article|aside|details|div");
    expect(richTextSource).not.toContain('| "paragraph"');
    expect(richTextSource).not.toContain('class="rich-paragraph"');
  });

  test("normalizes p tags through the same block-tag rule as div tags", () => {
    expect(richTextSource).toContain("|p|section|summary|ul");
  });
});

describe("message rich text table cells", () => {
  test("routes Markdown table cells through the inline rich parser", () => {
    expect(richTextSource).toContain("function parseInlineRichText");
    expect(richTextSource).toContain("children: parseInlineRichText(cell.trim())");
    expect(richTextSource).not.toContain("children: splitInlineTextReferences(cell.trim())");
  });

  test("sizes table cells from text length while leaving full content visible", () => {
    expect(richTextSource).toContain("const TABLE_CELL_MAX_CH = 96;");
    expect(richTextSource).toContain("function tableCellWidthStyle");
    expect(richTextSource).toContain('class="rich-table-cell-content"');
    expect(richTextSource).toContain('"--rich-table-cell-width"');
  });

  test("keeps compaction threshold explanation visible around inline HTML and angle brackets", () => {
    const text = [
      "按现在代码逻辑：",
      "触发注入条件是：<context_tokens >= min(60% * model_context_limit, 200k hard cap)>",
      "所以：<100万模型 -> 200k；16万模型 -> 96k>",
      "| 模型上下文上限 | 60% 阈值 | 200k hard cap 后 | 会在多少 context token 注入 compact 要求 |",
      "|---:|---:|---:|---:|",
      "| 1,000,000 | 600,000 | 200,000 | <b>200,000</b> |",
      "| 160,000 | 96,000 | 96,000 | <b>96,000</b> |",
      "- <b>100 万上下文模型</b>：到 <code>200k input tokens</code> 左右。",
      "补一句边界：<code>COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS</code> 会覆盖这个计算。",
    ].join("\n");

    expect(richTextSource).toContain("function preserveUnknownAngleBrackets");
    expect(richTextSource).toContain("const SUPPORTED_HTML_TAGS = new Set");
    expect(richTextSource).toContain("? match : escapeHtml(match)");
    expect(richTextSource).toContain("nodes.push(...parseInlineRichText(text));");
    expect(text).toContain(
      "触发注入条件是：<context_tokens >= min(60% * model_context_limit, 200k hard cap)>",
    );
    expect(text).toContain("所以：<100万模型 -> 200k；16万模型 -> 96k>");
  });
});

describe("message rich text scrollbars", () => {
  test("removes native scrollbar arrow buttons from rich code blocks", () => {
    const richContentCss = readFileSync(
      resolve(import.meta.dir, "../../../app/src/styles/parts/layout/rich-content.css"),
      "utf8",
    );

    expect(richContentCss).toContain(".rich-text pre::-webkit-scrollbar-button");
    expect(richContentCss).toContain("display: none;");
  });

  test("keeps rich text hover states from changing text color", () => {
    const richContentCss = readFileSync(
      resolve(import.meta.dir, "../../../app/src/styles/parts/layout/rich-content.css"),
      "utf8",
    );

    expect(richContentCss).toContain(".rich-text a");
    expect(richContentCss).toContain("color: inherit;");
    expect(richContentCss).not.toContain(".rich-local-path");
    expect(richContentCss).not.toContain(".rich-spoiler:hover");
  });

  test("does not render the extra rectangular file icon on attachment tiles", () => {
    expect(richTextSource).not.toContain('class="rich-file-icon"');
  });
});

describe("message rich text thinking glyph", () => {
  test("keeps the thinking glyph as an allowed inline rich-text span", () => {
    expect(richTextSource).toContain('| "thinkingGlyph"');
    expect(richTextSource).toContain('class="assistant-thinking-glyph"');
    expect(richTextSource).toContain('element.classList.contains("assistant-thinking-glyph")');
  });
});
