import type { Message } from "@tura/gateway-sdk";
import { asRecord } from "./message-tools";

export function assistantFooterModelText(message: Message): string {
  const runtime = messageRuntimeMeta(message);
  const provider = runtime.providerID ?? message.providerID;
  const modelId = runtime.modelID ?? message.modelID;
  return normalizedModelText(provider, modelId) || "Tura";
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
} {
  let cost = 0;
  let providerID: string | undefined;
  let modelID: string | undefined;

  for (const part of message.parts) {
    const state = asRecord(part.state);
    const candidates = [asRecord(part.metadata), asRecord(state.metadata)];
    for (const metadata of candidates) {
      const usage = asRecord(metadata.usage);
      cost += numericField(usage, "total_cost") ?? 0;

      const provider = asRecord(metadata.provider);
      providerID ??=
        stringField(provider, "provider_name") ??
        stringField(provider, "providerID") ??
        stringField(provider, "provider_id") ??
        stringField(metadata, "providerID") ??
        stringField(metadata, "provider_id");
      modelID ??=
        stringField(provider, "model_name") ??
        stringField(provider, "modelID") ??
        stringField(provider, "model_id") ??
        stringField(metadata, "modelID") ??
        stringField(metadata, "model_id");
    }
  }

  return { cost, providerID, modelID };
}

function numericField(record: Record<string, unknown>, key: string) {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function stringField(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
