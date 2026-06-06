import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import type { Message, RunResult } from "../types/session.js";
import { lastAssistantText } from "../types/session.js";

export function buildRunResult(
  sessionID: string,
  messages: Message[],
  status: RunResult["status"] = "completed",
): RunResult {
  const finalText = lastAssistantText(messages);
  const lastAssistant = [...messages].reverse().find((message) => message.role === "assistant");
  return {
    sessionID,
    status,
    finalText,
    messages,
    usage: lastAssistant?.tokens ?? null,
  };
}

export async function writeLastMessage(path: string | undefined, text: string): Promise<void> {
  if (!path) return;
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, text, "utf8");
}
