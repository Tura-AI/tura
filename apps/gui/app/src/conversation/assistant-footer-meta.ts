import type { Message } from "@tura/gateway-sdk";
import { asRecord } from "./message-tools";
import { normalizeAgentReasoningLevel } from "../../../../tui/src/agent-runtime-config";

export function assistantFooterMetaText(message: Message): string {
  const runtime = messageRuntimeMeta(message);
  const costValue = runtime.cost > 0 ? runtime.cost : (message.cost ?? 0);
  const cost = costValue > 0 ? `$${costValue.toFixed(4)}` : "";
  const runtimeText = [runtime.reasoningLevel, runtime.priorityEnabled ? "priority" : ""]
    .filter(Boolean)
    .join(" - ");
  return [runtimeText, cost].filter(Boolean).join(" · ");
}

function messageRuntimeMeta(message: Message): {
  cost: number;
  reasoningLevel?: ReturnType<typeof normalizeAgentReasoningLevel>;
  priorityEnabled: boolean;
} {
  let cost = 0;
  let reasoningLevel: ReturnType<typeof normalizeAgentReasoningLevel> | undefined;
  let priorityEnabled = false;

  for (const metadata of [asRecord(message.metadata)]) {
    const extracted = extractRuntimeMetadata(metadata);
    cost += extracted.cost;
    reasoningLevel ??= extracted.reasoningLevel;
    priorityEnabled ||= extracted.priorityEnabled;
  }

  for (const part of message.parts) {
    const state = asRecord(part.state);
    const candidates = [asRecord(part.metadata), asRecord(state.metadata)];
    for (const metadata of candidates) {
      const extracted = extractRuntimeMetadata(metadata);
      cost += extracted.cost;
      reasoningLevel ??= extracted.reasoningLevel;
      priorityEnabled ||= extracted.priorityEnabled;
    }
  }

  return { cost, reasoningLevel, priorityEnabled };
}

function extractRuntimeMetadata(metadata: Record<string, unknown>): {
  cost: number;
  reasoningLevel?: ReturnType<typeof normalizeAgentReasoningLevel>;
  priorityEnabled: boolean;
} {
  const usage = asRecord(metadata.usage);
  const runtime = asRecord(metadata.runtime);
  const runtimeReasoning =
    stringField(runtime, "reasoning_level") ??
    stringField(runtime, "reasoningLevel") ??
    stringField(metadata, "reasoning_level") ??
    stringField(metadata, "model_variant");
  return {
    cost: numericField(usage, "total_cost") ?? 0,
    reasoningLevel: runtimeReasoning ? normalizeAgentReasoningLevel(runtimeReasoning) : undefined,
    priorityEnabled:
      booleanField(runtime, "priority") ??
      booleanField(runtime, "model_acceleration_enabled") ??
      booleanField(metadata, "model_acceleration_enabled") ??
      false,
  };
}

function numericField(record: Record<string, unknown>, key: string) {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function stringField(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function booleanField(record: Record<string, unknown>, key: string): boolean | undefined {
  const value = record[key];
  return typeof value === "boolean" ? value : undefined;
}
