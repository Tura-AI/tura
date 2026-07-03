import type { Message } from "@tura/gateway-sdk";
import { asRecord } from "./message-tools";
import {
  formatAgentRuntimeModelText,
  normalizeAgentReasoningLevel,
} from "../../../../tui/src/agent-runtime-config";

export function assistantFooterModelText(message: Message): string {
  const runtime = messageRuntimeMeta(message);
  const provider = runtime.providerID ?? message.providerID;
  const modelId = runtime.modelID ?? message.modelID;
  const model = normalizedModelText(provider, modelId) || "Tura";
  return runtime.reasoningLevel
    ? formatAgentRuntimeModelText(
        model,
        { reasoningLevel: runtime.reasoningLevel, priorityEnabled: runtime.priorityEnabled },
        "priority",
      )
    : model;
}

export function assistantFooterMetaText(message: Message): string {
  const model = assistantFooterModelText(message);
  const runtime = messageRuntimeMeta(message);
  const costValue = runtime.cost > 0 ? runtime.cost : (message.cost ?? 0);
  const cost = costValue > 0 ? `$${costValue.toFixed(4)}` : "";
  return [model, cost].filter(Boolean).join(" · ");
}

function normalizedModelText(provider: string | undefined, modelId: string | undefined): string {
  const providerText = provider?.trim() ?? "";
  const modelText = modelId?.trim() ?? "";
  if (!providerText) {
    return modelText;
  }
  if (!modelText) {
    return providerText;
  }
  if (sameModelLabel(providerText, modelText) || providerText.endsWith(`/${modelText}`)) {
    return providerText;
  }
  if (modelText.startsWith(`${providerText}/`)) {
    return modelText;
  }
  return `${providerText}/${modelText}`;
}

function sameModelLabel(left: string, right: string): boolean {
  return left.trim().toLowerCase() === right.trim().toLowerCase();
}

function messageRuntimeMeta(message: Message): {
  cost: number;
  providerID?: string;
  modelID?: string;
  reasoningLevel?: ReturnType<typeof normalizeAgentReasoningLevel>;
  priorityEnabled: boolean;
} {
  let cost = 0;
  let providerID: string | undefined;
  let modelID: string | undefined;
  let reasoningLevel: ReturnType<typeof normalizeAgentReasoningLevel> | undefined;
  let priorityEnabled = false;

  for (const metadata of [asRecord(message.metadata)]) {
    const extracted = extractRuntimeMetadata(metadata);
    cost += extracted.cost;
    providerID ??= extracted.providerID;
    modelID ??= extracted.modelID;
    reasoningLevel ??= extracted.reasoningLevel;
    priorityEnabled ||= extracted.priorityEnabled;
  }

  for (const part of message.parts) {
    const state = asRecord(part.state);
    const candidates = [asRecord(part.metadata), asRecord(state.metadata)];
    for (const metadata of candidates) {
      const extracted = extractRuntimeMetadata(metadata);
      cost += extracted.cost;
      providerID ??= extracted.providerID;
      modelID ??= extracted.modelID;
      reasoningLevel ??= extracted.reasoningLevel;
      priorityEnabled ||= extracted.priorityEnabled;
    }
  }

  return { cost, providerID, modelID, reasoningLevel, priorityEnabled };
}

function extractRuntimeMetadata(metadata: Record<string, unknown>): {
  cost: number;
  providerID?: string;
  modelID?: string;
  reasoningLevel?: ReturnType<typeof normalizeAgentReasoningLevel>;
  priorityEnabled: boolean;
} {
  const usage = asRecord(metadata.usage);
  const provider = asRecord(metadata.provider);
  const runtime = asRecord(metadata.runtime);
  const runtimeReasoning =
    stringField(runtime, "reasoning_level") ??
    stringField(runtime, "reasoningLevel") ??
    stringField(metadata, "reasoning_level") ??
    stringField(metadata, "model_variant");
  return {
    cost: numericField(usage, "total_cost") ?? 0,
    providerID:
      stringField(provider, "provider_name") ??
      stringField(provider, "providerID") ??
      stringField(provider, "provider_id") ??
      stringField(metadata, "providerID") ??
      stringField(metadata, "provider_id"),
    modelID:
      stringField(provider, "model_name") ??
      stringField(provider, "modelID") ??
      stringField(provider, "model_id") ??
      stringField(metadata, "modelID") ??
      stringField(metadata, "model_id"),
    reasoningLevel: runtimeReasoning
      ? normalizeAgentReasoningLevel(runtimeReasoning)
      : undefined,
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
