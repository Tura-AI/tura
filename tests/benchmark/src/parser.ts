import path from "node:path";

import {
  ROUND_SCHEMA,
  emptyUsage,
  type BenchmarkAgentRound,
  type BenchmarkCliInstruction,
  type BenchmarkToolCall,
  type JsonValue,
  type TokenUsage,
} from "./contracts.js";
import { stableJsonHash, writeJsonFile } from "./io.js";

type UnknownRecord = Record<string, unknown>;

export type BenchmarkInstructionInput =
  | string
  | {
      commandName?: string;
      commandLine?: string;
      command?: string;
      args?: string[];
      cwd?: string;
      env?: Record<string, string>;
      raw?: JsonValue;
    };

export function normalizeCliInstruction(input: BenchmarkInstructionInput): BenchmarkCliInstruction {
  if (typeof input === "string") {
    const args = splitCommandLine(input);
    return { commandName: args[0] ?? "", commandLine: input, args };
  }

  const commandName = input.commandName ?? input.command ?? input.args?.[0] ?? "";
  const args = input.args ?? (input.commandLine ? splitCommandLine(input.commandLine) : [commandName].filter(Boolean));
  const commandLine = input.commandLine ?? args.map(quoteCommandArg).join(" ");
  return {
    commandName,
    commandLine,
    args,
    cwd: input.cwd,
    env: input.env,
    raw: input.raw,
  };
}

export function parseAgentRound(callback: unknown, roundIndex = 0): BenchmarkAgentRound {
  const record = asRecord(callback) ?? {};
  const startedAt = readString(record, ["startedAt", "startTimestamp", "started_at"]) ?? new Date().toISOString();
  const endedAt = readString(record, ["endedAt", "endTimestamp", "ended_at"]) ?? startedAt;
  const usage = readUsage(record);
  const toolCalls = normalizeToolCalls(callback);
  const rawJson = toJsonValue(callback);

  return {
    schema: ROUND_SCHEMA,
    roundId: readString(record, ["roundId", "id", "turnId", "turn_id"]) ?? `round-${roundIndex + 1}`,
    roundIndex,
    startedAt,
    endedAt,
    input: { fullContext: extractFullContext(record) },
    output: {
      fullOutput: extractFullOutput(record),
      assistantMessage: extractAssistantMessage(record),
    },
    usage,
    providerDurationMs: readNumber(record, ["providerDurationMs", "provider_duration_ms", "duration_ms"]) ?? 0,
    toolCalls,
    rawCallbackPath: rawJson ? undefined : undefined,
  };
}

export function parseJsonlRounds(text: string): BenchmarkAgentRound[] {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      try {
        return JSON.parse(line) as unknown;
      } catch {
        return null;
      }
    })
    .filter((value): value is unknown => value !== null)
    .map((value, index) => parseAgentRound(value, index));
}

export async function saveAgentRound(directory: string, round: BenchmarkAgentRound): Promise<string> {
  const filePath = path.join(directory, `${String(round.roundIndex + 1).padStart(4, "0")}-${round.roundId}.json`);
  await writeJsonFile(filePath, round as unknown as JsonValue);
  return filePath;
}

export function normalizeToolCalls(value: unknown): BenchmarkToolCall[] {
  const calls = collectToolCallCandidates(value);
  return calls.flatMap((call, index) => normalizeOneToolCall(call, index));
}

function collectToolCallCandidates(value: unknown): UnknownRecord[] {
  const root = asRecord(value);
  if (!root) return [];
  const result: UnknownRecord[] = [];
  pushArrayRecords(result, root.toolCalls);
  pushArrayRecords(result, root.tool_calls);
  pushArrayRecords(result, asRecord(root.message)?.tool_calls);
  pushArrayRecords(result, asRecord(root.assistantMessage)?.tool_calls);
  pushArrayRecords(result, asRecord(root.assistant_message)?.tool_calls);

  const body = asRecord(root.body) ?? asRecord(root.response) ?? asRecord(root.provider) ?? root;
  pushOpenAiOutput(result, body.output);
  pushArrayRecords(result, asRecord(asArray(body.choices)?.[0])?.message ? asRecord(asRecord(asArray(body.choices)?.[0])?.message)?.tool_calls : undefined);

  if (isFunctionCall(root)) result.push(root);
  return result;
}

function normalizeOneToolCall(call: UnknownRecord, index: number): BenchmarkToolCall[] {
  const name = toolName(call) || "tool";
  const id = readString(call, ["id", "call_id", "tool_call_id"]) ?? `${name}-${index + 1}`;
  const args = parseToolArguments(call);
  const parallelGroupId = readString(call, ["parallelGroupId", "parallel_group_id", "step"]);

  if (name === "command_run") {
    const commands = asArray(asRecord(args)?.commands);
    if (commands.length > 0) {
      return commands.map((command, commandIndex) => {
        const commandRecord = asRecord(command) ?? {};
        const commandName =
          readString(commandRecord, ["command_type", "commandType", "name"]) ??
          inferCommandName(readString(commandRecord, ["command", "command_line", "commandLine"]) ?? "command");
        return {
          id: `${id}:${commandIndex + 1}`,
          kind: "command",
          name: commandName,
          commandLine: commandLineFromRecord(commandRecord),
          arguments: toJsonValue(command) ?? {},
          parentToolName: name,
          parentToolCallId: id,
          parallelGroupId: readString(commandRecord, ["parallelGroupId", "parallel_group_id", "step"]) ?? parallelGroupId,
          raw: toJsonValue(command),
        };
      });
    }
  }

  return [
    {
      id,
      kind: "tool",
      name,
      commandLine: commandLineFromValue(args),
      arguments: toJsonValue(args) ?? {},
      parallelGroupId,
      raw: toJsonValue(call),
    },
  ];
}

function readUsage(record: UnknownRecord): TokenUsage {
  const candidates = [
    record.usage,
    asRecord(record.message)?.usage,
    asRecord(record.result)?.usage,
    asRecord(record.assistantMessageEvent)?.usage,
    asRecord(record.response)?.usage,
    asRecord(record.body)?.usage,
  ];
  const usage = emptyUsage();
  for (const candidate of candidates) {
    const item = asRecord(candidate);
    if (!item) continue;
    usage.inputTokens += readNumber(item, ["inputTokens", "input_tokens", "prompt_tokens"]) ?? 0;
    usage.cacheInputTokens +=
      readNumber(item, ["cacheInputTokens", "cached_input_tokens", "cache_read_input_tokens"]) ??
      readNumber(asRecord(item.input_tokens_details) ?? {}, ["cached_tokens"]) ??
      0;
    usage.outputTokens += readNumber(item, ["outputTokens", "output_tokens", "completion_tokens"]) ?? 0;
    usage.reasoningTokens +=
      readNumber(item, ["reasoningTokens", "reasoning_tokens", "reasoning_output_tokens"]) ??
      readNumber(asRecord(item.output_tokens_details) ?? {}, ["reasoning_tokens"]) ??
      0;
    usage.totalTokens += readNumber(item, ["totalTokens", "total_tokens"]) ?? 0;
  }
  if (usage.totalTokens === 0) {
    usage.totalTokens = usage.inputTokens + usage.cacheInputTokens + usage.outputTokens + usage.reasoningTokens;
  }
  return usage;
}

function extractFullContext(record: UnknownRecord): string {
  return readString(record, ["fullContext", "full_context", "inputContext", "input_context", "context"]) ??
    stringifyFirst(record.messages, asRecord(record.request)?.input, asRecord(record.body)?.input);
}

function extractFullOutput(record: UnknownRecord): string {
  return readString(record, ["fullOutput", "full_output", "output", "content", "text"]) ??
    stringifyFirst(asRecord(record.response)?.output, asRecord(record.body)?.output, record.message);
}

function extractAssistantMessage(record: UnknownRecord): string {
  const candidates = [
    record.assistantMessage,
    record.assistantmessage,
    record.assistant_message,
    asRecord(record.message)?.content,
    asRecord(record.response)?.output_text,
    asRecord(record.body)?.output_text,
  ];
  for (const candidate of candidates) {
    if (typeof candidate === "string") return candidate;
  }
  return extractTextFromOutput(asRecord(record.response)?.output ?? asRecord(record.body)?.output ?? record.output);
}

function extractTextFromOutput(value: unknown): string {
  const pieces: string[] = [];
  for (const item of asArray(value)) {
    const record = asRecord(item);
    if (!record) continue;
    if (typeof record.text === "string") pieces.push(record.text);
    for (const content of asArray(record.content)) {
      const contentRecord = asRecord(content);
      if (typeof contentRecord?.text === "string") pieces.push(contentRecord.text);
    }
  }
  return pieces.join("\n");
}

function parseToolArguments(call: UnknownRecord): unknown {
  const fn = asRecord(call.function);
  const raw = call.arguments ?? fn?.arguments ?? call.args ?? call.input ?? {};
  if (typeof raw === "string") {
    try {
      return JSON.parse(raw) as unknown;
    } catch {
      return raw;
    }
  }
  return raw;
}

function commandLineFromRecord(record: UnknownRecord): string {
  return readString(record, ["commandLine", "command_line", "command", "cmd"]) ?? commandLineFromValue(record);
}

function commandLineFromValue(value: unknown): string {
  if (typeof value === "string") return value;
  const record = asRecord(value);
  if (record) {
    const direct = readString(record, ["commandLine", "command_line", "command", "cmd"]);
    if (direct) return direct;
  }
  return JSON.stringify(toJsonValue(value) ?? {});
}

function toolName(call: UnknownRecord): string | undefined {
  return readString(call, ["name", "tool_name"]) ?? readString(asRecord(call.function) ?? {}, ["name"]);
}

function isFunctionCall(value: UnknownRecord): boolean {
  return value.type === "function_call" || Boolean(value.function) || Boolean(value.arguments && toolName(value));
}

function pushOpenAiOutput(result: UnknownRecord[], output: unknown): void {
  for (const item of asArray(output)) {
    const record = asRecord(item);
    if (record && isFunctionCall(record)) result.push(record);
  }
}

function pushArrayRecords(result: UnknownRecord[], value: unknown): void {
  for (const item of asArray(value)) {
    const record = asRecord(item);
    if (record) result.push(record);
  }
}

function splitCommandLine(value: string): string[] {
  const args: string[] = [];
  let current = "";
  let quote: "'" | '"' | null = null;
  for (let index = 0; index < value.length; index += 1) {
    const char = value[index];
    if ((char === '"' || char === "'") && quote === null) {
      quote = char;
    } else if (char === quote) {
      quote = null;
    } else if (/\s/.test(char) && quote === null) {
      if (current) args.push(current);
      current = "";
    } else {
      current += char;
    }
  }
  if (current) args.push(current);
  return args;
}

function quoteCommandArg(value: string): string {
  return /\s/.test(value) ? JSON.stringify(value) : value;
}

function inferCommandName(commandLine: string): string {
  return splitCommandLine(commandLine)[0] ?? "command";
}

function readString(record: UnknownRecord, keys: string[]): string | undefined {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.length > 0) return value;
    if (typeof value === "number") return String(value);
  }
  return undefined;
}

function readNumber(record: UnknownRecord, keys: string[]): number | undefined {
  for (const key of keys) {
    const value = Number(record[key]);
    if (Number.isFinite(value)) return value;
  }
  return undefined;
}

function stringifyFirst(...values: unknown[]): string {
  for (const value of values) {
    if (typeof value === "string") return value;
    if (value !== undefined && value !== null) return JSON.stringify(toJsonValue(value) ?? value);
  }
  return "";
}

function asRecord(value: unknown): UnknownRecord | null {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? (value as UnknownRecord) : null;
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function toJsonValue(value: unknown): JsonValue | undefined {
  if (value === null || typeof value === "string" || typeof value === "number" || typeof value === "boolean") return value;
  if (Array.isArray(value)) return value.map((item) => toJsonValue(item) ?? null);
  const record = asRecord(value);
  if (!record) return undefined;
  const out: Record<string, JsonValue> = {};
  for (const [key, item] of Object.entries(record)) out[key] = toJsonValue(item) ?? null;
  return out;
}

export function roundContentHash(round: BenchmarkAgentRound): string {
  return stableJsonHash(round as unknown as JsonValue);
}
