import type { MessagePart } from "@tura/gateway-sdk";
import { isToolPart, toolCreatedAt } from "./message-tools";

export type AssistantBlock = {
  type: "text" | "tools";
  parts: MessagePart[];
};

type AssistantBlockEntry = {
  block: AssistantBlock;
  createdAt?: number;
  index: number;
};

export function assistantPartBlocks(
  parts: MessagePart[],
  visibleTextIds: Set<string>,
): AssistantBlock[] {
  return assistantBlockEntries(parts, visibleTextIds).map(({ block }) => block);
}

export function assistantToolBlockForPart(
  parts: MessagePart[],
  partId: string,
): AssistantBlock | undefined {
  return assistantBlockEntries(
    parts,
    new Set(parts.filter((part) => !isToolPart(part)).map((part) => part.id)),
  )
    .map(({ block }) => block)
    .find((block) => block.type === "tools" && block.parts.some((part) => part.id === partId));
}

function assistantBlockEntries(
  parts: MessagePart[],
  visibleTextIds: Set<string>,
): AssistantBlockEntry[] {
  const entries: AssistantBlockEntry[] = [];
  for (const [index, part] of parts.entries()) {
    if (!isToolPart(part)) {
      if (visibleTextIds.has(part.id)) {
        entries.push({ block: { type: "text", parts: [part] }, index });
      }
      continue;
    }
    if (part.tool === "runtime") {
      continue;
    }
    entries.push({
      block: { type: "tools", parts: [part] },
      createdAt: toolCreatedAt(part),
      index,
    });
  }
  return entries.sort((left, right) => {
    if (left.createdAt !== undefined && right.createdAt !== undefined) {
      return left.createdAt - right.createdAt || left.index - right.index;
    }
    return left.index - right.index;
  });
}
