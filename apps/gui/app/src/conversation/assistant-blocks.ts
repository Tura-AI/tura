import type { MessagePart } from "@tura/gateway-sdk";
import { isToolPart } from "./message-tools";

export type AssistantBlock = {
  type: "text" | "tools";
  parts: MessagePart[];
};

export function assistantPartBlocks(
  parts: MessagePart[],
  visibleTextIds: Set<string>,
): AssistantBlock[] {
  const blocks: AssistantBlock[] = [];
  for (const part of parts) {
    if (!isToolPart(part)) {
      if (visibleTextIds.has(part.id)) {
        blocks.push({ type: "text", parts: [part] });
      }
      continue;
    }
    if (part.tool !== "runtime") {
      blocks.push({ type: "tools", parts: [part] });
    }
  }
  return blocks;
}

export function assistantToolBlockForPart(
  parts: MessagePart[],
  partId: string,
): AssistantBlock | undefined {
  const part = parts.find(
    (item) => item.id === partId && isToolPart(item) && item.tool !== "runtime",
  );
  return part ? { type: "tools", parts: [part] } : undefined;
}
