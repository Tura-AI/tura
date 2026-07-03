import { displayMessages, type AppState } from "../reducer.js";
import type { Message, MessagePart } from "../../types/session.js";
import { messagePartText, messageText } from "../../types/session.js";
import {
  activeCapabilities,
  richBlockBg,
  richHighlight,
  reset,
  stripAnsi,
  textAuxiliary,
  textPrimary,
  thinkingWaveBaseBlend,
  thinkingWaveGlow,
  thinkingWaveLow,
  thinkingWaveMid,
  visibleTextWidth,
  wrapAnsi,
} from "../render-terminal.js";
import {
  commandIsRunning,
  commandPartStatus,
  commandSectionLines,
  commandsForPart,
  partTranscriptLines,
  type CommandInfo,
} from "./commands.js";
import { displayMessageText, renderRichText } from "../render-rich-text.js";
import { thinkingAnimationFrame } from "./busy-animation.js";
import { panelBlankLine, panelLine } from "../styles/panel.js";
import { secondaryText } from "../styles/text.js";
import { isBusyState } from "../busy-state.js";

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

export type TranscriptLineKind = "message" | "command" | "gap";
type CommandDisplayMode = "cache" | "live";

type CommandDisplayPolicy = {
  includeCommands: boolean;
  showCommandDetails: boolean;
};

export type TranscriptRenderLine = {
  text: string;
  kind: TranscriptLineKind;
};

export function transcriptLines(state: AppState, cols: number, maxLines?: number): string[] {
  return transcriptRenderLines(state, cols, maxLines).map((line) => line.text);
}

export function transcriptRenderLines(
  state: AppState,
  cols: number,
  maxLines?: number,
): TranscriptRenderLine[] {
  if (maxLines !== undefined && maxLines <= 0) return [];
  const messages = splitTranscriptMessages(state).cache;
  return renderTranscriptMessages(state, cols, messages, {
    commandMode: "cache",
    maxLines,
  });
}

export function transcriptMessageGroups(state: AppState): { cache: Message[]; live: Message[] } {
  return splitTranscriptMessages(state);
}

export function transcriptRenderLinesForMessages(
  state: AppState,
  cols: number,
  messages: Message[],
  options: { commandMode?: CommandDisplayMode; maxLines?: number } = {},
): TranscriptRenderLine[] {
  if (options.maxLines !== undefined && options.maxLines <= 0) return [];
  return renderTranscriptMessages(state, cols, messages, {
    commandMode: options.commandMode ?? "cache",
    maxLines: options.maxLines,
  });
}

export function transcriptLiveLines(state: AppState, cols: number): string[] {
  return transcriptLiveRenderLines(state, cols).map((line) => line.text);
}

export function transcriptLiveRenderLines(state: AppState, cols: number): TranscriptRenderLine[] {
  return renderTranscriptMessages(state, cols, splitTranscriptMessages(state).live, {
    commandMode: "live",
  });
}

export function transcriptThinkingLines(state: AppState, cols: number): string[] {
  return ["", isThinking(state) ? thinkingLine(state, cols) : ""];
}

function renderTranscriptMessages(
  state: AppState,
  cols: number,
  messages: Message[],
  options: { commandMode: CommandDisplayMode; maxLines?: number },
): TranscriptRenderLine[] {
  const commandPolicy = commandDisplayPolicy(state, options.commandMode);
  const lines: TranscriptRenderLine[] = [];
  const renderedMessages =
    options.maxLines === undefined
      ? messages.map((message) => renderTranscriptMessage(message, state, cols, commandPolicy))
      : tailRenderedMessages(messages, state, cols, commandPolicy, options.maxLines);
  for (const rendered of renderedMessages) {
    addTranscriptGap(lines);
    lines.push(...rendered);
  }
  if (options.maxLines === undefined) return lines;
  return transcriptOutputLines(lines, options.maxLines);
}

function commandDisplayPolicy(
  state: AppState,
  commandMode: CommandDisplayMode,
): CommandDisplayPolicy {
  if (commandMode === "live") {
    return { includeCommands: true, showCommandDetails: true };
  }
  const showCommands = state.sessionConfig?.show_command_instructions !== false;
  return {
    includeCommands: showCommands,
    showCommandDetails: showCommands,
  };
}

function splitTranscriptMessages(state: AppState): { cache: Message[]; live: Message[] } {
  const messages = displayMessages(state);
  const liveIDs = liveMessageIDs(state, messages);
  if (!liveIDs.size) return { cache: messages, live: [] };
  const liveStart = messages.findIndex((message) => liveIDs.has(message.id));
  if (liveStart < 0) return { cache: messages, live: [] };
  return {
    cache: messages.slice(0, liveStart),
    live: messages.slice(liveStart),
  };
}

function liveMessageIDs(state: AppState, messages: Message[]): Set<string> {
  const sessionID = state.session?.id;
  const ids = new Set(
    Object.values(state.liveStreams)
      .filter((stream) => !sessionID || !stream.sessionID || stream.sessionID === sessionID)
      .map((stream) => stream.messageID),
  );
  for (const message of messages) {
    if (message.role === "assistant" && messageHasRunningPart(message)) ids.add(message.id);
  }
  return ids;
}

function messageHasRunningPart(message: Message): boolean {
  return (message.parts ?? []).some((part) => commandIsRunning(commandPartStatus(part)));
}

function tailRenderedMessages(
  messages: Message[],
  state: AppState,
  cols: number,
  commandPolicy: CommandDisplayPolicy,
  maxLines: number,
): TranscriptRenderLine[][] {
  const renderedMessages: TranscriptRenderLine[][] = [];
  const targetLines = Math.max(maxLines + 20, maxLines * 3);
  let renderedLineCount = 0;
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    const rendered = renderTranscriptMessage(message, state, cols, commandPolicy);
    if (!rendered.length) continue;
    renderedMessages.unshift(rendered);
    renderedLineCount += rendered.length + 1;
    if (renderedLineCount >= targetLines) break;
  }
  return renderedMessages;
}

function transcriptOutputLines(
  lines: TranscriptRenderLine[],
  maxLines: number,
): TranscriptRenderLine[] {
  if (maxLines <= 0) return [];
  return lines.slice(-maxLines);
}

function renderTranscriptMessage(
  message: Message,
  state: AppState,
  cols: number,
  commandPolicy: CommandDisplayPolicy,
): TranscriptRenderLine[] {
  return activeCapabilities.level === "plain"
    ? renderSimpleMessage(message, state, cols, commandPolicy)
    : renderRichMessage(message, state, cols, commandPolicy);
}

function renderSimpleMessage(
  message: Message,
  state: AppState,
  cols: number,
  commandPolicy: CommandDisplayPolicy,
): TranscriptRenderLine[] {
  const lines: TranscriptRenderLine[] = [];
  const prefixWidth = activeCapabilities.unicode ? 4 : 3;
  const contentWidth = Math.max(20, cols - prefixWidth - 2);

  if (message.role === "user") {
    const text = displayMessageText("user", messageText(message));
    const rendered = secondaryText(
      stripAnsi(renderRichText(text, richTextOptions(contentWidth, state))),
    );
    for (const line of wrapAnsi(rendered, contentWidth)) {
      lines.push(messageLine(simpleBodyLine(line, "user", cols)));
    }
    return lines;
  }

  for (const block of orderedMessageBlocks(message, commandPolicy.includeCommands)) {
    if (lines.length) lines.push(gapLine());
    if (block.kind === "text") {
      const richText = renderRichText(block.text, richTextOptions(contentWidth, state));
      const displayText =
        message.role === "assistant" ? agentText(richText) : secondaryText(stripAnsi(richText));
      for (const line of wrapAnsi(displayText, contentWidth)) {
        lines.push(messageLine(simpleBodyLine(line, message.role, cols)));
      }
      continue;
    }
    if (block.kind === "detail") {
      for (const line of wrapAnsi(secondaryText(block.text), contentWidth)) {
        lines.push(messageLine(simpleBodyLine(line, message.role, cols)));
      }
      continue;
    }
    lines.push(...commandRenderLines(block.commands, state, cols, cols, commandPolicy));
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
  commandPolicy: CommandDisplayPolicy,
): TranscriptRenderLine[] {
  const lines: TranscriptRenderLine[] = [];
  const contentWidth = richMessageContentWidth(cols);

  if (message.role === "user") {
    const userText = displayMessageText("user", messageText(message));
    const body = secondaryText(
      stripAnsi(renderRichText(userText, richTextOptions(contentWidth, state))),
    );
    const wrapped = body ? wrapAnsi(body, contentWidth) : [];
    if (wrapped.length) {
      lines.push(gapLine(richBlankRailLine("user", cols)));
      for (const line of wrapped) lines.push(messageLine(richContentLine(line, cols, "user")));
      lines.push(gapLine(richBlankRailLine("user", cols)));
    }
    return lines;
  }

  const blocks = orderedMessageBlocks(message, commandPolicy.includeCommands);
  if (!blocks.length && message.role !== "assistant") {
    lines.push(
      messageLine(richContentLine(`${textAuxiliary}${message.role}${reset}`, cols, message.role)),
    );
  }
  for (const block of blocks) {
    if (lines.length) lines.push(gapLine());
    if (block.kind === "text") {
      const richText = renderRichText(block.text, richTextOptions(contentWidth, state));
      const displayText =
        message.role === "assistant" ? agentText(richText) : secondaryText(stripAnsi(richText));
      const wrapped = wrapAnsi(displayText, contentWidth);
      if (message.role === "assistant") lines.push(gapLine(richBlankRailLine(message.role, cols)));
      for (const line of wrapped)
        lines.push(messageLine(richContentLine(line, cols, message.role)));
      if (message.role === "assistant") lines.push(gapLine(richBlankRailLine(message.role, cols)));
      continue;
    }
    if (block.kind === "detail") {
      const wrapped = wrapAnsi(secondaryText(block.text), contentWidth);
      lines.push(gapLine(richBlankRailLine(message.role, cols)));
      for (const line of wrapped) {
        lines.push(messageLine(richContentLine(secondaryText(line), cols, message.role)));
      }
      lines.push(gapLine(richBlankRailLine(message.role, cols)));
      continue;
    }
    lines.push(...commandRenderLines(block.commands, state, cols - 6, cols, commandPolicy));
  }
  return lines;
}

function richMessageContentWidth(cols: number): number {
  return Math.max(20, cols - 1);
}

function richTextOptions(
  contentWidth: number,
  state: AppState,
): { tableWidth: number; workspaceDirectory?: string } {
  return {
    tableWidth: Math.max(20, contentWidth - 2),
    workspaceDirectory: state.session?.directory ?? state.cwd,
  };
}

function agentText(value: string): string {
  if (!value) return value;
  return colorEachLine(value, textPrimary);
}

function colorEachLine(value: string, color: string): string {
  if (!value) return value;
  return value
    .split(/\r?\n/)
    .map((line) => `${color}${line.replaceAll(reset, `${reset}${color}`)}${reset}`)
    .join("\n");
}

function orderedMessageBlocks(message: Message, includeCommands: boolean): OrderedMessageBlock[] {
  if (message.role !== "assistant") {
    const text = displayMessageText(message.role, messageText(message));
    return text ? [{ kind: "text", text }] : [];
  }
  const blocks: OrderedMessageBlock[] = [];
  for (const part of orderedPartsForDisplay(message.parts ?? [])) {
    const text = partText(part);
    if (text) {
      const display = displayMessageText(message.role, text);
      if (display) blocks.push({ kind: "text", text: display });
      continue;
    }
    if (!includeCommands) continue;
    const commands = commandsForPart(part);
    if (commands.length) {
      blocks.push({
        kind: "commands",
        commands,
      });
      continue;
    }
    const details = partTranscriptLines(part);
    if (details.length) {
      for (const detail of details) blocks.push({ kind: "detail", text: detail });
    }
  }
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
  return messagePartText(part);
}

function richContentLine(content: string, cols: number, role = "assistant"): string {
  return panelLine(expandRichBlockBackground(content, cols), cols, role);
}

function expandRichBlockBackground(content: string, cols: number): string {
  if (activeCapabilities.level !== "rich" || !content.includes(richBlockBg)) return content;
  const innerWidth = Math.max(1, cols - 3);
  const missing = innerWidth - visibleTextWidth(content);
  if (missing <= 0) return content;
  return `${content}${richBlockBg}${" ".repeat(missing)}${reset}`;
}

function richBlankRailLine(role = "assistant", cols = 80): string {
  return panelBlankLine(role, cols);
}

function messageLine(text: string): TranscriptRenderLine {
  return { text, kind: "message" };
}

function gapLine(text = ""): TranscriptRenderLine {
  return { text, kind: "gap" };
}

function commandRenderLines(
  commands: CommandInfo[],
  state: AppState,
  summaryCols: number,
  detailCols: number,
  commandPolicy: CommandDisplayPolicy,
): TranscriptRenderLine[] {
  return commandSectionLines(
    commands,
    state,
    summaryCols,
    detailCols,
    commandPolicy.showCommandDetails,
  ).map((text) => ({
    text,
    kind: "command",
  }));
}

function addTranscriptGap(lines: TranscriptRenderLine[]): void {
  if (!lines.length) return;
  if (activeCapabilities.level === "plain") {
    if (lines.at(-1)?.text !== "") lines.push(gapLine());
    return;
  }
  lines.push(gapLine());
}

function isThinking(state: AppState): boolean {
  return isBusyState(state);
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

export function thinkingWaveText(value: string, frame: number): string {
  const segments = graphemes(value);
  if (!segments.length) return value;
  const center = frame % (segments.length + 6);
  return segments
    .map((segment, index) => `${thinkingWaveColor(Math.abs(index - center))}${segment}${reset}`)
    .join("");
}

function thinkingWaveColor(distance: number): string {
  if (distance <= 0) return richHighlight;
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
