import type { AppState } from "./reducer.js";
import type { Message, MessagePart } from "../types/session.js";
import { messageText, sessionTitle } from "../types/session.js";
import { t } from "../i18n.js";
import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";
import {
  activeCapabilities,
  bold,
  clear,
  dim,
  fit,
  gray,
  opencodeBorder,
  opencodeElementBg,
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
  underline,
  visibleTextWidth,
  wrap,
  wrapAnsi,
} from "./render-terminal.js";
import {
  compactInlinePayloads,
  compactPayloadField,
  displayMessageText,
  extractCommandsFromText,
  extractCommandsFromUnknown,
  firstCommandLine,
  renderRichText,
  toolSummary,
} from "./render-rich-text.js";
import { SplitBorder, SplitBorderFallback } from "./ui/border.js";

type CommandInfo = {
  command: string;
  tool?: string;
  status?: string;
};

type TranscriptTurn = {
  user?: Message;
  replies: Message[];
};

export function render(
  state: AppState,
  capabilities: TerminalCapabilities = detectTerminalCapabilities(),
): string {
  setActiveCapabilities(capabilities);
  if (capabilities.level === "plain") return renderPlain(state);
  const rows = process.stdout.rows || 30;
  const cols = process.stdout.columns || 100;
  const renderCols = terminalRenderCols(cols);
  const lines: string[] = [];
  lines.push(`${clear}${topBar(state, renderCols)}`);
  lines.push(...layoutSeparator(renderCols));

  if (state.help) {
    lines.push(...helpLines(renderCols, rows - 7));
  } else if (state.sessionsOpen) {
    lines.push(...sessionLines(state, renderCols, rows - 7));
  } else if (state.authOpen) {
    lines.push(...authLines(state, renderCols, rows - 7));
  } else if (state.settingsOpen) {
    lines.push(...settingsLines(state, renderCols, rows - 7));
  } else if (state.personasOpen) {
    lines.push(...personaLines(state, renderCols, rows - 7));
  } else if (state.modelsOpen) {
    lines.push(...modelLines(state, renderCols, rows - 7));
  } else {
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
  lines.push(...composerSeparator(renderCols));
  lines.push(...composerLines(state.composer, renderCols));
  lines.push(bottomMetaLine(state, renderCols));
  return fit(lines, Math.max(1, rows - 1), renderCols).join("\n");
}

function transcriptMaxLines(rows: number, cols: number, composer: string): number {
  const composerRows =
    activeCapabilities.level === "plain"
      ? Math.max(1, wrap(composer || "", Math.max(20, cols - 3)).length) + 1
      : Math.min(4, Math.max(1, wrap(composer || "", Math.max(20, cols - 6)).length)) + 2;
  return Math.max(4, rows - composerRows - 4);
}

function layoutSeparator(cols: number): string[] {
  if (activeCapabilities.level !== "plain") return [""];
  return [rule(cols)];
}

function composerSeparator(cols: number): string[] {
  if (activeCapabilities.level === "plain") return layoutSeparator(cols);
  return [""];
}

function renderPlain(state: AppState): string {
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
  else lines.push(...transcriptLines(state, renderCols, 20));
  if (state.notice) lines.push(...noticeLines(state.notice, renderCols));
  lines.push("");
  lines.push(...composerLines(state.composer, renderCols));
  lines.push(stripAnsi(bottomMetaLine(state, renderCols)));
  return stripAnsi(fit(lines, Math.max(1, rows - 1), renderCols).join("\n"));
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
    if (state.status === "error") return "×";
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
  const showCommands = Boolean(state.sessionConfig?.show_command_instructions);
  const visibleMessages =
    activeCapabilities.level === "plain" ? state.messages.slice(-16) : state.messages.slice(-10);
  if (activeCapabilities.level !== "plain") {
    for (const turn of groupTranscriptTurns(visibleMessages)) {
      const rendered = renderRichTurn(turn, state, cols, showCommands);
      if (!rendered.length) continue;
      addTranscriptGap(lines, turn.user?.role ?? turn.replies[0]?.role ?? "assistant", cols);
      lines.push(...rendered);
    }
  } else {
    for (const turn of groupTranscriptTurns(visibleMessages)) {
      const rendered = renderSimpleTurn(turn, state, cols, showCommands);
      if (!rendered.length) continue;
      addTranscriptGap(lines, turn.user?.role ?? turn.replies[0]?.role ?? "assistant", cols);
      lines.push(...rendered);
    }
  }
  if (isThinking(state)) {
    addTranscriptGap(lines, "assistant", cols);
    lines.push(thinkingLine(state, cols));
  }
  return visibleTranscriptLines(lines, maxLines);
}

function visibleTranscriptLines(lines: string[], maxLines: number): string[] {
  if (lines.length <= maxLines) return lines;
  if (activeCapabilities.level === "plain")
    return lines.slice(Math.max(0, lines.length - maxLines));
  const withoutLeadingUser = dropLeadingUserBlock(lines);
  if (withoutLeadingUser.length < lines.length) {
    if (withoutLeadingUser.length <= maxLines) return withoutLeadingUser;
    lines = withoutLeadingUser;
  }
  const tailCount = Math.min(8, Math.max(4, Math.floor(maxLines / 3)));
  const headCount = Math.max(1, maxLines - tailCount);
  return [...lines.slice(0, headCount), ...lines.slice(lines.length - tailCount)];
}

function dropLeadingUserBlock(lines: string[]): string[] {
  const firstContent = lines.findIndex((line) => line !== "");
  if (firstContent < 0 || !lines[firstContent]?.includes(`${opencodeText}▏`)) return lines;
  const nextGap = lines.findIndex((line, index) => index > firstContent && line === "");
  return nextGap >= 0 ? lines.slice(nextGap + 1) : lines;
}

function groupTranscriptTurns(messages: Message[]): TranscriptTurn[] {
  const turns: TranscriptTurn[] = [];
  let current: TranscriptTurn | undefined;
  for (const message of messages) {
    if (message.role === "user") {
      current = { user: message, replies: [] };
      turns.push(current);
      continue;
    }
    if (!current) {
      current = { replies: [] };
      turns.push(current);
    }
    current.replies.push(message);
  }
  return turns;
}

function renderSimpleTurn(
  turn: TranscriptTurn,
  state: AppState,
  cols: number,
  showCommands: boolean,
): string[] {
  const lines: string[] = [];
  const prefixWidth = activeCapabilities.unicode ? 4 : 3;
  const contentWidth = Math.max(20, cols - prefixWidth - 2);
  if (turn.user) {
    const text = displayMessageText("user", messageText(turn.user));
    const rendered = secondaryText(stripAnsi(renderRichText(text)));
    for (const line of wrapAnsi(rendered, contentWidth))
      lines.push(simpleBodyLine(line, "user", true, cols));
  }

  const replyLines: string[] = [];
  for (const message of turn.replies) {
    const text = displayMessageText(message.role, messageText(message));
    const partLines = message.parts.flatMap(partTranscriptLines);
    const commands = commandsForMessage(message);
    const richText = text ? renderRichText(text) : "";
    const displayText =
      message.role === "assistant" ? richText : secondaryText(stripAnsi(richText));
    for (const line of displayText ? wrapAnsi(displayText, contentWidth) : []) {
      replyLines.push(simpleBodyLine(line, message.role, false, cols));
    }
    for (const partLine of partLines) {
      for (const line of wrapAnsi(secondaryText(partLine), contentWidth)) {
        replyLines.push(simpleBodyLine(line, message.role, false, cols));
      }
    }
    if (message.role === "assistant" && commands.length) {
      if (replyLines.length && replyLines.at(-1) !== "") replyLines.push("");
      replyLines.push(
        commandDetailLine(commandSummaryLine(commands, state.commandDetailsOpen, cols), cols),
      );
      if (state.commandDetailsOpen || showCommands) {
        for (const line of commandDetailLines(commands, cols)) {
          replyLines.push(commandDetailLine(line, cols));
        }
      }
      replyLines.push("");
    }
  }
  if (replyLines.length) {
    if (lines.length) lines.push(simpleSpacerLine("assistant", cols));
    lines.push(...replyLines);
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

function renderRichTurn(
  turn: TranscriptTurn,
  state: AppState,
  cols: number,
  showCommands: boolean,
): string[] {
  const lines: string[] = [];
  const contentWidth = Math.max(20, cols - 8);
  if (turn.user) {
    const userText = displayMessageText("user", messageText(turn.user));
    const body = secondaryText(stripAnsi(renderRichText(userText)));
    const wrapped = body ? wrapAnsi(body, contentWidth) : [];
    if (wrapped.length) {
      lines.push(richBlankRailLine("user", cols));
      for (const [index, line] of wrapped.entries()) {
        const marker = index === 0 ? `${opencodePrimary}◆${reset}` : " ";
        lines.push(richContentLine(`${marker} ${line}`, cols, "user"));
      }
      lines.push(richBlankRailLine("user", cols));
    }
  }

  const replyBlocks: Array<{ role: string; lines: string[] }> = [];
  for (const message of turn.replies) {
    const blockLines: string[] = [];
    const commandLines: string[] = [];
    const text = displayMessageText(message.role, messageText(message));
    const partLines = message.parts.flatMap(partTranscriptLines);
    const commands = commandsForMessage(message);
    const primary = message.role === "assistant";
    const richText = text ? renderRichText(text) : "";
    const displayText = primary ? richText : secondaryText(stripAnsi(richText));
    const wrapped = displayText ? wrapAnsi(displayText, contentWidth) : [];
    if (wrapped.length) {
      for (const line of wrapped) blockLines.push(richContentLine(line, cols, message.role));
    } else if (message.role !== "assistant") {
      blockLines.push(
        richContentLine(`${opencodeTextWeak}${message.role}${reset}`, cols, message.role),
      );
    }
    for (const partLine of partLines) {
      for (const line of wrapAnsi(secondaryText(partLine), Math.max(20, cols - 8))) {
        blockLines.push(
          richContentLine(`${opencodeTextWeak}◇${reset} ${line}`, cols, message.role),
        );
      }
    }
    if (message.role === "assistant" && commands.length) {
      commandLines.push(
        commandDetailLine(commandSummaryLine(commands, state.commandDetailsOpen, cols - 6), cols),
      );
      if (state.commandDetailsOpen || (showCommands && activeCapabilities.level !== "rich")) {
        for (const line of commandDetailLines(commands, cols - 6)) {
          commandLines.push(commandDetailLine(line, cols));
        }
      }
    }
    if (blockLines.length || commandLines.length) {
      const commandSection = commandLines.length ? ["", ...commandLines, ""] : [];
      replyBlocks.push({
        role: message.role,
        lines: blockLines.length
          ? [
              richBlankRailLine(message.role, cols),
              ...blockLines,
              richBlankRailLine(message.role, cols),
              ...commandSection,
            ]
          : commandSection,
      });
    }
  }
  for (const block of replyBlocks) {
    if (lines.length && lines.at(-1) !== "") lines.push("");
    lines.push(...block.lines);
  }
  return lines;
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

function commandsForMessage(message: Message): CommandInfo[] {
  const commands = [
    ...extractCommandsFromText(messageText(message)).map((command) => ({
      command,
      tool: t("assistant"),
    })),
    ...message.parts.flatMap(commandsForPart),
  ];
  const seen = new Set<string>();
  const unique: CommandInfo[] = [];
  for (const item of commands) {
    const command = firstCommandLine(item.command);
    if (!command || seen.has(command)) continue;
    seen.add(command);
    unique.push({ ...item, command });
  }
  return unique.slice(0, 24);
}

function commandsForPart(part: MessagePart): CommandInfo[] {
  const state =
    part.state && typeof part.state === "object" ? (part.state as Record<string, unknown>) : {};
  const status = typeof state.status === "string" ? state.status : undefined;
  const tool = part.tool ?? t("tool");
  return [
    ...extractCommandsFromUnknown(state.input).map((command) => ({ command, tool, status })),
    ...extractCommandsFromUnknown(state.output).map((command) => ({ command, tool, status })),
    ...extractCommandsFromUnknown(part.metadata).map((command) => ({ command, tool, status })),
  ];
}

function commandSummaryLine(commands: CommandInfo[], expanded: boolean, cols: number): string {
  const count = `${t("commands")}: ${commands.length}`;
  const running = commands.some((command) =>
    /run|progress|pending|busy|question/i.test(command.status ?? ""),
  );
  const icon = activeCapabilities.unicode
    ? running
      ? "◇"
      : expanded
        ? "◇"
        : "◇"
    : running
      ? "#"
      : expanded
        ? "+"
        : "*";
  const label = `${icon} ${count}`;
  const visible = commandToggleLabel(label);
  return secondaryText(truncateAnsi(visible, Math.max(12, cols - 2)));
}

function commandToggleLabel(label: string): string {
  if (activeCapabilities.level === "plain") return label;
  const visible = `${underline}${label}${reset}`;
  if (activeCapabilities.level !== "rich" || !activeCapabilities.osc8) return visible;
  return `\x1b]8;;tura://commands/toggle\x1b\\${visible}\x1b]8;;\x1b\\`;
}

function commandDetailLines(commands: CommandInfo[], cols: number): string[] {
  const lines: string[] = [];
  for (const [index, command] of commands.entries()) {
    const isLast = index === commands.length - 1;
    const branch = activeCapabilities.unicode ? (isLast ? "└─" : "├─") : "|-";
    const stem = activeCapabilities.unicode ? (isLast ? "   " : "│  ") : "|  ";
    const symbol = statusSymbol(command.status);
    const meta = [command.tool ?? t("tool"), command.status].filter(Boolean).join(" ");
    lines.push(
      secondaryText(`${branch} ${stripAnsi(symbol)} #${index + 1}${meta ? ` ${meta}` : ""}`),
    );
    const prefix = "$ ";
    for (const line of wrapAnsi(
      secondaryText(`${prefix}${command.command}`),
      Math.max(20, cols - 2),
    )) {
      lines.push(`${secondaryText(stem)}${line}`);
    }
  }
  return lines;
}

function statusSymbol(status: string | undefined): string {
  const normalized = (status ?? "").toLowerCase();
  if (/fail|error|reject|denied/.test(normalized))
    return `${opencodePrimary}${activeCapabilities.unicode ? "✕" : "x"}${reset}`;
  if (/run|progress|pending|busy|question/.test(normalized))
    return `${opencodePrimary}${activeCapabilities.unicode ? "◆" : "#"}${reset}`;
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
  const commands = state.messages.slice(-24).flatMap(commandsForMessage);
  const suffix = commands.length ? `  ${t("commands")}: ${commands.length}` : "";
  const text = `${frame} thinking${suffix ? `  ${truncate(suffix.trim(), Math.max(12, cols - 14))}` : ""}`;
  if (activeCapabilities.level !== "plain")
    return splitBorderPanelLine(secondaryText(text), cols, "assistant", opencodePanelBg);
  return secondaryText(text);
}

function sessionLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = sectionLines(t("sessions"), cols);
  lines.push(
    sectionBodyLine(
      secondaryText(
        `${t("selectSessions")}  ${t("enterResume")}  ${t("createSession")}  /resume <id> ${t("directSwitch")}`,
      ),
      cols,
    ),
  );
  if (!state.sessions.length) {
    lines.push(sectionBodyLine(t("noSessions"), cols), sectionBlankLine(cols));
    return lines;
  }
  const entries = state.sessions.map((session) => {
    const current =
      session.id === state.session?.id ? t("active") : (session.status ?? t("sessionIdle"));
    const meta = `${current} ${formatTime(session.updated_at ?? session.created_at)}`;
    return [session.id, `${meta}  ${sessionTitle(session)}`] as [string, string];
  });
  const width = menuLabelWidth(cols);
  for (const [index, [label, description]] of entries.entries()) {
    const rendered = menuEntryLines(
      label,
      description,
      width,
      cols,
      index === state.selectedSessionIndex,
    );
    if (lines.length + rendered.length >= maxLines - 2) break;
    lines.push(...rendered);
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

function settingsLines(state: AppState, cols: number, maxLines: number): string[] {
  const config = state.sessionConfig;
  const lines = sectionLines(t("sessionSettings"), cols);
  if (!config) {
    lines.push(sectionBodyLine(t("noSessionConfig"), cols));
    lines.push(sectionBlankLine(cols));
    return lines;
  }
  const rows: Array<[string, unknown]> = [
    ["/model <provider/model>", config.model ?? config.active_model],
    ["/provider <id>", config.active_provider],
    ["/agent <name>", config.active_agent],
    ["/persona <id>", activePersonaID(state)],
    ["/variant <name>", config.model_variant],
    ["/priority <on/off>", config.model_acceleration_enabled],
    ["/session <type>", config.session_type],
    ["/context <limit>", config.context_message_limit],
    ["/validator <on/off>", config.validator_enabled],
    ["/commands <on/off>", config.show_command_instructions ?? false],
    ["/stall-guard <profile>", config.command_run_stall_guard_profile],
  ];
  const entries = settingEntries(rows);
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

function composerLines(value: string, cols: number): string[] {
  const text = value || "";
  if (activeCapabilities.level !== "plain") {
    return richComposerLines(text, cols);
  }
  const lines = wrap(text, Math.max(20, cols - 3));
  const inputLines =
    lines.length === 0
      ? [`${opencodePrimary}>${reset}`]
      : lines.map((line, index) => `${index === 0 ? `${opencodePrimary}>${reset}` : " "} ${line}`);
  return [...inputLines, `  ${stripAnsi(t("composerHint"))}`];
}

function richComposerLines(value: string, cols: number): string[] {
  const textWidth = Math.max(20, cols - 6);
  const lines = wrap(value || "", textWidth).slice(0, 4);
  return composerPanelLines(lines, cols);
}

function composerPanelLines(lines: string[], cols: number): string[] {
  const visible = lines.length && lines.some((line) => line) ? lines : [""];
  const body = visible.map((line, index) => {
    const prompt = index === 0 ? `${opencodePrimary}>${reset}` : " ";
    const content = line || `${opencodeTextWeak}${t("composerHint")}${reset}`;
    return splitBorderPanelLine(`${prompt} ${content}`, cols, "user", opencodeElementBg);
  });
  return [
    splitBorderPanelBlank("user", cols, opencodeElementBg),
    ...body,
    splitBorderPanelBlank("user", cols, opencodeElementBg),
  ];
}

function partTranscriptLines(part: MessagePart): string[] {
  if (part.type !== "tool") return [];
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

function formatTime(value: string | number | undefined): string {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "-";
  return `${date.getMonth() + 1}/${date.getDate()} ${date.getHours()}:${String(date.getMinutes()).padStart(2, "0")}`;
}
