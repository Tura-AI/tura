import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import type { JsonObject } from "../types/common.js";
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
    metadata: buildRunMetadata(messages),
  };
}

function buildRunMetadata(messages: Message[]): RunResult["metadata"] {
  const usage = aggregateUsage(messages);
  const commands = aggregateCommands(messages);
  return {
    input_token_usage: usage.inputTokens,
    input_token_cache: usage.cachedInputTokens,
    provider_time_ms: usage.providerTimeMs,
    total_time_ms: totalTimeMs(messages),
    commands: commands.total,
    failed_commands: commands.failed,
    tps: usage.providerTimeMs > 0 ? round(usage.outputTokens / (usage.providerTimeMs / 1000)) : 0,
    turns: messages.filter((message) => message.role === "user").length,
  };
}

function aggregateUsage(messages: Message[]): {
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  providerTimeMs: number;
} {
  const seen = new Set<string>();
  let inputTokens = 0;
  let cachedInputTokens = 0;
  let outputTokens = 0;
  let providerTimeMs = 0;

  for (const message of messages) {
    const explicitUsages = usageCandidates(message);
    for (const { usage, key } of explicitUsages) {
      if (key && seen.has(key)) continue;
      if (key) seen.add(key);
      inputTokens += numberField(usage, "input_tokens");
      cachedInputTokens += numberField(usage, "cached_input_tokens");
      outputTokens += numberField(usage, "output_tokens");
      providerTimeMs += numberField(usage, "latency_ms");
    }
    if (explicitUsages.length > 0) continue;
    const tokens = recordLike(message.tokens);
    if (!tokens) continue;
    inputTokens += numberField(tokens, "input");
    outputTokens += numberField(tokens, "output");
    cachedInputTokens += numberField(recordLike(tokens.cache), "read");
  }

  return { inputTokens, cachedInputTokens, outputTokens, providerTimeMs };
}

function usageCandidates(message: Message): Array<{ usage: JsonObject; key: string }> {
  const candidates: Array<{ usage: JsonObject; key: string }> = [];
  for (const part of message.parts ?? []) {
    for (const value of [part.metadata, recordLike(part.state)?.metadata]) {
      const metadata = recordLike(value);
      const usage = recordLike(metadata?.usage);
      if (usage) {
        const runtimeID = stringField(metadata, "runtime_id");
        candidates.push({
          usage,
          key: runtimeID ? `runtime:${runtimeID}` : `${message.id}:${part.id}:usage`,
        });
      }
    }
  }
  return candidates;
}

function aggregateCommands(messages: Message[]): { total: number; failed: number } {
  let total = 0;
  let failed = 0;
  const seen = new Set<string>();

  for (const message of messages) {
    for (const part of message.parts ?? []) {
      const state = recordLike(part.state);
      const metadata = recordLike(part.metadata);
      const tool =
        part.tool ??
        stringField(metadata, "tool") ??
        stringField(recordLike(state?.metadata), "tool") ??
        "";
      if (tool !== "command_run") continue;

      for (const command of commandItems(recordLike(state?.input) ?? recordLike(metadata?.input))) {
        const key = commandKey(command);
        if (key && seen.has(key)) continue;
        if (key) seen.add(key);
        total += 1;
      }
      for (const result of commandResults(state?.output, metadata?.output)) {
        const key = commandKey(result);
        if (key && !seen.has(key)) {
          seen.add(key);
          total += 1;
        }
        if (commandFailed(result)) failed += 1;
      }
      if (!state && metadata && commandFailed(metadata)) {
        total += 1;
        failed += 1;
      }
    }
  }

  return { total, failed };
}

function commandItems(value: JsonObject | undefined): JsonObject[] {
  if (!value) return [];
  const commands = arrayField(value, "commands").map(recordLike).filter(isRecord);
  if (commands.length > 0) return commands;
  if (stringField(value, "command_type") || stringField(value, "command")) return [value];
  return [];
}

function commandResults(...values: unknown[]): JsonObject[] {
  const results: JsonObject[] = [];
  for (const value of values) {
    const output = recordLike(value);
    if (!output) continue;
    results.push(...arrayField(output, "results").map(recordLike).filter(isRecord));
    const streamed = recordLike(output.streamed_command_run_result);
    results.push(...arrayField(streamed, "results").map(recordLike).filter(isRecord));
  }
  return results;
}

function commandFailed(command: JsonObject): boolean {
  if (command.success === false) return true;
  const status = stringField(command, "status")?.toLowerCase();
  return status === "failed" || status === "error";
}

function commandKey(command: JsonObject): string | undefined {
  const id = stringField(command, "command_id") ?? stringField(command, "id");
  if (id) return id;
  const line =
    stringField(command, "command_line") ??
    stringField(command, "command") ??
    stringField(command, "command_type");
  const step = numberField(command, "step");
  return line ? `${step}:${line}` : undefined;
}

function totalTimeMs(messages: Message[]): number {
  const times = messages
    .flatMap((message) => [
      message.created_at,
      message.updated_at,
      message.time?.created,
      message.time?.updated,
    ])
    .filter((value): value is number => typeof value === "number" && Number.isFinite(value));
  if (times.length === 0) return 0;
  return Math.max(...times) - Math.min(...times);
}

function recordLike(value: unknown): JsonObject | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as JsonObject)
    : undefined;
}

function isRecord(value: JsonObject | undefined): value is JsonObject {
  return Boolean(value);
}

function arrayField(object: JsonObject | undefined, key: string): unknown[] {
  const value = object?.[key];
  return Array.isArray(value) ? value : [];
}

function numberField(object: JsonObject | undefined, key: string): number {
  const value = object?.[key];
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function stringField(object: JsonObject | undefined, key: string): string | undefined {
  const value = object?.[key];
  return typeof value === "string" ? value : undefined;
}

function round(value: number): number {
  return Number.isFinite(value) ? Math.round(value * 100) / 100 : 0;
}

export async function writeLastMessage(path: string | undefined, text: string): Promise<void> {
  if (!path) return;
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, text, "utf8");
}
