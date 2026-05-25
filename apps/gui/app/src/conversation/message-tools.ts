import type { Message, MessagePart } from "@tura/gateway-sdk";
import { jsonPreview } from "../state/format";
import { t } from "../i18n";

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
  exitCode?: number;
};

export function isToolPart(part: MessagePart): boolean {
  return !!part.tool || part.type === "tool" || part.type.includes("tool");
}

export function asRecord(value: unknown): JsonRecord {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as JsonRecord)
    : {};
}

export function toolTitle(part: MessagePart): string {
  const state = asRecord(part.state);
  const title =
    stringField(state, "title") || stringField(state, "step_summary");
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
  const value =
    stringField(state, "status") ||
    stringField(asRecord(state.output), "status");
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
  const direct =
    numberField(state, "duration_ms") || numberField(state, "durationMs");
  if (direct) {
    return direct;
  }
  const time = asRecord(state.time);
  const started =
    numberField(time, "start") ||
    numberField(time, "started") ||
    numberField(state, "started_at");
  const ended =
    numberField(time, "end") ||
    numberField(time, "ended") ||
    numberField(state, "completed_at");
  if (started) {
    return Math.max(
      0,
      normalizeEpochMs(ended ?? Date.now()) - normalizeEpochMs(started),
    );
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

export function toolOutput(part?: MessagePart): string {
  if (!part) {
    return "";
  }
  const state = asRecord(part.state);
  const metadata = asRecord(part.metadata);
  const candidates = [
    state.output,
    state.error,
    metadata.output,
    metadata.error,
    state,
  ];
  for (const candidate of candidates) {
    const text = outputText(candidate);
    if (text.trim()) {
      return text;
    }
  }
  return jsonPreview(part.state || part.metadata);
}

export function toolRecords(parts: MessagePart[]): ToolRecord[] {
  const sharedSpecs = parts.flatMap((part) =>
    commandSpecs(asRecord(part.state)),
  );
  let specCursor = 0;
  const recordsByPart = parts.map((part) => {
    const state = asRecord(part.state);
    const streamed = streamedResults(state);
    const specs =
      streamed.length > 0
        ? sharedSpecs.slice(specCursor, specCursor + streamed.length)
        : [];
    if (streamed.length > 0 && part.tool !== "runtime") {
      specCursor += streamed.length;
    }
    return {
      part,
      records: toolPartRecords(part, specs),
    };
  });
  const nonRuntimeRecords = visibleToolRecords(
    recordsByPart
      .filter(({ part }) => part.tool !== "runtime")
      .flatMap(({ records }) => records),
  );
  const runtimeRecords = visibleToolRecords(
    recordsByPart.flatMap(({ part, records }) =>
      part.tool === "runtime" && !records.every(isRawRuntimeRecord)
        ? records
        : [],
    ),
  );
  const visibleRecords =
    nonRuntimeRecords.length > 0 ? nonRuntimeRecords : runtimeRecords;
  return visibleRecords.length > 0
    ? visibleRecords
    : parts
        .map((part) => fallbackRecord(part))
        .filter(
          (record) => !isCommandRunWrapper(record) && !isRuntimeWrapper(record),
        );
}

export function toolPartRecords(
  part: MessagePart,
  providedSpecs: JsonRecord[] = [],
): ToolRecord[] {
  const state = asRecord(part.state);
  const streamed = streamedResults(state);
  const localSpecs = commandSpecs(state);
  const specs = localSpecs.length > 0 ? localSpecs : providedSpecs;
  if (streamed.length > 0) {
    return streamed.map((value, index) =>
      commandRecord(part, asRecord(value), specs[index], index),
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
  const parsedOutput =
    typeof output === "string" ? parseJsonRecord(output) : asRecord(output);
  return [
    ...arrayField(asRecord(state.streamed_command_run_result), "results"),
    ...arrayField(
      asRecord(parsedOutput.streamed_command_run_result),
      "results",
    ),
    ...arrayField(parsedOutput, "results"),
  ];
}

export function diffLines(
  text: string,
): Array<{ kind: "add" | "del" | "ctx"; text: string }> {
  return text
    .split(/\r\n|\r|\n/u)
    .filter(
      (line) =>
        /^[+-]/u.test(line) &&
        !line.startsWith("+++") &&
        !line.startsWith("---"),
    )
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
  const rawCommand = normalizeCommandLine(
    stringField(result, "command_line") ||
      stringField(spec ?? {}, "command_line") ||
      stringField(result, "command") ||
      stringField(spec ?? {}, "command") ||
      stringField(result, "command_type") ||
      part.tool ||
      "command",
  );
  const commandType =
    stringField(result, "command_type") ||
    stringField(spec ?? {}, "command_type") ||
    part.tool ||
    "";
  const command = cleanCommandLine(commandType, rawCommand);
  const output = outputText(result.output) || outputText(result);
  const exitCode =
    numberField(result, "exit_code") ??
    numberField(result, "exitCode") ??
    exitCodeFromText(output);
  const status =
    result.success === false
      ? "failed"
      : toolStatusFromResult(result, exitCode);
  return {
    id: `${part.id}:record-${index}`,
    partId: part.id,
    groupId: toolGroupId(part),
    kind: isPatchCommand(commandType, rawCommand) ? "patch" : "command",
    title: commandTitle(commandType, rawCommand, index),
    command,
    output,
    status,
    durationMs:
      numberField(result, "duration_ms") ??
      numberField(result, "durationMs") ??
      durationFromText(output) ??
      fallbackDurationMs(status),
    exitCode,
  };
}

function patchRecordsFromState(
  part: MessagePart,
  state: JsonRecord,
): ToolRecord[] {
  const output = toolOutput(part);
  const command = toolCommand(part);
  if (
    !isPatchCommand(part.tool ?? "", command) &&
    diffLines(output).length === 0
  ) {
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
        toolDurationMs(part) ??
        durationFromText(output) ??
        fallbackDurationMs(toolStatus(state)),
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
      toolDurationMs(part) ??
      durationFromText(output) ??
      fallbackDurationMs(toolStatus(state)),
    exitCode:
      numberField(state, "exit_code") ??
      numberField(state, "exitCode") ??
      exitCodeFromText(output),
  };
}

function commandSpecs(state: JsonRecord): JsonRecord[] {
  const direct = [
    ...arrayField(asRecord(state.input), "commands").map(asRecord),
    ...arrayField(asRecord(asRecord(state.metadata).input), "commands").map(
      asRecord,
    ),
  ];
  const calls = arrayField(asRecord(state.provider_content), "tool_calls");
  const specs: JsonRecord[] = [...direct];
  for (const call of calls) {
    const functionCall = asRecord(asRecord(call).function);
    if (stringField(functionCall, "name") !== "command_run") {
      continue;
    }
    const raw = functionCall.arguments;
    try {
      const parsed = typeof raw === "string" ? JSON.parse(raw) : raw;
      specs.push(...arrayField(asRecord(parsed), "commands").map(asRecord));
    } catch {
      // Command specs are helpful labels only; streamed results still render without them.
    }
  }
  return specs;
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

function isRawRuntimeRecord(record: ToolRecord): boolean {
  return (
    record.kind === "tool" &&
    record.title.toLowerCase().includes("runtime") &&
    record.output.trim().startsWith("{")
  );
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

function visibleToolRecords(records: ToolRecord[]): ToolRecord[] {
  return dedupeRecords(
    records.filter(
      (record) => !isCommandRunWrapper(record) && !isRuntimeWrapper(record),
    ),
  );
}

function dedupeRecords(records: ToolRecord[]): ToolRecord[] {
  const seen = new Set<string>();
  return records.filter((record) => {
    const key = `${record.kind}:${record.title}:${record.command}:${record.output.slice(0, 256)}`;
    if (seen.has(key)) {
      return false;
    }
    seen.add(key);
    return true;
  });
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

function commandTitle(
  commandType: string,
  command: string,
  index: number,
): string {
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
  return value
    .trim()
    .replace(/[_-]+/gu, " ")
    .replace(/\s+/gu, " ")
    .toLowerCase();
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function isPatchCommand(commandType: string, command: string): boolean {
  const value = `${commandType} ${command}`.toLowerCase();
  return value.includes("apply_patch") || value.includes("patch");
}

function fallbackDurationMs(status: string): number | undefined {
  return ["completed", "failed", "success", "done"].includes(
    status.toLowerCase(),
  )
    ? 1000
    : undefined;
}

function durationFromText(text: string): number | undefined {
  const match = text.match(
    /Wall time:\s*([\d.]+)\s*(ms|milliseconds?|s|sec|seconds?)/iu,
  );
  if (!match) {
    return undefined;
  }
  const value = Number(match[1]);
  if (!Number.isFinite(value)) {
    return undefined;
  }
  const unit = match[2].toLowerCase();
  return unit.startsWith("ms") || unit.startsWith("millisecond")
    ? value
    : value * 1000;
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
  for (const key of [
    "aggregated_output",
    "stdout",
    "stderr",
    "text",
    "message",
    "error",
  ]) {
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
