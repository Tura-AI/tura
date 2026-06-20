import { describe, expect, test } from "bun:test";
import {
  reactionEmojiValues,
  stickerEmojiValues,
  stripEmojiDirectives,
} from "../../../app/src/conversation/message-rich-protocol";
import {
  localPathQueryValue,
  mediaSource,
  parseLocalTextReferences,
} from "../../../app/src/conversation/message-rich-text-paths";

describe("message rich text local path parsing", () => {
  test("parses relative, absolute, and spaced local markdown links", () => {
    const nodes = parseLocalTextReferences(
      [
        "[Relative](docs/My File.md)",
        "[Absolute](C:/tmp/My File.png)",
        "raw ./shots/final image.png.",
      ].join("\n"),
    );
    const locals = nodes.filter((node) => node.kind === "local-path");

    expect(locals.map((node) => node.path)).toEqual([
      "docs/My File.md",
      "C:/tmp/My File.png",
      "./shots/final image.png",
    ]);
    expect(locals.map((node) => node.label)).toEqual(["Relative", "Absolute", undefined]);
  });

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
});
