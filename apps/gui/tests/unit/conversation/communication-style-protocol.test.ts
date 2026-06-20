import type { Message } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import { avatarExpressionIdsForEmoji } from "../../../app/src/components/avatar/agent-avatar-protocol";
import {
  conversationReactionItems,
  latestSticker,
} from "../../../app/src/conversation/conversation-protocol";

function textMessage(id: string, role: Message["role"], text: string): Message {
  return {
    id,
    sessionID: "s1",
    role,
    parts: [
      {
        id: `${id}:text`,
        sessionID: "s1",
        messageID: id,
        type: "text",
        text,
      },
    ],
  };
}

describe("communication style protocol directives", () => {
  test("pins assistant reactions to the latest user message even when text is present", () => {
    const user = textMessage("user-1", "user", "can you check this?");
    const assistant = textMessage(
      "assistant-1",
      "assistant",
      "Yes, taking a look. [EMOJI:react:👍:EMOJI]",
    );

    const items = conversationReactionItems([user, assistant]);

    expect(items.map((item) => item.message.id)).toEqual(["user-1", "assistant-1"]);
    expect(items[0]?.reactions).toEqual(["👍"]);
    expect(items[1]?.reactions).toEqual([]);
  });

  test("drops pure reaction assistant messages after applying them to the user", () => {
    const user = textMessage("user-1", "user", "nice");
    const reaction = textMessage("assistant-reaction", "assistant", "[EMOJI:react:✨:EMOJI]");

    const items = conversationReactionItems([user, reaction]);

    expect(items.map((item) => item.message.id)).toEqual(["user-1"]);
    expect(items[0]?.reactions).toEqual(["✨"]);
  });

  test("extracts latest sticker for avatar expression selection", () => {
    const messages = [
      textMessage("assistant-1", "assistant", "[EMOJI:sticker:😴:EMOJI]"),
      textMessage("assistant-2", "assistant", "done [EMOJI:sticker:😂:EMOJI]"),
    ];
    const sticker = latestSticker(messages);

    expect(sticker).toBe("😂");
    expect(
      avatarExpressionIdsForEmoji(
        {
          name: "tura",
          root_directory: "",
          expression_directory: "",
          default_expression: "vigilant",
          default_direction: "right",
          expressions: [
            {
              id: "vigilant",
              name: "vigilant",
              source_directory: "",
              grid_path: "",
              frames: {},
            },
            {
              id: "laufgh",
              name: "laufgh",
              source_directory: "",
              grid_path: "",
              frames: {},
            },
          ],
        },
        sticker,
      ),
    ).toEqual(["laufgh"]);
  });
});
