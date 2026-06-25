import type { Message, MessagePart } from "@tura/gateway-sdk";
import { t } from "../i18n";
import { jsonPreview } from "../state/format";

type JsonRecord = Record<string, unknown>;

export type ToolRecord = {
  id: string;
  partId: string;
  groupId?: string;
  kind: "command" | "patch" | "tool";
  title: string;
  command: string;
  output: string;
  status: string;
  durationMs?: number;
  timeoutMs?: number;
  exitCode?: number;
};

export function isToolPart(part: MessagePart): boolean {
  return !!part.tool || part.type === "tool" || part.type.includes("tool");
}

export function asRecord(value: unknown): JsonRecord {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as JsonRecord) : {};
}

export function toolTitle(part: MessagePart): string {
  const state = asRecord(part.state);
  const title = stringField(state, "title") || stringField(state, "step_summary");
  return title || part.tool || part.type || "tool";
}

export function toolCommand(part: MessagePart): string {
  const state = asRecord(part.state);
  const input = asRecord(state.input);
  return (
    stringField(state, "command") ||
    stringField(state, "command_line") ||
    stringField(input, "command") ||
    stringField(input, "command_line") ||
    stringField(input, "step_summary") ||
    part.tool ||
    "command"
  );
}

export function toolStatus(state: JsonRecord): string {
  const value = stringField(state, "status") || stringField(asRecord(state.output), "status");
  if (!value) {
    return "pending";
  }
  if (value === "in_progress") {
    return "running";
  }
  if (value === "error") {
    return "failed";
  }
  return value;
}

export function toolDurationMs(part: MessagePart): number | undefined {
  const state = asRecord(part.state);
  const direct = numberField(state, "duration_ms") ?? numberField(state, "durationMs");
  if (direct !== undefined) {
    return direct;
  }
  const status = toolStatus(state);
  const time = asRecord(state.time);
  const started =
    numberField(time, "start") ??
    numberField(time, "started") ??
    numberField(state, "started_at") ??
    numberField(state, "created_at") ??
    numberField(state, "createdAt");
  const ended =
    numberField(time, "end") ??
    numberField(time, "ended") ??
    numberField(state, "completed_at") ??
    numberField(state, "updated_at") ??
    numberField(state, "updatedAt");
  if (started !== undefined && isRunningStatus(status)) {
    return Math.max(0, Date.now() - normalizeEpochMs(started));
  }
  if (started && ended) {
    return Math.max(0, normalizeEpochMs(ended) - normalizeEpochMs(started));
  }
  return undefined;
}

export function messageDurationMs(message: Message): number | undefined {
  const start = message.time?.created ?? message.created_at;
  const end = message.time?.updated ?? message.updated_at;
  if (!start || !end) {
    return undefined;
  }
  return Math.max(0, normalizeEpochMs(end) - normalizeEpochMs(start));
}

export function formatDuration(ms?: number): string {
  if (ms === undefined) {
    return "0s";
  }
  if (ms < 1000) {
    return `${Math.max(0.1, Math.round(ms / 100) / 10).toFixed(1)}s`;
  }
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) {
    return `${seconds}s`;
  }
  return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
}

export function formatCommandTiming(durationMs?: number, timeoutMs?: number): string {
  const duration = formatCompactDuration(durationMs);
  return timeoutMs === undefined ? duration : `${duration}/${formatCompactDuration(timeoutMs)}`;
}

export function toolOutput(part?: MessagePart): string {
  if (!part) {
    return "";
  }
  const state = asRecord(part.state);
  const metadata = asRecord(part.metadata);
  const candidates = [state.output, state.error, metadata.output, metadata.error, state];
  for (const candidate of candidates) {
    const text = outputText(candidate);
    if (text.trim()) {
      return text;
    }
  }
  return jsonPreview(part.state || part.metadata);
}

export function toolRecords(parts: MessagePart[]): ToolRecord[] {
  return parts
    .filter((part) => isToolPart(part) && part.tool !== "runtime")
    .flatMap((part) => toolPartRecords(part))
    .filter((record) => !isCommandRunWrapper(record) && !isRuntimeWrapper(record));
}

export function toolPartRecords(part: MessagePart): ToolRecord[] {
  const state = asRecord(part.state);
  const streamed = streamedResults(state);
  const specs = commandSpecs(state);
  if (specs.length > 0) {
    const resultsById = new Map(
      streamed
        .map(asRecord)
        .flatMap((result) => commandRecordKeys(result).map((key) => [key, result] as const))
        .filter((entry): entry is [string, JsonRecord] => Boolean(entry[0])),
    );
    return specs
      .map((spec, index) => ({ result: resultForSpec(resultsById, spec), spec, index }))
      .map(({ result, spec, index }) => commandRecord(part, result ?? spec, spec, index));
  }
  const visibleStreamed = streamed.map((value, index) => ({ result: asRecord(value), index }));
  if (visibleStreamed.length > 0) {
    return visibleStreamed.map(({ result, index }) =>
      commandRecord(part, result, undefined, index),
    );
  }
  const patchRecords = patchRecordsFromState(part, state);
  if (patchRecords.length > 0) {
    return patchRecords;
  }
  return [fallbackRecord(part)];
}

function streamedResults(state: JsonRecord): unknown[] {
  const output = state.output;
  const parsedOutput = typeof output === "string" ? parseJsonRecord(output) : asRecord(output);
  return uniqueCommandRecords([
    ...arrayField(asRecord(state.streamed_command_run_result), "results"),
    ...arrayField(asRecord(parsedOutput.streamed_command_run_result), "results"),
    ...arrayField(parsedOutput, "results"),
  ]);
}

export function diffLines(text: string): Array<{ kind: "add" | "del" | "ctx"; text: string }> {
  return text
    .split(/\r\n|\r|\n/u)
    .filter((line) => /^[+-]/u.test(line) && !line.startsWith("+++") && !line.startsWith("---"))
    .map((line) => ({
      kind: line.startsWith("+") ? "add" : line.startsWith("-") ? "del" : "ctx",
      text: line,
    }));
}

export function isPatchRecord(record: ToolRecord): boolean {
  return record.kind === "patch" || diffLines(record.output).length > 0;
}

function commandRecord(
  part: MessagePart,
  result: JsonRecord,
  spec: JsonRecord | undefined,
  index: number,
): ToolRecord {
  const commandSource =
    stringField(result, "command_line") ||
    stringField(spec ?? {}, "command_line") ||
    stringField(result, "command") ||
    stringField(spec ?? {}, "command") ||
    stringField(result, "command_type") ||
    part.tool ||
    "command";
  const rawCommand = normalizeCommandLine(commandSource);
  const commandType =
    stringField(result, "command_type") ||
    stringField(spec ?? {}, "command_type") ||
    part.tool ||
    "";
  const command = cleanCommandLine(commandType, rawCommand);
  const output = outputText(result.output) || outputText(result);
  const exitCode =
    numberField(result, "exit_code") ?? numberField(result, "exitCode") ?? exitCodeFromText(output);
  const status = result.success === false ? "failed" : toolStatusFromResult(result, exitCode);
  const recordId = commandRecordID(result) ?? commandRecordID(spec) ?? `record-${index}`;
  return {
    id: `${part.id}:${recordId}`,
    partId: part.id,
    groupId: toolGroupId(part),
    kind: isPatchCommand(commandType, rawCommand) ? "patch" : "command",
    title: commandTitle(commandType, rawCommand, index),
    command,
    output,
    status,
    durationMs: commandDurationMs(result, spec, status, output),
    timeoutMs: commandTimeoutMs(result, spec, commandSource),
    exitCode,
  };
}

function patchRecordsFromState(part: MessagePart, state: JsonRecord): ToolRecord[] {
  const output = toolOutput(part);
  const command = toolCommand(part);
  if (!isPatchCommand(part.tool ?? "", command) && diffLines(output).length === 0) {
    return [];
  }
  return [
    {
      id: `${part.id}:patch`,
      partId: part.id,
      groupId: toolGroupId(part),
      kind: "patch",
      title: commandTitle(part.tool ?? "apply_patch", command, 0),
      command: cleanCommandLine(part.tool ?? "apply_patch", command),
      output,
      status: toolStatus(state),
      durationMs:
        toolDurationMs(part) ?? durationFromText(output) ?? fallbackDurationMs(toolStatus(state)),
      timeoutMs: commandTimeoutMs(state, undefined, command),
      exitCode:
        numberField(state, "exit_code") ??
        numberField(state, "exitCode") ??
        exitCodeFromText(output),
    },
  ];
}

function fallbackRecord(part: MessagePart): ToolRecord {
  const state = asRecord(part.state);
  const output = toolOutput(part);
  const command = toolCommand(part);
  return {
    id: `${part.id}:tool`,
    partId: part.id,
    groupId: toolGroupId(part),
    kind: isPatchCommand(part.tool ?? "", command) ? "patch" : "tool",
    title: commandTitle(part.tool ?? part.type, command, 0),
    command: cleanCommandLine(part.tool ?? part.type, command),
    output,
    status: toolStatus(state),
    durationMs:
      toolDurationMs(part) ?? durationFromText(output) ?? fallbackDurationMs(toolStatus(state)),
    timeoutMs: commandTimeoutMs(state, undefined, command),
    exitCode:
      numberField(state, "exit_code") ?? numberField(state, "exitCode") ?? exitCodeFromText(output),
  };
}

function commandSpecs(state: JsonRecord): JsonRecord[] {
  return uniqueCommandRecords([
    ...arrayField(asRecord(state.input), "commands").map(asRecord),
    ...arrayField(state, "commands").map(asRecord),
    ...arrayField(asRecord(asRecord(state.metadata).input), "commands").map(asRecord),
  ]).map(asRecord);
}

function commandRecordID(record: JsonRecord | undefined): string | undefined {
  if (!record) {
    return undefined;
  }
  return stringField(record, "command_id") || stringField(record, "commandID");
}

function uniqueCommandRecords(records: unknown[]): unknown[] {
  const seen = new Map<string, number>();
  const next: unknown[] = [];
  for (const record of records) {
    const keys = commandRecordKeys(record);
    if (keys.length === 0) {
      next.push(record);
      continue;
    }
    const existingIndex = keys
      .map((key) => seen.get(key))
      .find((index): index is number => index !== undefined);
    if (existingIndex === undefined) {
      for (const key of keys) {
        seen.set(key, next.length);
      }
      next.push(record);
      continue;
    }
    next[existingIndex] = { ...asRecord(next[existingIndex]), ...asRecord(record) };
    for (const key of commandRecordKeys(next[existingIndex])) {
      seen.set(key, existingIndex);
    }
  }
  return next;
}

function resultForSpec(
  resultsById: Map<string, JsonRecord>,
  spec: JsonRecord,
): JsonRecord | undefined {
  for (const key of commandRecordKeys(spec)) {
    const result = resultsById.get(key);
    if (result) {
      return result;
    }
  }
  return undefined;
}

function commandRecordKeys(record: unknown): string[] {
  const value = asRecord(record);
  const command = asRecord(value.command);
  const keys = new Set<string>();
  const id = commandRecordID(value) || commandRecordID(command);
  if (id) {
    keys.add(`id:${id}`);
  }

  const provider =
    stringField(value, "provider_tool_call_id") ||
    stringField(value, "providerToolCallID") ||
    stringField(command, "provider_tool_call_id") ||
    stringField(command, "providerToolCallID");
  const index =
    numberField(value, "command_index") ??
    numberField(value, "commandIndex") ??
    numberField(command, "command_index") ??
    numberField(command, "commandIndex");
  if (provider && index !== undefined) {
    keys.add(`provider:${provider}:${index}`);
  }
  const step = numberField(value, "step") ?? numberField(command, "step");
  const commandLine =
    stringField(value, "command_line") ||
    stringField(command, "command_line") ||
    stringField(value, "command");
  const commandType =
    stringField(value, "command_type") ||
    stringField(command, "command_type") ||
    stringField(value, "name") ||
    stringField(command, "name");
  if (step !== undefined && commandLine) {
    keys.add(`step:${step}:${commandType ?? ""}:${commandLine}`);
  }
  return [...keys];
}

function toolGroupId(part: MessagePart): string | undefined {
  const state = asRecord(part.state);
  const metadata = asRecord(part.metadata);
  return (
    stringField(state, "llm_call_id") ||
    stringField(state, "turn_id") ||
    stringField(state, "response_id") ||
    stringField(metadata, "llm_call_id") ||
    stringField(metadata, "turn_id") ||
    stringField(metadata, "response_id")
  );
}

function normalizeCommandLine(value: string): string {
  const trimmed = value.trim();
  if (!trimmed.startsWith("{")) {
    return value;
  }
  try {
    const parsed = JSON.parse(trimmed);
    const command =
      typeof parsed.command === "string"
        ? parsed.command
        : typeof parsed.command_line === "string"
          ? parsed.command_line
          : "";
    return command || value;
  } catch {
    return value;
  }
}

function parsedCommandLine(value: string): JsonRecord {
  const trimmed = value.trim();
  if (!trimmed.startsWith("{")) {
    return {};
  }
  try {
    return asRecord(JSON.parse(trimmed));
  } catch {
    return {};
  }
}

function isCommandRunWrapper(record: ToolRecord): boolean {
  return record.kind === "tool" && record.command.trim() === "command_run";
}

function isRuntimeWrapper(record: ToolRecord): boolean {
  return (
    record.kind === "tool" &&
    record.command.trim() === "runtime" &&
    record.title.trim() === "runtime  runtime"
  );
}

function toolStatusFromResult(result: JsonRecord, exitCode?: number): string {
  const explicit = stringField(result, "status");
  if (explicit) {
    return toolStatus({ status: explicit });
  }
  if (exitCode !== undefined) {
    return exitCode === 0 ? "completed" : "failed";
  }
  if (result.success === true) {
    return "completed";
  }
  return "running";
}

function commandTitle(commandType: string, command: string, index: number): string {
  const prefix = commandLabel(commandType, command);
  const firstLine = cleanCommandLine(commandType, command)
    .split(/\r\n|\r|\n/u)[0]
    ?.trim();
  if (!firstLine) {
    return `${prefix}\u00a0\u00a0\u00a0\u00a0${t("command")} ${index + 1}`;
  }
  return `${prefix}\u00a0\u00a0\u00a0\u00a0${firstLine}`;
}

function commandLabel(commandType: string, command: string): string {
  const type = normalizedCommandType(commandType || command);
  const commandValue = normalizedCommandType(command);
  if (type.includes("apply patch") || commandValue.startsWith("apply patch")) {
    return t("commandTypePatch");
  }
  if (type.includes("bash") || commandValue.startsWith("bash")) {
    return t("commandTypeBash");
  }
  if (
    [
      "shell",
      "shell command",
      "shellcommand",
      "shll",
      "shll command",
      "powershell",
      "pwsh",
      "sh",
    ].some((value) => type === value || commandValue.startsWith(value))
  ) {
    return t("commandTypeShell");
  }
  const direct = commandTypeLabelByKey(type);
  if (direct) {
    return direct;
  }
  const fromCommand = commandTypeLabelByKey(commandValue);
  return fromCommand || commandType.trim() || t("commandTypeCommand");
}

function cleanCommandLine(commandType: string, command: string): string {
  const type = commandType.trim();
  const normalizedType = normalizedCommandType(type);
  let value = command.trim();
  if (type && value.toLowerCase().startsWith(type.toLowerCase())) {
    value = value.slice(type.length).trimStart();
  }
  value = stripCommandPrefix(value, normalizedType);
  for (const prefix of commandLineTypePrefixes()) {
    const next = stripCommandPrefix(value, prefix);
    if (next !== value) {
      value = next;
      break;
    }
  }
  return value || command.trim();
}

function commandTypeLabelByKey(type: string): string | undefined {
  const value = normalizedCommandType(type);
  if (value === "readmedia" || value.startsWith("read media")) {
    return t("commandTypeReadMedia");
  }
  if (value === "webdiscover" || value.startsWith("web discover")) {
    return t("commandTypeWebDiscover");
  }
  if (value === "compactcontext" || value.startsWith("compact context")) {
    return t("commandTypeCompactContext");
  }
  if (value === "formatcheck" || value.startsWith("format check")) {
    return t("commandTypeFormatCheck");
  }
  switch (value) {
    case "browser":
      return t("commandTypeBrowser");
    case "runtime":
      return t("commandTypeRuntime");
    default:
      return undefined;
  }
}

function commandLineTypePrefixes(): string[] {
  return [
    "apply patch",
    "applypatch",
    "apply_patch",
    "read media",
    "readmedia",
    "read_media",
    "web discover",
    "webdiscover",
    "web_discover",
    "compact context",
    "compactcontext",
    "compact_context",
  ];
}

function stripCommandPrefix(value: string, normalizedPrefix: string): string {
  if (!normalizedPrefix) {
    return value;
  }
  const variants = new Set([
    normalizedPrefix,
    normalizedPrefix.replaceAll(" ", "_"),
    normalizedPrefix.replaceAll(" ", "-"),
    normalizedPrefix.replaceAll(" ", ""),
  ]);
  for (const variant of variants) {
    const pattern = new RegExp(`^${escapeRegExp(variant)}(?:\\s+|$)`, "iu");
    if (pattern.test(value)) {
      return value.replace(pattern, "").trimStart();
    }
  }
  return value;
}

function normalizedCommandType(value: string): string {
  return value.trim().replace(/[_-]+/gu, " ").replace(/\s+/gu, " ").toLowerCase();
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function isPatchCommand(commandType: string, command: string): boolean {
  const value = `${commandType} ${command}`.toLowerCase();
  return value.includes("apply_patch") || value.includes("patch");
}

function fallbackDurationMs(status: string): number | undefined {
  return ["completed", "failed", "success", "done"].includes(status.toLowerCase())
    ? 1000
    : undefined;
}

function commandDurationMs(
  result: JsonRecord,
  spec: JsonRecord | undefined,
  status: string,
  output: string,
): number | undefined {
  return (
    numberField(result, "duration_ms") ??
    numberField(result, "durationMs") ??
    numberField(spec ?? {}, "duration_ms") ??
    numberField(spec ?? {}, "durationMs") ??
    durationFromTimeFields(result, status) ??
    durationFromTimeFields(spec ?? {}, status) ??
    durationFromText(output) ??
    fallbackDurationMs(status)
  );
}

function commandTimeoutMs(
  result: JsonRecord,
  spec: JsonRecord | undefined,
  rawCommand: string,
): number | undefined {
  const resultCommand = asRecord(result.command);
  const specCommand = asRecord(spec?.command);
  const parsed = parsedCommandLine(rawCommand);
  return (
    numberField(result, "timeout_ms") ??
    numberField(result, "timeoutMs") ??
    numberField(resultCommand, "timeout_ms") ??
    numberField(resultCommand, "timeoutMs") ??
    numberField(spec ?? {}, "timeout_ms") ??
    numberField(spec ?? {}, "timeoutMs") ??
    numberField(specCommand, "timeout_ms") ??
    numberField(specCommand, "timeoutMs") ??
    numberField(parsed, "timeout_ms") ??
    numberField(parsed, "timeoutMs")
  );
}

function durationFromTimeFields(record: JsonRecord, status: string): number | undefined {
  const time = asRecord(record.time);
  const started =
    numberField(time, "start") ??
    numberField(time, "started") ??
    numberField(record, "started_at") ??
    numberField(record, "created_at") ??
    numberField(record, "createdAt");
  const ended =
    numberField(time, "end") ??
    numberField(time, "ended") ??
    numberField(record, "completed_at") ??
    numberField(record, "updated_at") ??
    numberField(record, "updatedAt");
  if (started !== undefined && isRunningStatus(status)) {
    return Math.max(0, Date.now() - normalizeEpochMs(started));
  }
  if (started !== undefined && ended !== undefined) {
    return Math.max(0, normalizeEpochMs(ended) - normalizeEpochMs(started));
  }
  return undefined;
}

function isRunningStatus(status: string): boolean {
  return status === "running" || status === "in_progress";
}

function durationFromText(text: string): number | undefined {
  const match = text.match(/Wall time:\s*([\d.]+)\s*(ms|milliseconds?|s|sec|seconds?)/iu);
  if (!match) {
    return undefined;
  }
  const value = Number(match[1]);
  if (!Number.isFinite(value)) {
    return undefined;
  }
  const unit = match[2].toLowerCase();
  return unit.startsWith("ms") || unit.startsWith("millisecond") ? value : value * 1000;
}

function formatCompactDuration(ms?: number): string {
  if (ms === undefined) {
    return "0s";
  }
  if (ms < 1000) {
    return `${Math.max(0.1, Math.round(ms / 100) / 10).toFixed(1)}s`;
  }
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) {
    return `${seconds}s`;
  }
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return rest === 0 ? `${minutes}m` : `${minutes}m${rest}s`;
}

function exitCodeFromText(text: string): number | undefined {
  const match = text.match(/Exit code:\s*(-?\d+)/iu);
  return match ? Number(match[1]) : undefined;
}

function arrayField(record: JsonRecord, key: string): unknown[] {
  const value = record[key];
  return Array.isArray(value) ? value : [];
}

function normalizeEpochMs(value: number): number {
  return value > 10_000_000_000 ? value : value * 1000;
}

function outputText(value: unknown): string {
  if (value === undefined || value === null) {
    return "";
  }
  if (typeof value === "string") {
    return value;
  }
  const record = asRecord(value);
  for (const key of ["aggregated_output", "stdout", "stderr", "text", "message", "error"]) {
    const text = stringField(record, key);
    if (text) {
      return text;
    }
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function parseJsonRecord(value: string): JsonRecord {
  try {
    return asRecord(JSON.parse(value));
  } catch {
    return {};
  }
}

function stringField(record: JsonRecord, key: string): string | undefined {
  const value = record[key];
  return typeof value === "string" && value.trim() ? value : undefined;
}

function numberField(record: JsonRecord, key: string): number | undefined {
  const value = record[key];
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string") {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}
