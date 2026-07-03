import type { Message } from "@tura/gateway-sdk";
import { asRecord } from "./message-tools";

export function assistantFooterMetaText(message: Message): string {
  const runtimeCost = messageRuntimeCost(message);
  const costValue = runtimeCost > 0 ? runtimeCost : (message.cost ?? 0);
  const cost = costValue > 0 ? `$${costValue.toFixed(4)}` : "";
  return cost;
}

function messageRuntimeCost(message: Message): number {
  let cost = 0;

  for (const metadata of [asRecord(message.metadata)]) {
    cost += extractRuntimeCost(metadata);
  }

  for (const part of message.parts) {
    const state = asRecord(part.state);
    const candidates = [asRecord(part.metadata), asRecord(state.metadata)];
    for (const metadata of candidates) {
      cost += extractRuntimeCost(metadata);
    }
  }

  return cost;
}

function extractRuntimeCost(metadata: Record<string, unknown>): number {
  const usage = asRecord(metadata.usage);
  return numericField(usage, "total_cost") ?? 0;
}

function numericField(record: Record<string, unknown>, key: string) {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

