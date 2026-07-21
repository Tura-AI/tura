import { describe, expect, test } from "bun:test";
import { normalizeEnglishPunctuation } from "../../../app/src/conversation/message-punctuation";

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
});
