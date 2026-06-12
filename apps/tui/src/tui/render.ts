import type { AppState, SettingDetail } from "./reducer.js";
import type { Message, MessagePart } from "../types/session.js";
import { messageText, sessionTitle } from "../types/session.js";
import { t } from "../i18n.js";
import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";
import {
  activeCapabilities,
  bold,
  dim,
  gray,
  opencodeBorder,
  opencodePanelBg,
  opencodePrimary,
  opencodeText,
  opencodeTextWeak,
  pad,
  padVisible,
  reset,
  rule,
  setActiveCapabilities,
  stripAnsi,
  truncate,
  truncateAnsi,
  visibleTextWidth,
  wrap,
  wrapAnsi,
} from "./render-terminal.js";
import {
  compactInlinePayloads,
  compactPayloadField,
  displayMessageText,
  extractCommandsFromUnknown,
  firstCommandLine,
  isTaskStatusPayload,
  renderRichText,
  sanitizeRawTerminalText,
  toolSummary,
} from "./render-rich-text.js";
import { SplitBorder, SplitBorderFallback } from "./ui/border.js";

const COMPOSER_CURSOR_MARKER = "\x01\x02\x01";

type CommandInfo = {
  command: string;
  tool?: string;
  status?: string;
};

export type RenderedFrame = {
  frame: string;
  cursor?: { row: number; column: number };
};

export function render(
  state: AppState,
  capabilities: TerminalCapabilities = detectTerminalCapabilities(),
): string {
  return renderFrame(state, capabilities).frame;
}

export function renderFrame(
  state: AppState,
  capabilities: TerminalCapabilities = detectTerminalCapabilities(),
): RenderedFrame {
  setActiveCapabilities(capabilities);
  if (capabilities.level === "plain") return renderPlainFrame(state);
  const rows = process.stdout.rows || 30;
  const cols = process.stdout.columns || 100;
  const renderCols = terminalRenderCols(cols);
  const lines: string[] = [];
  lines.push(topBar(state, renderCols));

  if (state.help) {
    lines.push(...layoutSeparator(renderCols));
    lines.push(...helpLines(renderCols, rows - 7));
  } else if (state.sessionsOpen) {
    lines.push(...layoutSeparator(renderCols));
    lines.push(...sessionLines(state, renderCols, rows - 7));
  } else if (state.authOpen) {
    lines.push(...layoutSeparator(renderCols));
    lines.push(...authLines(state, renderCols, rows - 7));
  } else if (state.settingsOpen) {
    lines.push(...layoutSeparator(renderCols));
    lines.push(...settingsLines(state, renderCols, rows - 7));
  } else if (state.personasOpen) {
    lines.push(...layoutSeparator(renderCols));
    lines.push(...personaLines(state, renderCols, rows - 7));
  } else if (state.modelsOpen) {
    lines.push(...layoutSeparator(renderCols));
    lines.push(...modelLines(state, renderCols, rows - 7));
  } else {
    // Chat transcript: title sits directly above the conversation (no extra
    // blank line) so more history stays on screen.
    lines.push(
      ...transcriptLines(state, renderCols, transcriptMaxLines(rows, renderCols, state.composer)),
    );
  }

  if (state.permissions.length) {
    lines.push(...layoutSeparator(renderCols));
    for (const permission of state.permissions.slice(0, 3)) {
      const hint = `/approve ${permission.id} /deny ${permission.id}`;
      lines.push(
        `${richPrimary()}${t("permissions")}${reset} ${permission.id} ${permission.permission} ${hintText(hint)}`,
      );
    }
  }
  if (state.questions.length) {
    lines.push(...layoutSeparator(renderCols));
    for (const question of state.questions.slice(0, 3)) {
      const hint = t("answerHint", { id: question.id });
      lines.push(
        `${richPrimary()}${t("question")}${reset} ${question.id} ${truncate(question.question, Math.max(12, cols - 34))} ${hintText(hint)}`,
      );
    }
  }
  if (state.notice) lines.push(...noticeLines(state.notice, renderCols));
  const footerStart = lines.length;
  lines.push(bottomMetaLine(state, renderCols));
  if ((!state.settingsOpen || state.settingInput) && !state.sessionsOpen) {
    lines.push(...composerSeparator(renderCols));
    lines.push(
      ...composerLines(
        state.composer,
        renderCols,
        state.thinkingFrame,
        state.settingInput ? t("settingInputComposerHint") : undefined,
      ),
    );
  }
  return finalizeFrame(lines, Math.max(1, rows - 1), renderCols, footerStart);
}

function transcriptMaxLines(rows: number, cols: number, composer: string): number {
  const composerRows =
    activeCapabilities.level === "plain"
      ? Math.max(1, wrap(composer || "", Math.max(20, cols - 3)).length) + 1
      : Math.min(4, Math.max(1, wrap(composer || "", Math.max(20, cols - 6)).length)) + 2;
  return Math.max(0, rows - composerRows - 5);
}

function layoutSeparator(cols: number): string[] {
  if (activeCapabilities.level !== "plain") return [""];
  return [rule(cols)];
}

function composerSeparator(cols: number): string[] {
  if (activeCapabilities.level === "plain") return layoutSeparator(cols);
  return [""];
}

function renderPlainFrame(state: AppState): RenderedFrame {
  const cols = process.stdout.columns || 100;
  const rows = process.stdout.rows || 30;
  const renderCols = terminalRenderCols(cols);
  const lines: string[] = [];
  lines.push(stripAnsi(topBar(state, renderCols)));
  lines.push("");
  if (state.help) lines.push(...helpLines(renderCols, Math.max(4, rows - 5)));
  else if (state.sessionsOpen) lines.push(...sessionLines(state, renderCols, 20));
  else if (state.authOpen) lines.push(...authLines(state, renderCols, 20));
  else if (state.settingsOpen) lines.push(...settingsLines(state, renderCols, 20));
  else if (state.personasOpen) lines.push(...personaLines(state, renderCols, 20));
  else if (state.modelsOpen) lines.push(...modelLines(state, renderCols, 20));
  else
    lines.push(
      ...transcriptLines(state, renderCols, transcriptMaxLines(rows, renderCols, state.composer)),
    );
  if (state.notice) lines.push(...noticeLines(state.notice, renderCols));
  const footerStart = lines.length;
  lines.push(stripAnsi(bottomMetaLine(state, renderCols)));
  if ((!state.settingsOpen || state.settingInput) && !state.sessionsOpen) {
    lines.push("");
    lines.push(
      ...composerLines(
        state.composer,
        renderCols,
        state.thinkingFrame,
        state.settingInput ? t("settingInputComposerHint") : undefined,
      ),
    );
  }
  const rendered = finalizeFrame(lines, Math.max(1, rows - 1), renderCols, footerStart);
  return { ...rendered, frame: stripAnsi(rendered.frame) };
}

function finalizeFrame(
  lines: string[],
  rows: number,
  cols: number,
  footerStart = lines.length,
): RenderedFrame {
  const fitted = fitWithPinnedFooter(lines, rows, cols, footerStart);
  const cursor = findComposerCursor(fitted);
  return {
    frame: fitted.map((line) => line.replace(COMPOSER_CURSOR_MARKER, "")).join("\n"),
    cursor,
  };
}

function fitWithPinnedFooter(
  lines: string[],
  rows: number,
  cols: number,
  footerStart: number,
): string[] {
  const footer = lines.slice(footerStart).map((line) => truncateAnsi(line, cols));
  const contentRows = Math.max(0, rows - footer.length);
  const content = lines
    .slice(0, footerStart)
    .slice(0, contentRows)
    .map((line) => truncateAnsi(line, cols));
  return [...content, ...footer].slice(0, rows);
}

function findComposerCursor(lines: string[]): RenderedFrame["cursor"] {
  for (const [rowIndex, line] of lines.entries()) {
    const markerIndex = line.indexOf(COMPOSER_CURSOR_MARKER);
    if (markerIndex < 0) continue;
    return {
      row: rowIndex + 1,
      column: Math.max(1, visibleTextWidth(line.slice(0, markerIndex)) + 1),
    };
  }
  return undefined;
}

function terminalRenderCols(cols: number): number {
  return Math.max(20, cols - 1);
}

function topBar(state: AppState, cols: number): string {
  const title = state.session ? sessionTitle(state.session) : "tura";
  const color =
    activeCapabilities.level === "rich"
      ? opencodePrimary
      : activeCapabilities.level === "ansi"
        ? opencodePrimary
        : bold;
  return truncateAnsi(`${color}${title}${reset}`, cols);
}

function bottomMetaLine(state: AppState, cols: number): string {
  const pieces = bottomMetaPieces(state);
  if (activeCapabilities.level === "plain") {
    return truncateAnsi(`${dim}${pieces.join("  ")}${reset}`, cols);
  }
  return truncateAnsi(`${opencodeTextWeak}${pieces.join(bottomMetaDivider())}${reset}`, cols);
}

function bottomMetaPieces(state: AppState): string[] {
  const model = [
    state.session?.model ?? state.sessionConfig?.model ?? state.sessionConfig?.active_model,
    state.session?.model_variant ?? state.sessionConfig?.model_variant,
    (state.session?.model_acceleration_enabled ?? state.sessionConfig?.model_acceleration_enabled)
      ? t("priority")
      : undefined,
  ]
    .filter(Boolean)
    .join(" ");
  return [statusIndicator(state), model || "-", tokenSummary(state)];
}

function bottomMetaDivider(): string {
  return `${opencodeBorder} │ ${reset}${opencodeTextWeak}`;
}

function hintText(value: string): string {
  const color = activeCapabilities.level === "rich" ? opencodeTextWeak : dim;
  return `${color}${value}${reset}`;
}

function statusIndicator(state: AppState): string {
  if (activeCapabilities.unicode) {
    if (state.status === "busy") return ["◇", "◆", "◈", "◆"][state.thinkingFrame % 4] ?? "◇";
    if (state.status === "error") return "x";
    return "◇";
  }
  if (state.status === "busy") return ["-", "\\", "|", "/"][state.thinkingFrame % 4] ?? "-";
  if (state.status === "error") return "x";
  return "-";
}

function tokenSummary(state: AppState): string {
  const total = state.messages.reduce((sum, message) => sum + tokenTotal(message.tokens), 0);
  return `tokens ${total || "-"}`;
}

function tokenTotal(value: unknown): number {
  if (!value || typeof value !== "object") return 0;
  const record = value as Record<string, unknown>;
  for (const key of ["total_tokens", "total", "tokens"]) {
    const current = record[key];
    if (typeof current === "number" && Number.isFinite(current)) return current;
  }
  return (
    numberField(record, "input_tokens") +
    numberField(record, "prompt_tokens") +
    numberField(record, "input") +
    numberField(record, "output_tokens") +
    numberField(record, "completion_tokens") +
    numberField(record, "output") +
    numberField(record, "reasoning_tokens") +
    numberField(record, "reasoning_output_tokens") +
    numberField(record, "reasoning") +
    numberField(record, "cached_input_tokens") +
    numberField(record, "cache_read_input_tokens") +
    nestedNumberField(record, "cache", "read") +
    nestedNumberField(record, "cache", "write")
  );
}

function numberField(record: Record<string, unknown>, key: string): number {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function nestedNumberField(
  record: Record<string, unknown>,
  key: string,
  nestedKey: string,
): number {
  const value = record[key];
  if (!value || typeof value !== "object") return 0;
  return numberField(value as Record<string, unknown>, nestedKey);
}

function richPrimary(): string {
  return activeCapabilities.level === "plain" ? bold : opencodePrimary;
}

function transcriptLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines: string[] = [];
  const showCommands = state.sessionConfig?.show_command_instructions !== false;
  // Render up to last 100 messages to give scroll room without unbounded rendering cost.
  // Message order is the layout contract: user, agent text, and command blocks stay
  // exactly where the gateway inserted them. Do not regroup assistant messages by turn.
  for (const message of state.messages.slice(-100)) {
    const rendered = renderTranscriptMessage(message, state, cols, showCommands);
    if (!rendered.length) continue;
    addTranscriptGap(lines);
    lines.push(...rendered);
  }
  if (isThinking(state)) {
    addTranscriptGap(lines);
    lines.push(thinkingLine(state, cols));
  }
  return viewportLines(lines, maxLines, state.scrollOffset);
}

function viewportLines(lines: string[], maxLines: number, scrollOffset: number): string[] {
  if (maxLines <= 0) return [];
  if (scrollOffset === 0) return smartViewportLines(lines, maxLines);
  const maxOffset = Math.max(0, lines.length - maxLines);
  const offset = Math.min(scrollOffset, maxOffset);
  const bottom = lines.length - offset;
  const top = Math.max(0, bottom - maxLines);
  return lines.slice(top, bottom);
}

function smartViewportLines(lines: string[], maxLines: number): string[] {
  return lines.slice(Math.max(0, lines.length - maxLines));
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
      lines.push(simpleBodyLine(line, "user", true, cols));
    }
    return lines;
  }

  for (const block of orderedMessageBlocks(message)) {
    if (lines.length) lines.push("");
    if (block.kind === "text") {
      const richText = renderRichText(block.text);
      const displayText =
        message.role === "assistant" ? richText : secondaryText(stripAnsi(richText));
      for (const line of wrapAnsi(displayText, contentWidth)) {
        lines.push(simpleBodyLine(line, message.role, false, cols));
      }
      continue;
    }
    if (block.kind === "detail") {
      for (const line of wrapAnsi(secondaryText(block.text), contentWidth)) {
        lines.push(simpleBodyLine(line, message.role, false, cols));
      }
      continue;
    }
    lines.push(...commandSectionLines(block.commands, state, cols, cols, showCommands));
  }
  return lines;
}

function simpleBodyLine(line: string, role: string, _user: boolean, cols = 80): string {
  if (activeCapabilities.level === "plain") return `  ${stripAnsi(line)}`;
  return splitBorderPanelLine(line, cols, role, opencodePanelBg);
}

function simpleSpacerLine(role = "assistant", cols = 80): string {
  if (activeCapabilities.level === "plain") return "";
  return splitBorderPanelBlank(role, cols, opencodePanelBg);
}

function railCell(role: string, background = ""): string {
  const border = activeCapabilities.unicode ? SplitBorder : SplitBorderFallback;
  const rail = border.customBorderChars.vertical;
  return `${background}${role === "user" ? opencodeText : opencodeTextWeak}${rail}${reset}`;
}

function splitBorderPanelLine(
  content: string,
  cols: number,
  role = "assistant",
  background = opencodePanelBg,
): string {
  return `${railCell(role, background)}${coloredPanelBand(content, cols, background)}`;
}

function splitBorderPanelBlank(
  role = "assistant",
  cols = 80,
  background = opencodePanelBg,
): string {
  return splitBorderPanelLine("", cols, role, background);
}

function renderRichMessage(
  message: Message,
  state: AppState,
  cols: number,
  showCommands: boolean,
): string[] {
  const lines: string[] = [];
  const contentWidth = Math.max(20, cols - 8);

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
        message.role === "assistant" ? richText : secondaryText(stripAnsi(richText));
      const wrapped = wrapAnsi(displayText, contentWidth);
      if (message.role === "assistant") lines.push(richBlankRailLine(message.role, cols));
      for (const line of wrapped) lines.push(richContentLine(line, cols, message.role));
      if (message.role === "assistant") lines.push(richBlankRailLine(message.role, cols));
      continue;
    }
    if (block.kind === "detail") {
      lines.push(richBlankRailLine(message.role, cols));
      for (const line of wrapAnsi(secondaryText(block.text), Math.max(20, cols - 8))) {
        lines.push(richContentLine(secondaryText(line), cols, message.role));
      }
      lines.push(richBlankRailLine(message.role, cols));
      continue;
    }
    lines.push(...commandSectionLines(block.commands, state, cols - 6, cols, showCommands));
  }
  return lines;
}
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

function orderedMessageBlocks(message: Message): OrderedMessageBlock[] {
  if (message.role !== "assistant") {
    const text = displayMessageText(message.role, messageText(message));
    return text ? [{ kind: "text", text }] : [];
  }
  const blocks: OrderedMessageBlock[] = [];
  // Accumulate consecutive command parts so a single assistant message renders
  // one aggregated "Commands: N" summary (deduped) rather than one per part.
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
  return splitBorderPanelLine(content, cols, role, opencodePanelBg);
}

function richBlankRailLine(role = "assistant", cols = 80): string {
  return splitBorderPanelBlank(role, cols, opencodePanelBg);
}

function commandDetailLine(content: string, cols: number): string {
  return truncateAnsi(content, cols);
}

function coloredPanelBand(content: string, cols: number, background: string): string {
  const innerWidth = Math.max(1, cols - 3);
  const visible = truncateAnsi(content, innerWidth);
  const padded = padVisible(visible, innerWidth).replaceAll(reset, `${reset}${background}`);
  return `${background} ${padded} ${reset}`;
}

function addTranscriptGap(lines: string[], _role = "assistant", _cols = 80): void {
  if (!lines.length) return;
  if (activeCapabilities.level === "plain") {
    if (lines.at(-1) !== "") lines.push("");
    return;
  }
  lines.push("");
}

function secondaryText(value: string): string {
  if (!value) return value;
  return `${gray}${value.replaceAll(reset, `${reset}${gray}`)}${reset}`;
}

function uniqueCommands(commands: CommandInfo[]): CommandInfo[] {
  const seen = new Set<string>();
  const unique: CommandInfo[] = [];
  for (const item of commands) {
    const command = firstCommandLine(item.command);
    if (!command || seen.has(command)) continue;
    seen.add(command);
    unique.push({ ...item, command });
  }
  return unique;
}

function commandsForPart(part: MessagePart): CommandInfo[] {
  const state =
    part.state && typeof part.state === "object" ? (part.state as Record<string, unknown>) : {};
  const status = typeof state.status === "string" ? state.status : undefined;
  const tool = part.tool ?? t("tool");
  const commands = [
    ...extractCommandsFromUnknown(state.input).map((command) => ({ command, tool, status })),
    ...extractCommandsFromUnknown(state.output).map((command) => ({ command, tool, status })),
    ...extractCommandsFromUnknown(part.metadata).map((command) => ({ command, tool, status })),
  ];
  if (commands.length || part.tool !== "command_run") return commands;
  const summary =
    commandRunPayloadSummary(state.output) ??
    commandRunPayloadSummary(state.input) ??
    commandRunPayloadSummary(part.metadata) ??
    toolSummary(state).trim();
  return summary ? [{ command: summary, tool, status }] : [];
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
  const lines = [commandDetailLine(commandSummaryLine(commands, state, summaryCols), detailCols)];
  if (showCommands) {
    for (const line of commandDetailLines(commands, state, summaryCols)) {
      lines.push(commandDetailLine(line, detailCols));
    }
  }
  return lines;
}

function commandSummaryLine(commands: CommandInfo[], state: AppState, cols: number): string {
  const count = `${t("commands")}: ${commands.length}`;
  const running = commands.some((command) =>
    /run|progress|pending|busy|question/i.test(command.status ?? ""),
  );
  const icon = activeCapabilities.unicode
    ? running
      ? state.thinkingFrame % 2 === 0
        ? "◆"
        : "◇"
      : "◇"
    : running
      ? "#"
      : "*";
  const label = `${icon} ${count}`;
  return secondaryText(truncateAnsi(label, Math.max(12, cols - 2)));
}

function commandDetailLines(commands: CommandInfo[], state: AppState, cols: number): string[] {
  const lines: string[] = [];
  for (const [index, command] of commands.entries()) {
    const isLast = index === commands.length - 1;
    const branch = activeCapabilities.unicode ? (isLast ? "└─" : "├─") : "|-";
    const symbol = statusSymbol(command.status, state.thinkingFrame);
    const meta = [command.tool ?? t("tool"), command.status].filter(Boolean).join(" ");
    const prefix = `${branch} ${stripAnsi(symbol)} #${index + 1}${meta ? ` ${meta}` : ""}  $ `;
    lines.push(secondaryText(truncateAnsi(`${prefix}${command.command}`, Math.max(20, cols - 2))));
  }
  return lines;
}

function statusSymbol(status: string | undefined, frame: number): string {
  const normalized = (status ?? "").toLowerCase();
  if (/fail|error|reject|denied/.test(normalized)) return `${opencodePrimary}x${reset}`;
  if (/run|progress|pending|busy|question/.test(normalized))
    return `${opencodePrimary}${activeCapabilities.unicode ? (frame % 2 === 0 ? "■" : "□") : "#"}${reset}`;
  if (/done|complete|success|ok/.test(normalized))
    return `${opencodePrimary}${activeCapabilities.unicode ? "✓" : "+"}${reset}`;
  return `${dim}${activeCapabilities.unicode ? "•" : "-"}${reset}`;
}

function isThinking(state: AppState): boolean {
  return state.status === "busy" || state.session?.status === "busy";
}

function thinkingLine(state: AppState, cols: number): string {
  const frames = activeCapabilities.unicode
    ? ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
    : ["|", "/", "-", "\\"];
  const frame = frames[state.thinkingFrame % frames.length] ?? ".";
  const text = `${frame} thinking  ${secondsSinceLastUserMessage(state)}s`;
  if (activeCapabilities.level !== "plain")
    return splitBorderPanelLine(secondaryText(text), cols, "assistant", opencodePanelBg);
  return secondaryText(text);
}

function secondsSinceLastUserMessage(state: AppState): number {
  const message = [...state.messages].reverse().find((item) => item.role === "user");
  const created = message?.created_at ?? message?.time?.created ?? message?.updated_at;
  if (!created || !Number.isFinite(created)) return 0;
  return Math.max(0, Math.floor((Date.now() - created) / 1000));
}

function sessionStateMarker(state: AppState, session: AppState["sessions"][number]): string {
  if (session.status === "busy")
    return state.thinkingFrame % 2 === 0
      ? activeCapabilities.unicode
        ? "◆"
        : "#"
      : activeCapabilities.unicode
        ? "◇"
        : "o";
  if (session.id === state.session?.id) return "";
  const seen = state.seenSessionMessageCounts[session.id] ?? session.message_count ?? 0;
  const count = session.message_count ?? 0;
  return count > seen ? (activeCapabilities.unicode ? "◆" : "#") : "";
}

function lastMessagePreview(messages: Message[]): string {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const text = messageText(messages[index]).replace(/\s+/g, " ").trim();
    if (text) return text;
  }
  return "";
}

function sessionLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = sectionLines(t("sessions"), cols);
  lines.push(
    sectionBodyLine(
      secondaryText(`${t("selectSessions")}  ${t("enterResume")}  ${t("createSession")}`),
      cols,
    ),
  );
  if (!state.sessions.length) {
    lines.push(sectionBodyLine(t("noSessions"), cols), sectionBlankLine(cols));
    return lines;
  }
  const entries = state.sessions.map((session) => {
    const marker = sessionStateMarker(state, session);
    const label = [sessionTitle(session), marker].filter(Boolean).join(" ");
    const preview =
      state.sessionPreviews[session.id] ||
      (session.id === state.session?.id ? lastMessagePreview(state.messages) : "");
    return [label, preview] as [string, string];
  });
  const width = menuLabelWidth(cols);
  for (const [index, [label, description]] of entries.entries()) {
    if (lines.length + 1 >= maxLines - 2) break;
    lines.push(
      sessionEntryLine(label, description, width, cols, index === state.selectedSessionIndex),
    );
  }
  lines.push(sectionBlankLine(cols));
  return lines.slice(0, maxLines);
}

function authLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = sectionLines(t("providerLogin"), cols);
  lines.push(
    sectionBodyLine(
      secondaryText(`${t("loginProvider")} ${t("startsAuth")}  ${t("logoutProvider")}`),
      cols,
    ),
  );
  const providers = state.providers?.all ?? [];
  if (!providers.length) {
    lines.push(sectionBodyLine(t("noProviders"), cols), sectionBlankLine(cols));
    return lines;
  }
  for (const [index, provider] of providers.entries()) {
    const status = state.authStatuses[provider.id];
    const methods = state.authMethods?.[provider.id] ?? [];
    const connected = status?.authenticated || state.providers?.connected.includes(provider.id);
    const marker = connected ? t("connected") : t("needsLogin");
    const statusText = [
      provider.name,
      marker,
      provider.source,
      status?.login ? `${t("loginState")}:${status.login}` : undefined,
      status?.auth_state ? `${t("authState")}:${status.auth_state}` : undefined,
      status?.runtime_state ? `${t("runtime")}:${status.runtime_state}` : undefined,
      status?.account_id ? `${t("account")}:${status.account_id}` : undefined,
      status?.token_env
        ? `${t("env")}:${status.token_env}`
        : provider.env?.[0]
          ? `${t("env")}:${provider.env[0]}`
          : undefined,
    ]
      .filter(Boolean)
      .join("  ");
    const width = menuLabelWidth(cols);
    lines.push(...menuEntryLines(provider.id, statusText, width, cols, index === 0));
    if (methods.length) {
      for (const [methodIndex, method] of methods.slice(0, 4).entries()) {
        const availability = method.available === false ? " unavailable" : "";
        lines.push(
          ...menuEntryLines(
            `${methodIndex}`,
            `${method.label || method.login} ${method.type}${method.kind ? `/${method.kind}` : ""}${availability}`,
            width,
            cols,
            false,
          ),
        );
      }
    }
  }
  lines.push(sectionBlankLine(cols));
  return lines.slice(0, maxLines);
}

export type SettingEntry = {
  detail: SettingDetail;
  label: string;
  value: unknown;
};

export function settingsCommandEntries(state: AppState): Array<[string, unknown]> {
  return settingsEntries(state).map((entry) => [settingCommandLabel(entry.detail), entry.value]);
}

export function settingsEntries(state: AppState): SettingEntry[] {
  const config = state.sessionConfig;
  if (!config) return [];
  return [
    { detail: "model", label: t("settingModel"), value: config.model ?? config.active_model },
    { detail: "provider", label: t("settingProvider"), value: configuredProviderSummary(state) },
    { detail: "agent", label: t("settingAgent"), value: config.active_agent },
    { detail: "persona", label: t("settingPersona"), value: activePersonaID(state) },
    { detail: "variant", label: t("settingReasoning"), value: config.model_variant },
    { detail: "priority", label: t("settingPriority"), value: config.model_acceleration_enabled },
    {
      detail: "commands",
      label: t("settingCommandExpansion"),
      value: config.show_command_instructions !== false,
    },
    {
      detail: "stallGuard",
      label: t("settingStallGuard"),
      value: config.command_run_stall_guard_profile,
    },
  ];
}

function settingsLines(state: AppState, cols: number, maxLines: number): string[] {
  const config = state.sessionConfig;
  const lines = sectionLines(settingTitle(state), cols);
  if (!config) {
    lines.push(sectionBodyLine(t("noSessionConfig"), cols));
    lines.push(sectionBlankLine(cols));
    return lines;
  }
  lines.push(sectionBodyLine(secondaryText(settingHint(state)), cols));
  if (state.settingInput)
    lines.push(sectionBodyLine(secondaryText(state.settingInput.prompt), cols));
  if (state.settingDetail) {
    lines.push(...settingDetailLines(state, cols, maxLines - lines.length - 1));
    lines.push(sectionBlankLine(cols));
    return lines.slice(0, maxLines);
  }
  const entries = settingEntries(settingsEntries(state).map((entry) => [entry.label, entry.value]));
  const settingWidth = menuLabelWidth(cols);
  for (const [index, [label, value]] of entries.entries()) {
    const rendered = menuEntryLines(
      label,
      value,
      settingWidth,
      cols,
      index === state.selectedSettingsIndex,
    );
    if (lines.length + rendered.length >= maxLines - 2) break;
    lines.push(...rendered);
  }
  lines.push(sectionBlankLine(cols));
  return lines.slice(0, maxLines);
}

function settingTitle(state: AppState): string {
  const detail = state.settingDetail;
  if (!detail) return t("sessionSettings");
  if (detail === "providerAuth" && state.selectedProviderID)
    return `${t("sessionSettings")} / ${t("settingProvider")} / ${state.selectedProviderID}`;
  const entry = settingStaticEntries().find((item) => item.detail === detail);
  return `${t("sessionSettings")} / ${entry?.label ?? t("settings")}`;
}

function settingHint(state: AppState): string {
  const detail = state.settingDetail;
  if (!detail) return t("settingRootHint");
  if (detail === "model") return t("settingModelHint");
  if (detail === "provider") return t("settingProviderHint");
  if (detail === "providerAuth") return providerAuthHint(state);
  if (detail === "variant") return t("settingReasoningHint");
  if (detail === "priority") return t("settingPriorityHint");
  if (detail === "agent") return t("settingAgentHint");
  if (detail === "persona") return t("settingPersonaHint");
  if (detail === "commands") return t("settingCommandExpansionHint");
  if (detail === "stallGuard") return t("settingStallGuardHint");
  return t("settingDetailHint");
}

function settingStaticEntries(): Array<{ detail: SettingDetail; label: string }> {
  return [
    { detail: "model", label: t("settingModel") },
    { detail: "provider", label: t("settingProvider") },
    { detail: "providerAuth", label: t("settingProvider") },
    { detail: "agent", label: t("settingAgent") },
    { detail: "persona", label: t("settingPersona") },
    { detail: "variant", label: t("settingReasoning") },
    { detail: "priority", label: t("settingPriority") },
    { detail: "commands", label: t("settingCommandExpansion") },
    { detail: "stallGuard", label: t("settingStallGuard") },
  ];
}

function settingCommandLabel(detail: SettingDetail): string {
  const labels: Record<SettingDetail, string> = {
    model: "/model <provider/model>",
    provider: "/provider <id>",
    providerAuth: "/provider <id>",
    agent: "/agent <name>",
    persona: "/persona <id>",
    variant: "/variant <name>",
    priority: "/priority <on/off>",
    commands: "/commands <on/off>",
    stallGuard: "/stall-guard <profile>",
  };
  return labels[detail];
}

export function settingOptions(state: AppState): Array<[string, string, unknown]> {
  const active = state.sessionConfig;
  if (!state.settingDetail || !active) return [];
  if (state.settingDetail === "model") {
    const rows: Array<[string, string, string]> = [];
    for (const provider of state.providers?.all ?? []) {
      for (const [modelID, model] of Object.entries(provider.models ?? {})) {
        const id = `${provider.id}/${modelID}`;
        rows.push([id, [model.name, model.status].filter(Boolean).join("  "), id]);
      }
    }
    return rows;
  }
  if (state.settingDetail === "provider") {
    return settingProviders(state).map((provider) => [
      providerLabelWithStatus(state, provider.id),
      providerDescription(state, provider.id),
      provider.id,
    ]);
  }
  if (state.settingDetail === "providerAuth") {
    const providerID = state.selectedProviderID;
    if (!providerID) return [];
    const methods = state.authMethods?.[providerID] ?? [];
    const rows: Array<[string, string, unknown]> = methods.map((method, index) => {
      const isOAuth = /oauth/i.test([method.type, method.kind, method.login].join(" "));
      return [
        isOAuth
          ? `${t("oauthLogin")}: ${method.label || method.login}`
          : method.label || method.login,
        [
          method.kind,
          method.available === false ? method.unavailable_reason || t("notConnected") : undefined,
          method.token_env ? `${t("env")}:${method.token_env}` : undefined,
        ]
          .filter(Boolean)
          .join("  "),
        { action: isOAuth ? "oauth" : "api-key", providerID, method: index },
      ];
    });
    if (!rows.some(([, , value]) => authAction(value)?.action === "api-key")) {
      rows.push([
        t("apiKey"),
        providerApiKeyDescription(state, providerID),
        { action: "api-key", providerID },
      ]);
    }
    rows.push([
      t("logout"),
      providerStatusText(state, providerID),
      { action: "logout", providerID },
    ]);
    return rows;
  }
  if (state.settingDetail === "agent") {
    return state.agents.map((agent) => [
      storedAgentID(agent) ?? t("unknown"),
      [agent.summary?.description, agent.summary?.source].filter(Boolean).join("  "),
      storedAgentID(agent) ?? "",
    ]);
  }
  if (state.settingDetail === "persona") {
    return state.personas.map((persona) => [
      personaID(persona) ?? t("unknown"),
      [persona.summary?.description, persona.summary?.source].filter(Boolean).join("  "),
      personaID(persona) ?? "",
    ]);
  }
  if (state.settingDetail === "variant")
    return ["low", "medium", "high", "xhigh"].map((value) => [value, "", value]);
  if (state.settingDetail === "priority")
    return [
      [t("on"), t("priority"), true],
      [t("off"), t("priority"), false],
    ];
  if (state.settingDetail === "commands")
    return [
      [t("on"), t("settingCommandExpansion"), true],
      [t("off"), t("settingCommandExpansion"), false],
    ];
  if (state.settingDetail === "stallGuard")
    return ["default", "relaxed", "strict", "off"].map((value) => [value, "", value]);
  return [];
}

function authAction(value: unknown): { action?: string } | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as { action?: string })
    : undefined;
}

function settingDetailLines(state: AppState, cols: number, maxLines: number): string[] {
  const options = settingOptions(state);
  if (!options.length) return [sectionBodyLine(t("noOptions"), cols)];
  const width = menuLabelWidth(cols);
  const lines: string[] = [];
  const active = activeSettingValue(state);
  for (const [index, [label, description, value]] of options.entries()) {
    const decoratedLabel = value === active ? `${label} ${activeMarker()}` : label;
    const rendered = menuEntryLines(
      decoratedLabel,
      description,
      width,
      cols,
      index === state.selectedSettingOptionIndex,
    );
    if (lines.length + rendered.length >= maxLines) break;
    lines.push(...rendered);
  }
  return lines;
}

function activeSettingValue(state: AppState): unknown {
  const config = state.sessionConfig;
  if (state.settingDetail === "model") return config?.model ?? config?.active_model;
  if (state.settingDetail === "provider") return config?.active_provider;
  if (state.settingDetail === "providerAuth") return undefined;
  if (state.settingDetail === "agent") return state.session?.agent ?? config?.active_agent;
  if (state.settingDetail === "persona") return activePersonaID(state);
  if (state.settingDetail === "variant") return config?.model_variant;
  if (state.settingDetail === "priority") return Boolean(config?.model_acceleration_enabled);
  if (state.settingDetail === "commands") return config?.show_command_instructions !== false;
  if (state.settingDetail === "stallGuard") return config?.command_run_stall_guard_profile;
  return undefined;
}

function activeMarker(): string {
  return activeCapabilities.unicode ? "✓" : "*";
}

function configuredProviderSummary(state: AppState): string {
  const providers = settingProviders(state);
  return t("providerConfiguredRatio", {
    configured: configuredProviderCount(state, providers),
    total: providers.length,
  });
}

function configuredProviderCount(state: AppState, providers = settingProviders(state)): number {
  const ids = new Set<string>();
  for (const id of state.providers?.connected ?? []) ids.add(id);
  for (const [id, status] of Object.entries(state.authStatuses)) {
    if (status.configured || status.authenticated) ids.add(id);
  }
  return providers.filter((provider) => ids.has(provider.id)).length;
}

function providerLabelWithStatus(state: AppState, providerID: string): string {
  return `${providerID} (${providerStatusText(state, providerID)})`;
}

function providerDescription(state: AppState, providerID: string): string {
  const provider = state.providers?.all.find((item) => item.id === providerID);
  const methods = state.authMethods?.[providerID] ?? [];
  return [
    provider?.name,
    provider?.source,
    methods.length ? `${t("auth")}:${methods.map((method) => method.type).join("/")}` : undefined,
    provider?.env?.[0] ? `${t("env")}:${provider.env[0]}` : undefined,
  ]
    .filter(Boolean)
    .join("  ");
}

function providerStatusText(state: AppState, providerID: string): string {
  const status = state.authStatuses[providerID];
  if (status?.authenticated) return t("authenticated");
  if (status?.configured) return t("configured");
  if (state.providers?.connected.includes(providerID)) return t("connected");
  if (status?.auth_state) return status.auth_state;
  if (status?.runtime_state) return status.runtime_state;
  return t("notConnected");
}

function providerApiKeyDescription(state: AppState, providerID: string): string {
  const method = (state.authMethods?.[providerID] ?? []).find((item) =>
    /key|token|api/i.test([item.type, item.kind, item.login, item.label].filter(Boolean).join(" ")),
  );
  return [
    method?.label,
    method?.api_key_url ? t("openUrl", { url: method.api_key_url }) : undefined,
    method?.docs_url ? t("docs") : undefined,
  ]
    .filter(Boolean)
    .join("  ");
}

function providerAuthHint(state: AppState): string {
  const providerID = state.selectedProviderID;
  if (!providerID) return t("settingProviderAuthHint");
  const docs = providerDocsUrl(state, providerID);
  return docs
    ? `${t("settingProviderAuthHint")} ${t("providerDocsHint", { url: docs })}`
    : t("settingProviderAuthHint");
}

function providerDocsUrl(state: AppState, providerID: string): string | undefined {
  const provider = state.providers?.all.find((item) => item.id === providerID);
  const direct = stringField(provider?.options, "api_docs") ?? provider?.api ?? undefined;
  if (direct) return direct;
  for (const method of state.authMethods?.[providerID] ?? []) {
    if (method.api_key_url) return method.api_key_url;
    if (method.docs_url) return method.docs_url;
  }
  for (const model of Object.values(provider?.models ?? {})) {
    const docs = stringField(model.options, "api_docs");
    if (docs) return docs;
  }
  return undefined;
}

function settingProviders(state: AppState): NonNullable<AppState["providers"]>["all"] {
  return (state.providers?.all ?? []).filter(isLlmProvider);
}

function isLlmProvider(provider: NonNullable<AppState["providers"]>["all"][number]): boolean {
  const domains = stringArrayField(provider.options, "domains");
  if (domains.length) return domains.some((domain) => domain.toLowerCase() === "llm");
  const capabilities = stringArrayField(provider.options, "capabilities");
  if (capabilities.some((capability) => capability.toLowerCase().startsWith("llm."))) return true;
  return Object.keys(provider.models ?? {}).length > 0;
}

function personaLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = sectionLines(t("personas"), cols);
  lines.push(sectionBodyLine(secondaryText(t("selectPersonas")), cols));
  if (!state.personas.length) {
    lines.push(sectionBodyLine(t("noPersonas"), cols), sectionBlankLine(cols));
    return lines;
  }
  const active = activePersonaID(state);
  const entries = state.personas.map((persona) => {
    const id = personaID(persona) ?? t("unknown");
    const marker = id === active ? t("active") : (persona.summary?.source ?? "");
    const description =
      persona.summary?.description ?? stringField(persona.config, "description") ?? "";
    const style =
      typeof persona.communication_style === "string" ? persona.communication_style.trim() : "";
    return [
      id,
      [marker, description, style ? style.replace(/\s+/g, " ") : undefined]
        .filter(Boolean)
        .join("  "),
    ] as [string, string];
  });
  const width = menuLabelWidth(cols);
  for (const [index, [label, description]] of entries.entries()) {
    const rendered = menuEntryLines(
      label,
      description,
      width,
      cols,
      index === state.selectedPersonaIndex,
    );
    if (lines.length + rendered.length >= maxLines - 2) break;
    lines.push(...rendered);
  }
  lines.push(sectionBlankLine(cols));
  return lines.slice(0, maxLines);
}

function modelLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = sectionLines(t("models"), cols);
  lines.push(sectionBodyLine(secondaryText(t("selectModels")), cols));
  const providers = state.providers?.all ?? [];
  let row = 0;
  const entries: Array<[string, string, boolean]> = [];
  for (const provider of providers) {
    const defaults = state.providers?.default[provider.id];
    const connected = state.providers?.connected.includes(provider.id) ? t("connected") : "";
    for (const model of Object.keys(provider.models ?? {}).slice(0, 12)) {
      entries.push([
        `${provider.id}/${model}`,
        [provider.name, connected, model === defaults ? `(${t("defaultModel")})` : undefined]
          .filter(Boolean)
          .join("  "),
        row === state.selectedModelIndex,
      ]);
      row += 1;
    }
  }
  if (!entries.length) {
    lines.push(sectionBodyLine(t("noProviders"), cols), sectionBlankLine(cols));
    return lines.slice(0, maxLines);
  }
  const width = menuLabelWidth(cols);
  for (const [label, description, selected] of entries) {
    const rendered = menuEntryLines(label, description, width, cols, selected);
    if (lines.length + rendered.length >= maxLines - 2) break;
    lines.push(...rendered);
  }
  lines.push(sectionBlankLine(cols));
  return lines.slice(0, maxLines);
}

function helpLines(cols: number, maxLines: number): string[] {
  const entries = commandHelpEntries();
  const commandWidth = menuLabelWidth(cols);
  const lines = sectionLines(t("help"), cols);
  lines.push(
    ...sectionEntriesLines(entries, commandWidth, cols, Math.max(0, maxLines - lines.length - 1)),
  );
  lines.push(sectionBlankLine(cols));
  return activeCapabilities.level === "rich"
    ? lines.filter(Boolean).slice(0, maxLines)
    : lines.slice(0, maxLines);
}

function sectionLines(title: string, cols: number): string[] {
  const titleLine = sectionTitleLine(title, cols);
  if (activeCapabilities.level === "plain") return [stripAnsi(titleLine), ""];
  return [sectionBodyLine(titleLine, cols), sectionBlankLine(cols)];
}

function sectionTitleLine(title: string, _cols: number): string {
  if (activeCapabilities.level === "plain") return `--- ${title} ---------`;
  const left = "───";
  const right = "─────────";
  return `${opencodeTextWeak}${left} ${reset}${opencodeText}${title}${reset}${opencodeTextWeak} ${right}${reset}`;
}

function sectionBodyLine(content: string, cols: number): string {
  if (activeCapabilities.level === "rich") return richContentLine(content, cols, "assistant");
  return simpleBodyLine(content, "assistant", false, cols);
}

function sectionBlankLine(cols: number): string {
  return activeCapabilities.level === "rich"
    ? richBlankRailLine("assistant", cols)
    : simpleSpacerLine("assistant", cols);
}

function settingEntries(rows: Array<[string, unknown]>): Array<[string, string]> {
  return rows
    .filter(([, value]) => value !== undefined && value !== null && value !== "")
    .map(([label, value]) => [label, formatSettingValue(value)]);
}

function sectionEntryLines(
  label: string,
  description: string,
  labelWidth: number,
  cols: number,
): string[] {
  if (activeCapabilities.level === "rich") {
    return richHelpEntryLines(label, description, labelWidth, cols);
  }
  return simpleHelpEntryLines(label, description, labelWidth, cols);
}

function menuEntryLines(
  label: string,
  description: string,
  labelWidth: number,
  cols: number,
  selected: boolean,
): string[] {
  const marker = selected ? "> " : "  ";
  return sectionEntryLines(`${marker}${label}`, description, labelWidth, cols);
}

function sessionEntryLine(
  label: string,
  description: string,
  labelWidth: number,
  cols: number,
  selected: boolean,
): string {
  const marker = selected ? "> " : "  ";
  const leftWidth = Math.max(8, labelWidth);
  const rightWidth = Math.max(4, visibleTextWidth(label));
  const left = truncateAnsi(`${marker}${label}`, leftWidth);
  const right = truncateAnsi(description, rightWidth);
  const content =
    activeCapabilities.level === "plain"
      ? `${pad(left, leftWidth)}  ${right}`
      : `${opencodePrimary}${pad(left, leftWidth)}${reset}   ${secondaryText(right)}`;
  return sectionBodyLine(truncateAnsi(content, Math.max(20, cols - 2)), cols);
}

function sectionEntriesLines(
  entries: Array<[string, string]>,
  labelWidth: number,
  cols: number,
  maxLines: number,
): string[] {
  const lines: string[] = [];
  for (const [label, description] of entries) {
    const rendered = sectionEntryLines(label, description, labelWidth, cols);
    if (lines.length + rendered.length > maxLines) break;
    lines.push(...rendered);
  }
  return lines;
}

function helpEntryWidth(entries: Array<[string, string]>): number {
  return Math.min(
    activeCapabilities.level === "rich" ? 32 : 24,
    Math.max(8, ...entries.map(([command]) => visibleTextWidth(command))),
  );
}

function menuLabelWidth(cols: number): number {
  const desired = helpEntryWidth(commandHelpEntries()) * 2;
  const gutter = activeCapabilities.level === "rich" ? 12 : 8;
  const maxByTerminal = Math.max(8, cols - gutter - 20);
  return Math.max(8, Math.min(desired, maxByTerminal));
}

function formatSettingValue(value: unknown): string {
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === "number") return Number.isFinite(value) ? String(value) : "";
  return String(value);
}

function simpleHelpEntryLines(
  command: string,
  description: string,
  commandWidth: number,
  cols: number,
): string[] {
  const descriptionWidth = Math.max(12, cols - commandWidth - 8);
  const descriptionLines = wrapWords(description, descriptionWidth);
  if (activeCapabilities.level === "plain") {
    return descriptionLines.map((line, index) =>
      index === 0
        ? `  ${pad(command, commandWidth)}  ${line}`
        : `  ${" ".repeat(commandWidth)}  ${line}`,
    );
  }
  return descriptionLines.map((line, index) =>
    simpleBodyLine(
      index === 0
        ? `${opencodePrimary}${pad(command, commandWidth)}${reset}   ${secondaryText(line)}`
        : `${" ".repeat(commandWidth)}   ${secondaryText(line)}`,
      "assistant",
      false,
      cols,
    ),
  );
}

function richHelpEntryLines(
  command: string,
  description: string,
  commandWidth: number,
  cols: number,
): string[] {
  const descriptionWidth = Math.max(12, cols - commandWidth - 12);
  const descriptionLines = wrapWords(description, descriptionWidth);
  return descriptionLines.map((line, index) =>
    richContentLine(
      index === 0
        ? `${opencodePrimary}${pad(command, commandWidth)}${reset}   ${opencodeTextWeak}${line}${reset}`
        : `${" ".repeat(commandWidth)}   ${opencodeTextWeak}${line}${reset}`,
      cols,
      "assistant",
    ),
  );
}

function wrapWords(text: string, width: number): string[] {
  const safeWidth = Math.max(8, width);
  const lines: string[] = [];
  for (const inputLine of text.split(/\r?\n/)) {
    let line = "";
    for (const word of inputLine.split(/\s+/).filter(Boolean)) {
      if (!line) {
        if (visibleTextWidth(word) <= safeWidth) line = word;
        else lines.push(...wrap(word, safeWidth));
        continue;
      }
      if (visibleTextWidth(`${line} ${word}`) <= safeWidth) {
        line = `${line} ${word}`;
        continue;
      }
      lines.push(line);
      if (visibleTextWidth(word) <= safeWidth) line = word;
      else {
        const wrapped = wrap(word, safeWidth);
        lines.push(...wrapped.slice(0, -1));
        line = wrapped.at(-1) ?? "";
      }
    }
    lines.push(line);
  }
  return lines.length ? lines : [""];
}

function commandHelpEntries(): Array<[string, string]> {
  return [
    ["/chat", t("helpChat")],
    ["/commands", t("helpCommands")],
    ["/new", t("helpNew")],
    ["/resume <id>", t("helpResume")],
    ["/auth", t("providerLogin")],
    [t("loginProvider"), t("helpLogin")],
    [t("logoutProvider"), t("helpLogout")],
    ["/settings", t("helpSettings")],
    ["/model <provider/model>", t("helpModel")],
    ["/agent <name>", t("agent")],
    ["/personas", t("personas")],
    ["/persona <name>", t("applyPersona")],
    ["/sessions", t("helpSessions")],
    ["/models", t("helpModels")],
    ["/abort", t("helpAbort")],
    ["/stop", t("helpStop")],
    [t("configGet"), t("helpConfigGet")],
    [t("configSet"), t("helpConfigSet")],
    ["/quit", t("helpQuit")],
  ];
}

function noticeLines(value: string, cols: number): string[] {
  const text = compactNotice(value);
  return wrap(`${dim}${text}${reset}`, cols).slice(0, 3);
}

function compactNotice(value: string): string {
  const trimmed = value.trim();
  if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) return trimmed;
  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const object = parsed as Record<string, unknown>;
      const pieces: string[] = [];
      for (const key of [
        "active_agent",
        "model",
        "active_model",
        "model_variant",
        "service_tier",
        "session_type",
      ]) {
        const item = object[key];
        if (item !== undefined && item !== null && item !== "")
          pieces.push(`${key}:${String(item)}`);
      }
      if ("model_acceleration_enabled" in object)
        pieces.push(`${t("priority")}:${String(object.model_acceleration_enabled)}`);
      if (pieces.length) return `${t("settings")} ${pieces.join("  ")}`;
      if ("mano" in object || "router" in object || "lsp" in object) {
        return `${t("status")} mano:${serviceState(object.mano)}  ${t("router")}:${serviceState(object.router)}  lsp:${Array.isArray(object.lsp) ? object.lsp.length : 0}`;
      }
    }
  } catch {
    // Fall back to a short single-line JSON preview.
  }
  return trimmed.replace(/\s+/g, " ");
}

function serviceState(value: unknown): string {
  if (value && typeof value === "object") {
    const object = value as Record<string, unknown>;
    return String(object.status ?? object.error ?? t("unknown"));
  }
  return t("unknown");
}

function composerLines(value: string, cols: number, frame = 0, hint = t("composerHint")): string[] {
  const text = value || "";
  if (activeCapabilities.level !== "plain") {
    return richComposerLines(text, cols, frame, hint);
  }
  const lines = wrap(text, Math.max(20, cols - 3));
  const cursor = COMPOSER_CURSOR_MARKER;
  const inputLines =
    lines.length === 0
      ? [`${opencodePrimary}>${reset} ${cursor}`]
      : lines.map(
          (line, index) =>
            `${index === 0 ? `${opencodePrimary}>${reset}` : " "} ${line}${index === lines.length - 1 ? cursor : ""}`,
        );
  return [...inputLines, `  ${stripAnsi(hint)}`];
}

function richComposerLines(value: string, cols: number, _frame: number, hint: string): string[] {
  const textWidth = Math.max(20, cols - 6);
  const lines = wrap(value || "", textWidth).slice(0, 4);
  return composerPanelLines(lines, cols, hint);
}

function composerPanelLines(lines: string[], cols: number, hint = t("composerHint")): string[] {
  const visible = lines.length && lines.some((line) => line) ? lines : [""];
  const body = visible.map((line, index) => {
    const prompt = index === 0 ? `${opencodePrimary}>${reset}` : " ";
    const isLast = index === visible.length - 1;
    const content = line
      ? `${line}${isLast ? COMPOSER_CURSOR_MARKER : ""}`
      : `${COMPOSER_CURSOR_MARKER}${opencodeTextWeak}${truncateAnsi(hint, Math.max(1, cols - 7))}${reset}`;
    return splitBorderPanelLine(`${prompt} ${content}`, cols, "user");
  });
  return [splitBorderPanelBlank("user", cols), ...body, splitBorderPanelBlank("user", cols)];
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

function personaID(persona: AppState["personas"][number]): string | undefined {
  const configName = persona.config?.persona_name;
  return persona.summary?.id ?? (typeof configName === "string" ? configName : undefined);
}

function activePersonaID(state: AppState): string | undefined {
  const agentID = state.session?.agent ?? state.sessionConfig?.active_agent;
  const agent = state.agents.find((item) => storedAgentID(item) === agentID);
  const first = Array.isArray(agent?.config?.agent_persona)
    ? agent?.config?.agent_persona[0]
    : undefined;
  if (first && typeof first === "object" && !Array.isArray(first)) {
    const name = (first as Record<string, unknown>).persona_name;
    if (typeof name === "string" && name.trim()) return name.trim();
  }
  const runtimePersonas = (
    agent as unknown as { options?: { personas?: AppState["personas"] } } | undefined
  )?.options?.personas;
  return runtimePersonas?.[0] ? personaID(runtimePersonas[0]) : undefined;
}

function storedAgentID(agent: AppState["agents"][number]): string | undefined {
  return agent.summary?.id ?? (agent as unknown as { name?: string }).name;
}

function stringField(value: Record<string, unknown> | undefined, key: string): string | undefined {
  const item = value?.[key];
  return typeof item === "string" ? item : undefined;
}

function stringArrayField(value: Record<string, unknown> | undefined, key: string): string[] {
  const item = value?.[key];
  return Array.isArray(item)
    ? item.filter((entry): entry is string => typeof entry === "string")
    : [];
}
