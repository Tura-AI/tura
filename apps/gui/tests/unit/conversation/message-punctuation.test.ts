import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { normalizeEnglishPunctuation } from "../../../app/src/conversation/message-punctuation";

const richTextSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/conversation/message-rich-text.tsx"),
  "utf8",
);
const textPartsSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/conversation/message-text-parts.tsx"),
  "utf8",
);

describe("assistant message punctuation", () => {
  test("normalizes smart apostrophes and fullwidth punctuation on English lines", () => {
    expect(normalizeEnglishPunctuation("hy I’m here， are you ready？")).toBe(
      "hy I'm here, are you ready?",
    );
    expect(normalizeEnglishPunctuation("‘Hello’ — “world”！")).toBe("'Hello' — \"world\"!");
  });

  test("normalizes contractions without rewriting Chinese punctuation", () => {
    expect(normalizeEnglishPunctuation("示例：hy I’m here。")).toBe("示例：hy I'm here。");
    expect(normalizeEnglishPunctuation("示例：👋 I’m here。")).toBe("示例：👋 I'm here。");
    expect(normalizeEnglishPunctuation("示例：hy I’m here， okay？")).toBe(
      "示例：hy I'm here, okay？",
    );
    expect(normalizeEnglishPunctuation("他说：“你好，我很好。”")).toBe("他说：“你好，我很好。”");
  });

  test("is idempotent for already normalized English text", () => {
    const text = "hy I'm here, are you ready?";
    expect(normalizeEnglishPunctuation(normalizeEnglishPunctuation(text))).toBe(text);
  });

  test("applies normalization only to assistant prose and preserves code nodes", () => {
    expect(textPartsSource).toContain('normalizePunctuation={props.role === "assistant"}');
    expect(richTextSource).toContain('node.tag === "code" || node.tag === "pre"');
  });
});
