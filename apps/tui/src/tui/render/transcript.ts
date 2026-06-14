import { displayMessages, type AppState } from "../reducer.js";
import type { Message, MessagePart } from "../../types/session.js";
import { messageText } from "../../types/session.js";
import { t } from "../../i18n.js";
import {
  activeCapabilities,
  dim,
  opencodePrimary,
  opencodeTextWeak,
  reset,
  stripAnsi,
  textAuxiliary,
  textPrimary,
  thinkingWaveBaseBlend,
  thinkingWaveGlow,
  thinkingWaveLow,
  thinkingWaveMid,
  thinkingWaveMoon,
  truncateAnsi,
  wrapAnsi,
} from "../render-terminal.js";
import {
  compactInlinePayloads,
  compactPayloadField,
  displayMessageText,
  extractCommandsFromUnknown,
  firstCommandLine,
  isTaskStatusPayload,
  looksLikeCommand,
  renderRichText,
  sanitizeRawTerminalText,
  toolSummary,
} from "../render-rich-text.js";
import { thinkingAnimationFrame } from "./busy-animation.js";
import { panelBlankLine, panelLine } from "../styles/panel.js";
import { secondaryText } from "../styles/text.js";

type CommandInfo = {
  command: string;
  step?: number;
  tool?: string;
  status?: string;
};

interface OrderedTextBlock {
  kind: "text";
  text: string;
}
interface OrderedDetailBlock {
  kind: "detail";
  text: string;
}
interface OrderedCommandsBlock {
  kind: "commands";
  commands: CommandInfo[];
}
type OrderedMessageBlock = OrderedTextBlock | OrderedDetailBlock | OrderedCommandsBlock;

export function transcriptLines(state: AppState, cols: number, maxLines?: number): string[] {
  if (maxLines !== undefined && maxLines <= 0) return [];
  const messages = splitTranscriptMessages(state).cache;
  return renderTranscriptMessages(state, cols, messages, maxLines);
}

export function transcriptLiveLines(state: AppState, cols: number): string[] {
  const lines = renderTranscriptMessages(state, cols, splitTranscriptMessages(state).live);
  if (isThinking(state, displayMessages(state))) {
    lines.push("", thinkingLine(state, cols));
  }
  return lines;
}

function renderTranscriptMessages(
  state: AppState,
  cols: number,
  messages: Message[],
  maxLines?: number,
): string[] {
  const showCommands = state.sessionConfig?.show_command_instructions !== false;
  const lines: string[] = [];
  const renderedMessages =
    maxLines === undefined
      ? messages.map((message) => renderTranscriptMessage(message, state, cols, showCommands))
      : tailRenderedMessages(messages, state, cols, showCommands, maxLines);
  for (const rendered of renderedMessages) {
    addTranscriptGap(lines);
    lines.push(...rendered);
  }
  if (maxLines === undefined) return lines;
  return transcriptOutputLines(lines, maxLines);
}

function splitTranscriptMessages(state: AppState): { cache: Message[]; live: Message[] } {
  const messages = displayMessages(state);
  if (!isLiveTurnActive(state, messages)) return { cache: messages, live: [] };
  const liveStart = liveTurnStartIndex(state, messages);
  return {
    cache: messages.slice(0, liveStart),
    live: messages.slice(liveStart),
  };
}

function isLiveTurnActive(state: AppState, messages: Message[]): boolean {
  const sessionID = state.session?.id;
  const hasLiveStream = Object.values(state.liveStreams).some(
    (stream) => !sessionID || !stream.sessionID || stream.sessionID === sessionID,
  );
  if (hasLiveStream) return true;
  if (state.status === "busy" || state.session?.status === "busy") return true;
  return messages.at(-1)?.role === "user";
}

function liveTurnStartIndex(state: AppState, messages: Message[]): number {
  const sessionID = state.session?.id;
  const liveMessageIDs = new Set(
    Object.values(state.liveStreams)
      .filter((stream) => !sessionID || !stream.sessionID || stream.sessionID === sessionID)
      .map((stream) => stream.messageID),
  );
  const liveMessageIndex = messages.findIndex((message) => liveMessageIDs.has(message.id));
  const runningMessageIndex = latestRunningAssistantIndex(messages);
  const anchorIndex =
    liveMessageIndex >= 0
      ? liveMessageIndex
      : runningMessageIndex >= 0
        ? runningMessageIndex
        : Math.max(0, lastUserIndex(messages));
  for (let index = anchorIndex; index >= 0; index -= 1) {
    if (messages[index]?.role === "user") return index;
  }
  return Math.max(0, anchorIndex);
}

function latestRunningAssistantIndex(messages: Message[]): number {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message?.role === "assistant" && messageHasRunningPart(message)) return index;
  }
  return -1;
}

function messageHasRunningPart(message: Message): boolean {
  return (message.parts ?? []).some((part) => commandIsRunning(commandPartStatus(part)));
}

function lastUserIndex(messages: Message[]): number {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    if (messages[index]?.role === "user") return index;
  }
  return -1;
}

function tailRenderedMessages(
  messages: Message[],
  state: AppState,
  cols: number,
  showCommands: boolean,
  maxLines: number,
): string[][] {
  const renderedMessages: string[][] = [];
  const targetLines = Math.max(maxLines + 20, maxLines * 3);
  let renderedLineCount = 0;
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    const rendered = renderTranscriptMessage(message, state, cols, showCommands);
    if (!rendered.length) continue;
    renderedMessages.unshift(rendered);
    renderedLineCount += rendered.length + 1;
    if (renderedLineCount >= targetLines) break;
  }
  return renderedMessages;
}

function transcriptOutputLines(lines: string[], maxLines: number): string[] {
  if (maxLines <= 0) return [];
  return lines.slice(-maxLines);
}

function renderTranscriptMessage(
  message: Message,
  state: AppState,
  cols: number,
  showCommands: boolean,
): string[] {
  return activeCapabilities.level === "plain"
    ? renderSimpleMessage(message, state, cols, showCommands)
    : renderRichMessage(message, state, cols, showCommands);
}

function renderSimpleMessage(
  message: Message,
  state: AppState,
  cols: number,
  showCommands: boolean,
): string[] {
  const lines: string[] = [];
  const prefixWidth = activeCapabilities.unicode ? 4 : 3;
  const contentWidth = Math.max(20, cols - prefixWidth - 2);

  if (message.role === "user") {
    const text = displayMessageText("user", messageText(message));
    const rendered = secondaryText(stripAnsi(renderRichText(text)));
    for (const line of wrapAnsi(rendered, contentWidth)) {
      lines.push(simpleBodyLine(line, "user", cols));
    }
    return lines;
  }

  for (const block of orderedMessageBlocks(message)) {
    if (lines.length) lines.push("");
    if (block.kind === "text") {
      const richText = renderRichText(block.text);
      const displayText =
        message.role === "assistant" ? agentText(richText) : secondaryText(stripAnsi(richText));
      for (const line of wrapAnsi(displayText, contentWidth)) {
        lines.push(simpleBodyLine(line, message.role, cols));
      }
      continue;
    }
    if (block.kind === "detail") {
      for (const line of wrapAnsi(secondaryText(block.text), contentWidth)) {
        lines.push(simpleBodyLine(line, message.role, cols));
      }
      continue;
    }
    lines.push(...commandSectionLines(block.commands, state, cols, cols, showCommands));
  }
  return lines;
}

function simpleBodyLine(line: string, role: string, cols = 80): string {
  if (activeCapabilities.level === "plain") return `  ${stripAnsi(line)}`;
  return panelLine(line, cols, role);
}

function renderRichMessage(
  message: Message,
  state: AppState,
  cols: number,
  showCommands: boolean,
): string[] {
  const lines: string[] = [];
  const contentWidth = richMessageContentWidth(cols);

  if (message.role === "user") {
    const userText = displayMessageText("user", messageText(message));
    const body = secondaryText(stripAnsi(renderRichText(userText)));
    const wrapped = body ? wrapAnsi(body, contentWidth) : [];
    if (wrapped.length) {
      lines.push(richBlankRailLine("user", cols));
      for (const line of wrapped) lines.push(richContentLine(line, cols, "user"));
      lines.push(richBlankRailLine("user", cols));
    }
    return lines;
  }

  const blocks = orderedMessageBlocks(message);
  if (!blocks.length && message.role !== "assistant") {
    lines.push(richContentLine(`${opencodeTextWeak}${message.role}${reset}`, cols, message.role));
  }
  for (const block of blocks) {
    if (lines.length) lines.push("");
    if (block.kind === "text") {
      const richText = renderRichText(block.text);
      const displayText =
        message.role === "assistant" ? agentText(richText) : secondaryText(stripAnsi(richText));
      const wrapped = wrapAnsi(displayText, contentWidth);
      if (message.role === "assistant") lines.push(richBlankRailLine(message.role, cols));
      for (const line of wrapped) lines.push(richContentLine(line, cols, message.role));
      if (message.role === "assistant") lines.push(richBlankRailLine(message.role, cols));
      continue;
    }
    if (block.kind === "detail") {
      const wrapped = wrapAnsi(secondaryText(block.text), contentWidth);
      lines.push(richBlankRailLine(message.role, cols));
      for (const line of wrapped) {
        lines.push(richContentLine(secondaryText(line), cols, message.role));
      }
      lines.push(richBlankRailLine(message.role, cols));
      continue;
    }
    lines.push(...commandSectionLines(block.commands, state, cols - 6, cols, showCommands));
  }
  return lines;
}

function richMessageContentWidth(cols: number): number {
  return Math.max(20, cols - 1);
}

function agentText(value: string): string {
  if (!value) return value;
  return colorEachLine(value, textPrimary);
}

function auxiliaryText(value: string): string {
  return colorEachLine(stripAnsi(value), textAuxiliary);
}

function colorEachLine(value: string, color: string): string {
  if (!value) return value;
  return value
    .split(/\r?\n/)
    .map((line) => `${color}${line.replaceAll(reset, `${reset}${color}`)}${reset}`)
    .join("\n");
}

function orderedMessageBlocks(message: Message): OrderedMessageBlock[] {
  if (message.role !== "assistant") {
    const text = displayMessageText(message.role, messageText(message));
    return text ? [{ kind: "text", text }] : [];
  }
  const blocks: OrderedMessageBlock[] = [];
  let pendingCommands: CommandInfo[] = [];
  const flushCommands = () => {
    if (pendingCommands.length) {
      blocks.push({ kind: "commands", commands: uniqueCommands(pendingCommands) });
      pendingCommands = [];
    }
  };
  for (const part of orderedPartsForDisplay(message.parts ?? [])) {
    const text = partText(part);
    if (text) {
      const display = displayMessageText(message.role, text);
      if (display) {
        flushCommands();
        blocks.push({ kind: "text", text: display });
      }
      continue;
    }
    const commands = commandsForPart(part);
    if (commands.length) {
      pendingCommands.push(...commands);
      continue;
    }
    const details = partTranscriptLines(part);
    if (details.length) {
      flushCommands();
      for (const detail of details) blocks.push({ kind: "detail", text: detail });
    }
  }
  flushCommands();
  return blocks;
}

function orderedPartsForDisplay(parts: MessagePart[]): MessagePart[] {
  return [...parts].sort((left, right) => partDisplayRank(left) - partDisplayRank(right));
}

function partDisplayRank(part: MessagePart): number {
  if (part.type === "text" || part.type === "message" || !part.type) return 0;
  if (part.tool || part.type === "tool") return 2;
  return 1;
}

function partText(part: MessagePart): string {
  if (part.type !== "text" && part.type !== "message" && part.type) return "";
  return part.text ?? part.content ?? "";
}

function richContentLine(content: string, cols: number, role = "assistant"): string {
  return panelLine(content, cols, role);
}

function richBlankRailLine(role = "assistant", cols = 80): string {
  return panelBlankLine(role, cols);
}

function addTranscriptGap(lines: string[]): void {
  if (!lines.length) return;
  if (activeCapabilities.level === "plain") {
    if (lines.at(-1) !== "") lines.push("");
    return;
  }
  lines.push("");
}

function uniqueCommands(commands: CommandInfo[]): CommandInfo[] {
  const seen = new Set<string>();
  const unique: CommandInfo[] = [];
  for (const item of commands) {
    const key = firstCommandLine(item.command);
    if (!key) continue;
    const dedupeKey = `${item.step ?? ""}\0${key}`;
    if (seen.has(dedupeKey)) continue;
    seen.add(dedupeKey);
    unique.push({ ...item, command: sanitizeRawTerminalText(item.command).trim() });
  }
  return unique;
}

function commandsForPart(part: MessagePart): CommandInfo[] {
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
  if (commands.length || part.tool !== "command_run") return commands;
  const summary =
    commandRunPayloadSummary(state.output) ??
    commandRunPayloadSummary(state.input) ??
    commandRunPayloadSummary(part.metadata) ??
    toolSummary(state).trim();
  return summary ? [{ command: summary, tool, status }] : [];
}

function commandInfosFromUnknown(
  value: unknown,
  tool: string,
  status: string | undefined,
): CommandInfo[] {
  if (!value) return [];
  if (typeof value === "string") {
    return extractCommandsFromUnknown(value).map((command) => ({ command, tool, status }));
  }
  if (Array.isArray(value))
    return value.flatMap((item) => commandInfosFromUnknown(item, tool, status));
  if (!isRecord(value) || isTaskStatusPayload(value)) return [];
  const commands: CommandInfo[] = [];
  const step = numberField(value, "step");
  const command = commandLineFromRecord(value);
  if (command) commands.push({ command, step, tool, status });
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
  return streamedCommandRunResults(state, part.metadata)
    .map((result, index) =>
      commandInfoFromStreamedResult(result, specs[index], tool, fallbackStatus),
    )
    .filter((command): command is CommandInfo => Boolean(command));
}

function streamedCommandRunResults(
  state: Record<string, unknown>,
  metadata: unknown,
): Record<string, unknown>[] {
  const output = recordLike(state.output);
  const stateStream = recordLike(state.streamed_command_run_result);
  const outputStream = recordLike(output.streamed_command_run_result);
  const metadataRecord = recordLike(metadata);
  const metadataOutput = recordLike(metadataRecord.output);
  const metadataStream = recordLike(metadataOutput.streamed_command_run_result);
  return [
    ...arrayField(stateStream, "results"),
    ...arrayField(outputStream, "results"),
    ...arrayField(output, "results"),
    ...arrayField(metadataStream, "results"),
  ].filter(
    (value): value is Record<string, unknown> => isRecord(value) && !isTaskStatusPayload(value),
  );
}

function commandSpecs(
  state: Record<string, unknown>,
  metadata: unknown,
): Record<string, unknown>[] {
  const input = recordLike(state.input);
  const metadataRecord = recordLike(metadata);
  const metadataInput = recordLike(metadataRecord.input);
  return [...arrayField(input, "commands"), ...arrayField(metadataInput, "commands")].filter(
    (value): value is Record<string, unknown> => isRecord(value),
  );
}

function commandInfoFromStreamedResult(
  result: Record<string, unknown>,
  spec: Record<string, unknown> | undefined,
  tool: string,
  fallbackStatus: string | undefined,
): CommandInfo | undefined {
  if (isTaskStatusCommand(result) || (spec && isTaskStatusCommand(spec))) return undefined;
  const command = commandLineFromStreamedResult(result, spec);
  if (!command) return undefined;
  return {
    command,
    step: commandStepFromStreamedResult(result, spec),
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
  return (
    numberField(result, "step") ??
    numberField(resultCommand, "step") ??
    numberField(spec ?? {}, "step") ??
    numberField(specCommand, "step")
  );
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
    stringField(result, "command") ??
    stringField(resultCommand, "command") ??
    stringField(spec ?? {}, "command") ??
    stringField(specCommand, "command") ??
    stringField(result, "command_type") ??
    stringField(spec ?? {}, "command_type");
  return command ? sanitizeRawTerminalText(command).trim() : undefined;
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

function isTaskStatusCommand(record: Record<string, unknown>): boolean {
  const commandType = stringField(record, "command_type") ?? stringField(record, "command");
  return (
    commandType
      ?.trim()
      .toLowerCase()
      .replace(/[-\s]+/g, "_") === "task_status"
  );
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
  if (typeof value === "number" && Number.isInteger(value) && value > 0) return value;
  if (typeof value === "string" && /^\d+$/u.test(value.trim())) {
    const parsed = Number(value.trim());
    return parsed > 0 ? parsed : undefined;
  }
  return undefined;
}

function commandLineFromRecord(record: Record<string, unknown>): string | undefined {
  for (const key of ["command_line", "command"]) {
    const value = record[key];
    if (typeof value === "string" && looksLikeCommand(value)) return firstCommandLine(value);
  }
  return undefined;
}

function commandRunPayloadSummary(value: unknown): string | undefined {
  if (!value) return undefined;
  if (isTaskStatusPayload(value)) return undefined;
  if (typeof value === "string") return compactPayloadField(value)?.trim() || undefined;
  if (Array.isArray(value)) {
    for (const item of value) {
      const summary = commandRunPayloadSummary(item);
      if (summary) return summary;
    }
    return undefined;
  }
  if (typeof value !== "object") return undefined;
  const object = value as Record<string, unknown>;
  for (const key of ["task_detail", "step_summary", "summary", "status", "label"]) {
    const item = object[key];
    if (typeof item === "string" && item.trim()) return sanitizeRawTerminalText(item).trim();
  }
  for (const key of ["input", "output", "metadata", "commands", "results", "steps"]) {
    const summary = commandRunPayloadSummary(object[key]);
    if (summary) return summary;
  }
  return undefined;
}

function commandSectionLines(
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
  const count = `${t("commands")}: ${commands.length}`;
  const running = commands.some((command) => commandIsRunning(command.status));
  const runningIcons = activeCapabilities.unicode ? ["◆", "◇", "◈"] : ["#", "*", "+"];
  const icon = activeCapabilities.unicode
    ? running
      ? (runningIcons[state.thinkingFrame % runningIcons.length] ?? "◆")
      : "◇"
    : running
      ? (runningIcons[state.thinkingFrame % runningIcons.length] ?? "#")
      : "*";
  const label = `${icon} ${count}`;
  return auxiliaryText(truncateAnsi(label, Math.max(12, cols - 2)));
}

function commandDetailLines(commands: CommandInfo[], state: AppState, _cols: number): string[] {
  const lines: string[] = [];
  for (const [index, command] of commands.entries()) {
    const isLast = index === commands.length - 1;
    const branch = activeCapabilities.unicode ? (isLast ? "└─" : "├─") : "|-";
    const symbol = statusSymbol(command.status, state.thinkingFrame);
    const meta = [command.tool ?? t("tool"), command.status].filter(Boolean).join(" ");
    const step = command.step ?? index + 1;
    const prefix = `${branch} ${stripAnsi(symbol)} #${step}${meta ? ` ${meta}` : ""}  $ `;
    const text = `${prefix}${firstCommandLine(command.command)}`;
    lines.push(auxiliaryText(text));
  }
  return lines;
}

function statusSymbol(status: string | undefined, frame: number): string {
  const normalized = (status ?? "").toLowerCase();
  if (/fail|error|reject|denied/.test(normalized)) return `${opencodePrimary}x${reset}`;
  if (commandIsRunning(status)) {
    const frames = activeCapabilities.unicode ? ["■", "□", "◧"] : ["#", "*", "+"];
    return `${opencodePrimary}${frames[frame % frames.length] ?? frames[0]}${reset}`;
  }
  if (/done|complete|success|ok/.test(normalized))
    return `${opencodePrimary}${activeCapabilities.unicode ? "✓" : "+"}${reset}`;
  return `${dim}${activeCapabilities.unicode ? "•" : "-"}${reset}`;
}

function commandIsRunning(status: string | undefined): boolean {
  return /run|progress|pending|busy|question|in[_ -]?progress|execut|start/i.test(status ?? "");
}

function isThinking(state: AppState, messages = displayMessages(state)): boolean {
  if (state.status !== "busy" && state.session?.status !== "busy") return false;
  if (state.questions.length || state.permissions.length) return true;
  const sessionID = state.session?.id;
  if (
    Object.values(state.liveStreams).some(
      (stream) => !sessionID || !stream.sessionID || stream.sessionID === sessionID,
    )
  )
    return true;
  if (!messages.length || messages.at(-1)?.role === "user") return true;
  return messages.some((message) =>
    (message.parts ?? []).some((part) => commandIsRunning(commandPartStatus(part))),
  );
}

function commandPartStatus(part: MessagePart): string | undefined {
  if (part.tool !== "command_run" && part.type !== "tool") return undefined;
  if (!part.state || typeof part.state !== "object") return undefined;
  const status = (part.state as { status?: unknown }).status;
  return typeof status === "string" ? status : undefined;
}

function thinkingLine(state: AppState, cols: number): string {
  const frame = activeCapabilities.unicode
    ? thinkingAnimationFrame(state.thinkingFrame, true)
    : (["|", "/", "-", "\\"][state.thinkingFrame % 4] ?? ".");
  const text = `${frame} thinking  ${secondsSinceLastUserMessage(state)}s`;
  if (activeCapabilities.level !== "plain")
    return panelLine(thinkingWaveText(text, state.thinkingFrame), cols);
  return secondaryText(text);
}

function thinkingWaveText(value: string, frame: number): string {
  const segments = graphemes(value);
  if (!segments.length) return value;
  const center = frame % (segments.length + 6);
  return segments
    .map((segment, index) => `${thinkingWaveColor(Math.abs(index - center))}${segment}${reset}`)
    .join("");
}

function thinkingWaveColor(distance: number): string {
  if (distance <= 0) return thinkingWaveMoon;
  if (distance <= 1) return thinkingWaveGlow;
  if (distance <= 2) return thinkingWaveMid;
  if (distance <= 3) return thinkingWaveLow;
  if (distance <= 4) return thinkingWaveBaseBlend;
  return textAuxiliary;
}

function graphemes(value: string): string[] {
  const segmenter =
    typeof Intl !== "undefined" && "Segmenter" in Intl
      ? new Intl.Segmenter(undefined, { granularity: "grapheme" })
      : undefined;
  return segmenter ? [...segmenter.segment(value)].map((item) => item.segment) : Array.from(value);
}

function secondsSinceLastUserMessage(state: AppState): number {
  const message = [...displayMessages(state)].reverse().find((item) => item.role === "user");
  const created = message?.created_at ?? message?.time?.created ?? message?.updated_at;
  if (!created || !Number.isFinite(created)) return 0;
  return Math.max(0, Math.floor((Date.now() - created) / 1000));
}

function partTranscriptLines(part: MessagePart): string[] {
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
