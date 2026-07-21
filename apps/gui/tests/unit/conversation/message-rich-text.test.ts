import { describe, expect, test } from "bun:test";
import {
  reactionEmojiValues,
  stickerEmojiValues,
  stripEmojiDirectives,
} from "../../../app/src/conversation/message-rich-protocol";
import {
  localPathQueryValue,
  mediaSource,
} from "../../../app/src/conversation/message-rich-text-paths";
import { setLanguage } from "../../../app/src/i18n";
import { partText } from "../../../app/src/state/global-store";

describe("message rich text media paths", () => {
  test("normalizes file URLs and includes workspace directory in media requests", () => {
    expect(localPathQueryValue("file:///C:/tmp/My%20File.png")).toBe("C:/tmp/My File.png");
    expect(mediaSource("shots/final image.png", "C:/repo with space")).toBe(
      "/file/media?path=shots%2Ffinal+image.png&directory=C%3A%2Frepo+with+space",
    );
  });

  test("uses the connected gateway for Tauri media", () => {
    expect(mediaSource(".tura/media/input/shot.png", "C:/repo", "http://127.0.0.1:4217")).toBe(
      "http://127.0.0.1:4217/file/media?path=.tura%2Fmedia%2Finput%2Fshot.png&directory=C%3A%2Frepo",
    );
  });
});

describe("message text localization", () => {
  test("localizes structured runtime stopped assistant parts", () => {
    setLanguage("zh-CN");
    expect(
      partText({
        text: "MANO failed while processing this prompt: one-shot worker cancelled",
        metadata: { kind: "runtime_status", code: "runtime_stopped" },
      }),
    ).toBe("Runtime 已停止。");
    setLanguage("en");
    expect(
      partText({
        text: "MANO failed while processing this prompt: one-shot worker cancelled",
        metadata: { kind: "runtime_status", code: "runtime_stopped" },
      }),
    ).toBe("Runtime stopped.");
    setLanguage(undefined);
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
