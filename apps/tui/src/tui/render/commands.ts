import { t } from "../../i18n.js";
import type { MessagePart } from "../../types/session.js";
import type { AppState } from "../reducer.js";
import {
  activeCapabilities,
  dim,
  reset,
  richHighlight,
  stripAnsi,
  textAuxiliary,
  truncateAnsi,
} from "../render-terminal.js";
import {
  compactInlinePayloads,
  compactPayloadField,
  firstCommandLine,
  sanitizeRawTerminalText,
  toolSummary,
} from "../render-payload.js";
import { renderRichText } from "../render-rich-text.js";
import { iconAnimationFrame } from "./busy-animation.js";

export type CommandInfo = {
  command: string;
  name?: string;
  step?: number;
  tool?: string;
  status?: string;
};

export function commandsForPart(part: MessagePart): CommandInfo[] {
  const state =
    part.state && typeof part.state === "object" ? (part.state as Record<string, unknown>) : {};
  const status = typeof state.status === "string" ? state.status : undefined;
  const tool = part.tool ?? t("tool");
  const streamedCommands = streamedCommandRunCommands(part, state, tool, status);
  if (streamedCommands.length) return streamedCommands;
  const commands = [
    ...commandInfosFromUnknown(state.input, tool, status),
    ...commandInfosFromUnknown(state.output, tool, status),
    ...commandInfosFromUnknown(part.metadata, tool, status),
  ];
  return commands;
}

function commandInfosFromUnknown(
  value: unknown,
  tool: string,
  status: string | undefined,
): CommandInfo[] {
  if (!value) return [];
  if (typeof value === "string") {
    const record = recordLike(value);
    return Object.keys(record).length ? commandInfosFromUnknown(record, tool, status) : [];
  }
  if (Array.isArray(value))
    return value.flatMap((item) => commandInfosFromUnknown(item, tool, status));
  if (!isRecord(value)) return [];
  const commands: CommandInfo[] = [];
  const step = numberField(value, "step");
  const command = commandLineFromRecord(value);
  const name = commandNameFromRecord(value);
  if (command && name) commands.push({ command, name, step, tool, status });
  for (const key of ["commands", "results", "steps", "input", "output"]) {
    commands.push(...commandInfosFromUnknown(value[key], tool, status));
  }
  return commands;
}

function streamedCommandRunCommands(
  part: MessagePart,
  state: Record<string, unknown>,
  tool: string,
  fallbackStatus: string | undefined,
): CommandInfo[] {
  if (part.tool !== "command_run") return [];
  const specs = commandSpecs(state, part.metadata);
  const streamed = streamedCommandRunResults(state, part.metadata);
  if (specs.length) {
    const resultsById = new Map(
      streamed.flatMap((result) => commandRecordKeys(result).map((key) => [key, result] as const)),
    );
    return specs
      .map((spec, index) => ({
        result: resultForSpec(resultsById, spec) ?? streamed[index],
        spec,
        index,
      }))
      .map(({ result, spec, index }) =>
        commandInfoFromStreamedResult(
          result ?? spec,
          spec,
          index,
          tool,
          result || streamed.length === 0 ? fallbackStatus : undefined,
        ),
      )
      .filter((command): command is CommandInfo => Boolean(command));
  }
  return streamed
    .map((result, index) =>
      commandInfoFromStreamedResult(result, undefined, index, tool, fallbackStatus),
    )
    .filter((command): command is CommandInfo => Boolean(command));
}

function streamedCommandRunResults(
  state: Record<string, unknown>,
  metadata: unknown,
): Record<string, unknown>[] {
  const output = recordLike(state.output);
  const stateMetadata = recordLike(state.metadata);
  const stateMetadataOutput = recordLike(stateMetadata.output);
  const stateStream = recordLike(state.streamed_command_run_result);
  const outputStream = recordLike(output.streamed_command_run_result);
  const stateMetadataStream = recordLike(stateMetadataOutput.streamed_command_run_result);
  const metadataRecord = recordLike(metadata);
  const metadataOutput = recordLike(metadataRecord.output);
  const metadataStream = recordLike(metadataOutput.streamed_command_run_result);
  return [
    ...arrayField(stateStream, "results"),
    ...arrayField(outputStream, "results"),
    ...arrayField(output, "results"),
    ...arrayField(stateMetadataStream, "results"),
    ...arrayField(stateMetadataOutput, "results"),
    ...arrayField(metadataStream, "results"),
    ...arrayField(metadataOutput, "results"),
  ]
    .filter((value): value is Record<string, unknown> => isRecord(value))
    .reduce<Record<string, unknown>[]>((records, record) => {
      const keys = commandRecordKeys(record);
      const existingIndex = keys
        .map((key) => records.findIndex((item) => commandRecordKeys(item).includes(key)))
        .find((index) => index >= 0);
      if (existingIndex === undefined) return [...records, record];
      records[existingIndex] = { ...records[existingIndex], ...record };
      return records;
    }, []);
}

function commandSpecs(
  state: Record<string, unknown>,
  metadata: unknown,
): Record<string, unknown>[] {
  const input = recordLike(state.input);
  const output = recordLike(state.output);
  const outputStream = recordLike(output.streamed_command_run_result);
  const stateMetadata = recordLike(state.metadata);
  const stateMetadataInput = recordLike(stateMetadata.input);
  const stateMetadataOutput = recordLike(stateMetadata.output);
  const stateMetadataStream = recordLike(stateMetadataOutput.streamed_command_run_result);
  const metadataRecord = recordLike(metadata);
  const metadataInput = recordLike(metadataRecord.input);
  const metadataOutput = recordLike(metadataRecord.output);
  const metadataStream = recordLike(metadataOutput.streamed_command_run_result);
  return [
    ...arrayField(input, "commands"),
    ...arrayField(outputStream, "commands"),
    ...arrayField(output, "commands"),
    ...arrayField(stateMetadataInput, "commands"),
    ...arrayField(stateMetadataStream, "commands"),
    ...arrayField(stateMetadataOutput, "commands"),
    ...arrayField(metadataInput, "commands"),
    ...arrayField(metadataStream, "commands"),
    ...arrayField(metadataOutput, "commands"),
  ]
    .filter((value): value is Record<string, unknown> => isRecord(value))
    .reduce<Record<string, unknown>[]>((records, record) => {
      const keys = commandRecordKeys(record);
      const existingIndex = keys
        .map((key) => records.findIndex((item) => commandRecordKeys(item).includes(key)))
        .find((index) => index >= 0);
      if (existingIndex === undefined) return [...records, record];
      records[existingIndex] = { ...records[existingIndex], ...record };
      return records;
    }, []);
}

function commandInfoFromStreamedResult(
  result: Record<string, unknown>,
  spec: Record<string, unknown> | undefined,
  index: number,
  tool: string,
  fallbackStatus: string | undefined,
): CommandInfo | undefined {
  const name = commandNameFromStreamedResult(result, spec);
  if (!name) return undefined;
  const command = commandLineFromStreamedResult(result, spec);
  if (!command) return undefined;
  return {
    command,
    name,
    step: commandStepFromStreamedResult(result, spec) ?? index + 1,
    tool,
    status: commandStatusFromStreamedResult(result, fallbackStatus),
  };
}

function commandStepFromStreamedResult(
  result: Record<string, unknown>,
  spec: Record<string, unknown> | undefined,
): number | undefined {
  const resultCommand = recordLike(result.command);
  const specCommand = recordLike(spec?.command);
  const step =
    numberField(result, "step") ??
    numberField(resultCommand, "step") ??
    numberField(spec ?? {}, "step") ??
    numberField(specCommand, "step");
  return step !== undefined && step > 0 ? Math.trunc(step) : undefined;
}

function commandLineFromStreamedResult(
  result: Record<string, unknown>,
  spec: Record<string, unknown> | undefined,
): string | undefined {
  const resultCommand = recordLike(result.command);
  const specCommand = recordLike(spec?.command);
  const command =
    stringField(result, "command_line") ??
    stringField(resultCommand, "command_line") ??
    stringField(spec ?? {}, "command_line") ??
    stringField(specCommand, "command_line") ??
    commandFieldWithType(result) ??
    commandFieldWithType(resultCommand) ??
    commandFieldWithType(spec ?? {}) ??
    commandFieldWithType(specCommand);
  if (command) return sanitizeRawTerminalText(command).trim();
  return commandNameFromStreamedResult(result, spec);
}

function commandNameFromStreamedResult(
  result: Record<string, unknown>,
  spec: Record<string, unknown> | undefined,
): string | undefined {
  const resultCommand = recordLike(result.command);
  const specCommand = recordLike(spec?.command);
  return (
    commandNameFromRecord(result) ??
    commandNameFromRecord(resultCommand) ??
    commandNameFromRecord(spec ?? {}) ??
    commandNameFromRecord(specCommand)
  );
}

function commandStatusFromStreamedResult(
  result: Record<string, unknown>,
  fallbackStatus: string | undefined,
): string | undefined {
  if (result.success === false) return "failed";
  if (typeof result.status === "string")
    return result.status === "in_progress" ? "running" : result.status;
  if (result.success === true) return "completed";
  return fallbackStatus;
}

function recordLike(value: unknown): Record<string, unknown> {
  if (typeof value === "string") {
    try {
      const parsed = JSON.parse(value) as unknown;
      return isRecord(parsed) ? parsed : {};
    } catch {
      return {};
    }
  }
  return isRecord(value) ? value : {};
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function arrayField(record: Record<string, unknown>, key: string): unknown[] {
  const value = record[key];
  return Array.isArray(value) ? value : [];
}

function stringField(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  return typeof value === "string" && value.trim() ? value : undefined;
}

function numberField(record: Record<string, unknown>, key: string): number | undefined {
  const value = record[key];
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && /^\d+(?:\.\d+)?$/u.test(value.trim())) {
    const parsed = Number(value.trim());
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}

function commandLineFromRecord(record: Record<string, unknown>): string | undefined {
  const commandType = commandTypeFromRecord(record);
  if (!commandType) return undefined;
  const command =
    stringField(record, "command_line") ??
    stringField(record, "commandLine") ??
    commandFieldWithType(record);
  if (command) return sanitizeRawTerminalText(command).trim();
  return commandType;
}

function commandNameFromRecord(record: Record<string, unknown>): string | undefined {
  return commandTypeFromRecord(record);
}

function commandRecordID(record: Record<string, unknown> | undefined): string | undefined {
  if (!record) return undefined;
  return stringField(record, "command_id") ?? stringField(record, "commandID");
}

function commandRecordKeys(record: Record<string, unknown>): string[] {
  const command = recordLike(record.command);
  const keys = new Set<string>();
  const id = commandRecordID(record) ?? commandRecordID(command);
  if (id) keys.add(`id:${id}`);

  const provider =
    stringField(record, "provider_tool_call_id") ??
    stringField(record, "providerToolCallID") ??
    stringField(command, "provider_tool_call_id") ??
    stringField(command, "providerToolCallID");
  const index =
    numberField(record, "command_index") ??
    numberField(record, "commandIndex") ??
    numberField(command, "command_index") ??
    numberField(command, "commandIndex");
  if (provider && index !== undefined) keys.add(`provider:${provider}:${index}`);

  const step = numberField(record, "step") ?? numberField(command, "step");
  const commandLine =
    stringField(record, "command_line") ??
    stringField(command, "command_line") ??
    stringField(record, "command");
  const commandType =
    stringField(record, "command_type") ??
    stringField(command, "command_type") ??
    stringField(record, "name") ??
    stringField(command, "name");
  if (step !== undefined && commandLine)
    keys.add(`step:${step}:${commandType ?? ""}:${commandLine}`);
  return [...keys];
}

function resultForSpec(
  resultsById: Map<string, Record<string, unknown>>,
  spec: Record<string, unknown>,
): Record<string, unknown> | undefined {
  for (const key of commandRecordKeys(spec)) {
    const result = resultsById.get(key);
    if (result) return result;
  }
  return undefined;
}

function commandTypeFromRecord(record: Record<string, unknown>): string | undefined {
  return stringField(record, "command_type") ?? stringField(record, "commandType");
}

function commandFieldWithType(record: Record<string, unknown> | undefined): string | undefined {
  if (!record || !commandTypeFromRecord(record)) return undefined;
  const command = stringField(record, "command");
  const commandType = commandTypeFromRecord(record);
  if (!command || command === commandType) return undefined;
  return command;
}

export function commandSectionLines(
  commands: CommandInfo[],
  state: AppState,
  summaryCols: number,
  detailCols: number,
  showCommands: boolean,
): string[] {
  const lines = [truncateAnsi(commandSummaryLine(commands, state, summaryCols), detailCols)];
  if (showCommands) {
    for (const line of commandDetailLines(commands, state, summaryCols)) {
      lines.push(truncateAnsi(line, detailCols));
    }
  }
  return lines;
}

function commandSummaryLine(commands: CommandInfo[], state: AppState, cols: number): string {
  const running = commands.some((command) => commandIsRunning(command.status));
  const runningIcons = activeCapabilities.unicode ? ["◆", "◇", "◈"] : ["#", "*", "+"];
  const frame = iconAnimationFrame(state.thinkingFrame);
  const icon = activeCapabilities.unicode
    ? running
      ? (runningIcons[frame % runningIcons.length] ?? "◆")
      : "◇"
    : running
      ? (runningIcons[frame % runningIcons.length] ?? "#")
      : "*";
  const label = `${icon} ${t("commands")}`;
  return auxiliaryText(truncateAnsi(label, Math.max(12, cols - 2)));
}

function commandDetailLines(commands: CommandInfo[], state: AppState, _cols: number): string[] {
  const lines: string[] = [];
  for (const [index, command] of commands.entries()) {
    const isLast = index === commands.length - 1;
    const branch = activeCapabilities.unicode ? (isLast ? "└─" : "├─") : "|-";
    const symbol = statusSymbol(command.status, state.thinkingFrame);
    const meta = [command.name, command.status].filter(Boolean).join(" ");
    const step = command.step ?? index + 1;
    const prefix = `${branch} ${stripAnsi(symbol)} #${step}${meta ? ` ${meta}` : ""}  $ `;
    const text = `${prefix}${firstCommandLine(command.command)}`;
    lines.push(auxiliaryText(text));
  }
  return lines;
}

function statusSymbol(status: string | undefined, frame: number): string {
  const normalized = (status ?? "").toLowerCase();
  if (/fail|error|reject|denied/.test(normalized)) return `${richHighlight}x${reset}`;
  if (commandIsRunning(status)) {
    const frames = activeCapabilities.unicode ? ["■", "□", "◧"] : ["#", "*", "+"];
    const iconFrame = iconAnimationFrame(frame);
    return `${richHighlight}${frames[iconFrame % frames.length] ?? frames[0]}${reset}`;
  }
  if (/done|complete|success|ok/.test(normalized))
    return `${richHighlight}${activeCapabilities.unicode ? "✓" : "+"}${reset}`;
  return `${dim}${activeCapabilities.unicode ? "•" : "-"}${reset}`;
}

export function commandIsRunning(status: string | undefined): boolean {
  return /run|progress|pending|busy|question|in[_ -]?progress|exec(?:ute|uting|uted|ution)?|start/i.test(
    status ?? "",
  );
}

export function commandPartStatus(part: MessagePart): string | undefined {
  if (part.tool !== "command_run" && part.type !== "tool") return undefined;
  if (!part.state || typeof part.state !== "object") return undefined;
  const status = (part.state as { status?: unknown }).status;
  return typeof status === "string" ? status : undefined;
}

export function partTranscriptLines(part: MessagePart): string[] {
  if (part.type !== "tool") return [];
  if (part.tool === "runtime" || part.tool === "command_run") return [];
  if (commandsForPart(part).length) return [];
  const state =
    part.state && typeof part.state === "object" ? (part.state as Record<string, unknown>) : {};
  const status = typeof state.status === "string" ? state.status : t("updated");
  const tool = part.tool ?? t("tool");
  const rawSummary = toolSummary(state);
  const compactSummary = compactPayloadField(rawSummary) ?? compactInlinePayloads(rawSummary);
  const summary = truncateAnsi(renderRichText(compactSummary), 88);
  return [`[${tool}: ${summary || status}]`];
}

function auxiliaryText(value: string): string {
  return activeCapabilities.level === "plain"
    ? `${dim}${value}${reset}`
    : `${textAuxiliary}${value}${reset}`;
}
