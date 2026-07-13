import { displayMessages, type AppState } from "./reducer.js";
import type { Message } from "../types/session.js";
import { messageText, sessionHasQuestionStatus, sessionTitle } from "../types/session.js";
import { t } from "../i18n.js";
import { personaCommunicationStyle, personaDescription } from "../persona-display.js";
import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";
import {
  activeCapabilities,
  bold,
  borderColor,
  dim,
  reset,
  richHighlight,
  rule,
  setActiveCapabilities,
  stripAnsi,
  textAuxiliary,
  truncate,
  truncateAnsi,
  wrap,
} from "./render-terminal.js";
import { composerLines } from "./render/composer.js";
import {
  busyAnimationFrame,
  questionAnimationFrame,
  thinkingAnimationFrame,
} from "./render/busy-animation.js";
import {
  finalizeFrame,
  plainFrame,
  terminalRenderCols,
  type RenderedFrame,
} from "./render/frame.js";
import {
  transcriptMessageGroups,
  transcriptLiveRenderLines,
  transcriptRenderLines,
  transcriptRenderLinesForMessages,
  transcriptThinkingLines,
  thinkingWaveText,
  type TranscriptRenderLine,
} from "./render/transcript.js";
import { secondaryText } from "./styles/text.js";
import { isBusyState, sessionHasRunningCommand } from "./busy-state.js";
import { runtimeModelFromConfig } from "./model-config.js";
import {
  commandHelpEntries,
  menuEntryLines,
  menuLabelWidth,
  menuLabelWidthFor,
  sectionBlankLine,
  sectionBodyLine,
  sectionEntriesLines,
  sectionLines,
  sessionEntryLine,
  sessionLabelWidth,
} from "./render/section-ui.js";
import { settingsLines, settingsPageInfo } from "./render/settings.js";

export { settingOptions, settingsCommandEntries, settingsEntries } from "./render/settings.js";

export type RenderedChatFrame = RenderedFrame & {
  renderCols: number;
  cacheFrame: string;
  cacheLines: string[];
  liveFrame: string;
  liveRows: TranscriptRenderLine[];
  tailCacheMessageCount: number;
  activeLiveMessageCount: number;
  liveStreamKey: string;
  chromeFrame: string;
  chromeCursor?: RenderedFrame["cursor"];
  liveRegionFrame: string;
  liveRegionCursor?: RenderedFrame["cursor"];
  cache: RenderedChatCache;
};

export type RenderedChatCache = {
  sessionID: string;
  renderCols: number;
  capabilitiesKey: string;
  settingsKey: string;
  cacheMessageCount: number;
  cacheRows: TranscriptRenderLine[];
  cacheLines: string[];
  cacheFrame: string;
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
  const cols = process.stdout.columns || 100;
  const rows = process.stdout.rows || 40;
  const renderCols = terminalRenderCols(cols);
  const panelMaxLines = Math.max(1, rows - 4);
  const lines: string[] = [];
  const chatSurface = shouldShowComposer(state);
  if (!chatSurface) lines.push(sessionTitleLine(state, renderCols));

  if (state.help) {
    lines.push(...layoutSeparator(renderCols));
    lines.push(...helpLines(renderCols, panelMaxLines));
  } else if (state.sessionsOpen) {
    lines.push(...layoutSeparator(renderCols));
    lines.push(...sessionLines(state, renderCols, panelMaxLines));
  } else if (state.authOpen) {
    lines.push(...layoutSeparator(renderCols));
    lines.push(...authLines(state, renderCols, panelMaxLines));
  } else if (state.settingsOpen) {
    lines.push(...layoutSeparator(renderCols));
    lines.push(...settingsLines(state, renderCols, panelMaxLines));
  } else if (state.personasOpen) {
    lines.push(...layoutSeparator(renderCols));
    lines.push(...personaLines(state, renderCols, panelMaxLines));
  } else if (state.modelsOpen) {
    lines.push(...layoutSeparator(renderCols));
    lines.push(...modelLines(state, renderCols, panelMaxLines));
  } else {
    lines.push(...chatTranscriptLines(state, renderCols));
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
  if (chatSurface) lines.push(...transcriptThinkingLines(state, renderCols));
  if (chatSurface) lines.push(...bottomTitleLines(state, renderCols));
  lines.push(bottomMetaLine(state, renderCols));
  if (shouldShowComposer(state)) {
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
  return finalizeFrame(lines, 0, renderCols);
}

export function renderChatFrameParts(
  state: AppState,
  capabilities: TerminalCapabilities = detectTerminalCapabilities(),
  options: { cache?: RenderedChatCache } = {},
): RenderedChatFrame {
  setActiveCapabilities(capabilities);
  const cols = process.stdout.columns || 100;
  const renderCols = terminalRenderCols(cols);
  const groups = transcriptMessageGroups(state);
  const reusableCache = reusableRenderedChatCache(
    options.cache,
    state.session?.id ?? "",
    renderCols,
    capabilities,
    state,
    groups.cache.length,
  );
  const renderedCache =
    reusableCache ?? renderChatCache(state, capabilities, renderCols, groups.cache);
  const tailCacheMessages = groups.cache.slice(renderedCache.cacheMessageCount);
  const tailCacheRows = transcriptRenderLinesForMessages(state, renderCols, tailCacheMessages, {
    commandMode: "cache",
  });
  const activeLiveRows = transcriptRenderLinesForMessages(state, renderCols, groups.live, {
    commandMode: "live",
  });
  const liveRowsForFrame =
    tailCacheRows.length && activeLiveRows.length
      ? [...tailCacheRows, { text: "", kind: "gap" as const }, ...activeLiveRows]
      : tailCacheRows.length
        ? tailCacheRows
        : activeLiveRows;
  const liveRows = liveLinesWithCacheBoundary(renderedCache.cacheRows, liveRowsForFrame);
  const cacheLines = renderedCache.cacheLines;
  const liveLines = liveRows.map((line) => line.text);
  const chromeLines = [
    ...transcriptThinkingLines(state, renderCols),
    ...bottomTitleLines(state, renderCols),
    bottomMetaLine(state, renderCols),
    ...composerSeparator(renderCols),
    ...composerLines(
      state.composer,
      renderCols,
      state.thinkingFrame,
      state.settingInput ? t("settingInputComposerHint") : undefined,
    ),
  ];
  const live = finalizeFrame(liveLines, 0, renderCols);
  const chrome = finalizeFrame(chromeLines, 0, renderCols);
  const liveRegion = finalizeFrame([...liveLines, ...chromeLines], 0, renderCols);
  const rendered = reusableCache
    ? liveRegion
    : finalizeFrame([...cacheLines, ...liveLines, ...chromeLines], 0, renderCols);
  return {
    ...rendered,
    renderCols,
    cacheFrame: renderedCache.cacheFrame,
    cacheLines,
    liveFrame: live.frame,
    liveRows,
    tailCacheMessageCount: tailCacheMessages.length,
    activeLiveMessageCount: groups.live.length,
    liveStreamKey: activeLiveStreamKey(state),
    chromeFrame: chrome.frame,
    chromeCursor: chrome.cursor,
    liveRegionFrame: liveRegion.frame,
    liveRegionCursor: liveRegion.cursor,
    cache: renderedCache,
  };
}

function renderChatCache(
  state: AppState,
  capabilities: TerminalCapabilities,
  renderCols: number,
  cacheMessages: Message[],
): RenderedChatCache {
  const cacheRows = transcriptRenderLinesForMessages(state, renderCols, cacheMessages);
  const cacheLines = cacheRows.map((line) => line.text);
  const cache = finalizeFrame(cacheLines, 0, renderCols);
  return {
    sessionID: state.session?.id ?? "",
    renderCols,
    capabilitiesKey: renderedChatCapabilitiesKey(capabilities),
    settingsKey: renderedChatSettingsKey(state),
    cacheMessageCount: cacheMessages.length,
    cacheRows,
    cacheLines,
    cacheFrame: cache.frame,
  };
}

function reusableRenderedChatCache(
  cache: RenderedChatCache | undefined,
  sessionID: string,
  renderCols: number,
  capabilities: TerminalCapabilities,
  state: AppState,
  cacheMessageCount: number,
): RenderedChatCache | undefined {
  if (!cache) return undefined;
  if (cache.sessionID !== sessionID) return undefined;
  if (cache.renderCols !== renderCols) return undefined;
  if (cache.capabilitiesKey !== renderedChatCapabilitiesKey(capabilities)) return undefined;
  if (cache.settingsKey !== renderedChatSettingsKey(state)) return undefined;
  if (cache.cacheMessageCount > cacheMessageCount) return undefined;
  return cache;
}

function renderedChatCapabilitiesKey(capabilities: TerminalCapabilities): string {
  return [
    capabilities.level,
    capabilities.color,
    capabilities.unicode ? "unicode" : "ascii",
    capabilities.osc8 ? "osc8" : "no-osc8",
    capabilities.richText,
  ].join("\0");
}

function renderedChatSettingsKey(state: AppState): string {
  return state.sessionConfig?.show_command_instructions === false
    ? "commands:hidden"
    : "commands:shown";
}

function activeLiveStreamKey(state: AppState): string {
  const sessionID = state.session?.id;
  return Object.values(state.liveStreams)
    .filter((stream) => !sessionID || !stream.sessionID || stream.sessionID === sessionID)
    .map((stream) => `${stream.sessionID ?? ""}\u0000${stream.messageID}\u0000${stream.partID}`)
    .sort()
    .join("\n");
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
  const rows = process.stdout.rows || 40;
  const renderCols = terminalRenderCols(cols);
  const panelMaxLines = Math.max(1, rows - 4);
  const lines: string[] = [];
  const chatSurface = shouldShowComposer(state);
  if (!chatSurface) {
    lines.push(stripAnsi(sessionTitleLine(state, renderCols)));
    lines.push("");
  }
  if (state.help) lines.push(...helpLines(renderCols, panelMaxLines));
  else if (state.sessionsOpen) lines.push(...sessionLines(state, renderCols, panelMaxLines));
  else if (state.authOpen) lines.push(...authLines(state, renderCols, panelMaxLines));
  else if (state.settingsOpen) lines.push(...settingsLines(state, renderCols, panelMaxLines));
  else if (state.personasOpen) lines.push(...personaLines(state, renderCols, panelMaxLines));
  else if (state.modelsOpen) lines.push(...modelLines(state, renderCols, panelMaxLines));
  else lines.push(...chatTranscriptLines(state, renderCols));
  if (state.notice) lines.push(...noticeLines(state.notice, renderCols));
  if (chatSurface) lines.push(...transcriptThinkingLines(state, renderCols));
  if (chatSurface) lines.push(...bottomTitleLines(state, renderCols).map(stripAnsi));
  lines.push(stripAnsi(bottomMetaLine(state, renderCols)));
  if (shouldShowComposer(state)) {
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
  return plainFrame(finalizeFrame(lines, 0, renderCols));
}

function chatTranscriptLines(state: AppState, cols: number): string[] {
  if (state.sessionLoading && !state.messages.length) return [sessionLoadingLine(state, cols)];
  const transcript = transcriptRenderLines(state, cols);
  const liveLines = liveLinesWithCacheBoundary(transcript, transcriptLiveRenderLines(state, cols));
  return [...transcript, ...liveLines].map((line) => line.text);
}

function liveLinesWithCacheBoundary(
  cacheLines: TranscriptRenderLine[],
  liveLines: TranscriptRenderLine[],
): TranscriptRenderLine[] {
  if (!cacheLines.length || !liveLines.length) return liveLines;
  return [{ text: "", kind: "gap" }, ...liveLines];
}

function shouldShowComposer(state: AppState): boolean {
  if (state.settingInput) return true;
  if (state.sessionLoading && !state.messages.length) return false;
  return !(
    state.help ||
    state.sessionsOpen ||
    state.authOpen ||
    state.settingsOpen ||
    state.personasOpen ||
    state.modelsOpen
  );
}

function sessionTitleLine(state: AppState, cols: number): string {
  const title = surfaceTitle(state);
  const color =
    activeCapabilities.level === "rich"
      ? richHighlight
      : activeCapabilities.level === "ansi"
        ? richHighlight
        : bold;
  return truncateAnsi(`${color}${title}${reset}`, cols);
}

function surfaceTitle(state: AppState): string {
  if (state.sessionsOpen) return state.cwd.trim() || state.session?.directory?.trim() || "tura";
  return state.session ? sessionTitle(state.session) : "tura";
}

function bottomTitleLines(state: AppState, cols: number): string[] {
  const title = sessionTitleLine(state, cols);
  return activeCapabilities.level === "plain" ? ["", title] : ["", title];
}

function bottomMetaLine(state: AppState, cols: number): string {
  const pieces = bottomMetaPieces(state);
  if (activeCapabilities.level === "plain") {
    return truncateAnsi(`${dim}${pieces.join("  ")}${reset}`, cols);
  }
  return truncateAnsi(`${textAuxiliary}${pieces.join(bottomMetaDivider())}${reset}`, cols);
}

function bottomMetaPieces(state: AppState): string[] {
  const panelPage = panelPageInfo(state);
  if (panelPage) return [`${panelPage.label} ${panelPage.current}/${panelPage.total}`];
  const pieces = [
    bottomMetaModel(state),
    bottomMetaVariant(state),
    bottomMetaPriority(state),
    bottomMetaAgent(state),
    bottomMetaPersona(state),
    contextTokenSummary(state),
    usageSummary(state),
  ].filter((item): item is string => Boolean(item));
  return pieces.length ? pieces : [statusIndicator(state)];
}

type PanelPageInfo = { label: string; current: number; total: number };

function panelPageInfo(state: AppState): PanelPageInfo | undefined {
  const maxLines = Math.max(1, (process.stdout.rows || 40) - 4);
  if (state.settingsOpen) return settingsPageInfo(state, maxLines);
  if (state.sessionsOpen) return sessionPageInfo(state, maxLines);
  return undefined;
}

function bottomMetaModel(state: AppState): string | undefined {
  const configuredModel = runtimeModelFromConfig(state.sessionConfig, state.modelConfig);
  if (configuredModel) return configuredModel;
  const sessionModel = stringOrUndefined(state.session?.model);
  if (sessionModel?.includes("/")) return sessionModel;
  return undefined;
}

function bottomMetaVariant(state: AppState): string | undefined {
  return (
    stringOrUndefined(state.sessionConfig?.model_variant) ??
    stringOrUndefined(state.session?.model_variant)
  );
}

function bottomMetaPriority(state: AppState): string | undefined {
  const enabled =
    state.sessionConfig?.model_acceleration_enabled ?? state.session?.model_acceleration_enabled;
  return enabled ? "priority" : undefined;
}

function bottomMetaAgent(state: AppState): string | undefined {
  return (
    stringOrUndefined(state.sessionConfig?.active_agent) ?? stringOrUndefined(state.session?.agent)
  );
}

function bottomMetaPersona(state: AppState): string | undefined {
  return stringOrUndefined(state.sessionConfig?.active_persona) ?? "tura";
}

function bottomMetaDivider(): string {
  return `${borderColor} │ ${reset}${textAuxiliary}`;
}

function hintText(value: string): string {
  const color = activeCapabilities.level === "rich" ? textAuxiliary : dim;
  return `${color}${value}${reset}`;
}

function statusIndicator(state: AppState): string {
  if (activeCapabilities.unicode) {
    if (state.status === "error") return "x";
    if (isBusyState(state)) return busyAnimationFrame(state.thinkingFrame, true);
    return "○";
  }
  if (state.status === "error") return "x";
  if (isBusyState(state)) return busyAnimationFrame(state.thinkingFrame, false);
  return "-";
}

function numberField(record: Record<string, unknown>, key: string): number {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function contextTokenSummary(state: AppState): string | undefined {
  const context = state.session?.context_tokens;
  const limit = numberField(context ?? {}, "limit");
  if (limit <= 0) return undefined;
  const input = Math.max(0, numberField(context ?? {}, "input"));
  return `context ${compactTokenCount(input)}/${compactTokenCount(limit)} ${contextBar(input, limit)}`;
}

function usageSummary(state: AppState): string | undefined {
  const usage = state.session?.usage;
  if (!usage) return undefined;
  const cost = typeof usage.cost === "number" && Number.isFinite(usage.cost) ? usage.cost : 0;
  if (cost <= 0) return undefined;
  return formatUsageCost(cost, usage.currency);
}

function formatUsageCost(cost: number, currency: string | null | undefined): string {
  const amount = cost < 0.01 ? cost.toFixed(4) : cost.toFixed(2);
  return currency === "USD" || !currency ? `$${amount}` : `${amount} ${currency}`;
}

function compactTokenCount(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0";
  if (value >= 1_000_000) return `${trimDecimal(value / 1_000_000)}m`;
  if (value >= 1_000) return `${trimDecimal(value / 1_000)}k`;
  return Math.round(value).toString();
}

function trimDecimal(value: number): string {
  const rounded = Math.round(value * 10) / 10;
  return Number.isInteger(rounded) ? rounded.toFixed(0) : rounded.toFixed(1);
}

function contextBar(input: number, limit: number): string {
  const cells = 6;
  const unitsPerCell = 10;
  const totalUnits = cells * unitsPerCell;
  const filledUnits = Math.max(0, Math.min(totalUnits, (input / limit) * totalUnits));
  let bar = "";
  for (let index = 0; index < cells; index += 1) {
    const cellUnits = Math.max(0, Math.min(unitsPerCell, filledUnits - index * unitsPerCell));
    bar += contextBarCell(cellUnits, unitsPerCell);
  }
  return bar;
}

function contextBarCell(units: number, unitsPerCell: number): string {
  if (units >= unitsPerCell) return "█";
  if (units <= 0) return "░";
  if (units < unitsPerCell * 0.66) return "▒";
  return "▓";
}

function richPrimary(): string {
  return activeCapabilities.level === "plain" ? bold : richHighlight;
}

function sessionStateMarker(state: AppState, session: AppState["sessions"][number]): string {
  if (session.status === "busy") {
    return busyAnimationFrame(state.thinkingFrame, activeCapabilities.unicode);
  }
  if (sessionHasRunningCommand(state, session.id)) {
    return busyAnimationFrame(state.thinkingFrame, activeCapabilities.unicode);
  }
  if (sessionHasQuestionStatus(session)) {
    return questionAnimationFrame(state.thinkingFrame, activeCapabilities.unicode);
  }
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
  if (state.sessionLoading) lines.push(sessionLoadingLine(state, cols));
  else
    lines.push(
      sectionBodyLine(
        secondaryText(
          `${t("selectSessions")}  ${t("enterOpenSession")}  ${t("shiftEnterCopySession")}  ${t("deleteSessionHint")}`,
        ),
        cols,
      ),
    );
  const entries = state.sessions.map((session) => {
    const marker = sessionStateMarker(state, session);
    const label = [sessionTitle(session), marker].filter(Boolean).join(" ");
    const preview =
      state.sessionPreviews[session.id] ||
      (session.id === state.session?.id ? lastMessagePreview(displayMessages(state)) : "");
    return [label, preview] as [string, string];
  });
  const width = sessionLabelWidth([t("newSession"), ...entries.map(([label]) => label)], cols);
  const items: Array<[string, string]> = [[t("newSession"), t("createSession")], ...entries];
  const visibleEntries = Math.max(1, maxLines - lines.length - 1);
  const start = pageStartForIndex(state.selectedSessionIndex, visibleEntries, items.length);
  for (const [offset, [label, description]] of items.slice(start).entries()) {
    const index = start + offset;
    if (offset >= visibleEntries) break;
    lines.push(
      sessionEntryLine(label, description, width, cols, index === state.selectedSessionIndex),
    );
  }
  if (!state.sessions.length && lines.length < maxLines - 1)
    lines.push(sectionBodyLine(t("noSessions"), cols));
  lines.push(sectionBlankLine(cols));
  return lines.slice(0, maxLines);
}

function sessionLoadingLine(state: AppState, cols: number): string {
  const frame = activeCapabilities.unicode
    ? thinkingAnimationFrame(state.thinkingFrame, true)
    : (["|", "/", "-", "\\"][state.thinkingFrame % 4] ?? ".");
  const title = state.sessionLoading?.title?.trim();
  const label =
    state.sessionLoading?.kind === "deleting" ? t("sessionDeleting") : t("sessionLoading");
  const text = title ? `${frame} ${label} ${title}` : `${frame} ${label}`;
  if (activeCapabilities.level === "plain") return sectionBodyLine(secondaryText(text), cols);
  return sectionBodyLine(thinkingWaveText(text, state.thinkingFrame), cols);
}

function sessionPageInfo(state: AppState, maxLines: number): PanelPageInfo {
  const headerLines = sectionLines(t("sessions"), 80).length + 1;
  const visibleEntries = Math.max(1, maxLines - headerLines - 1);
  return {
    label: t("sessionSelectPage"),
    ...pageInfoForIndex(state.selectedSessionIndex, visibleEntries, state.sessions.length + 1),
  };
}

function pageStartForIndex(index: number, pageSize: number, total: number): number {
  if (total <= 0) return 0;
  const safePageSize = Math.max(1, pageSize);
  const safeIndex = Math.max(0, Math.min(index, total - 1));
  return Math.floor(safeIndex / safePageSize) * safePageSize;
}

function pageInfoForIndex(
  index: number,
  pageSize: number,
  total: number,
): { current: number; total: number } {
  if (total <= 0) return { current: 1, total: 1 };
  const safePageSize = Math.max(1, pageSize);
  const totalPages = Math.max(1, Math.ceil(total / safePageSize));
  const safeIndex = Math.max(0, Math.min(index, total - 1));
  return {
    current: Math.min(totalPages, Math.floor(safeIndex / safePageSize) + 1),
    total: totalPages,
  };
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
      status?.account_id ? `${t("account")}:${status.account_id}` : undefined,
      provider.source,
      status?.login ? `${t("loginState")}:${status.login}` : undefined,
      status?.auth_state ? `${t("authState")}:${status.auth_state}` : undefined,
      status?.runtime_state ? `${t("runtime")}:${status.runtime_state}` : undefined,
      status?.token_env
        ? `${t("env")}:${status.token_env}`
        : provider.env?.[0]
          ? `${t("env")}:${provider.env[0]}`
          : undefined,
    ]
      .filter(Boolean)
      .join("  ");
    const width = menuLabelWidthFor(
      providers.map((item) => item.id),
      cols,
    );
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

function personaLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = sectionLines(t("personas"), cols);
  lines.push(sectionBodyLine(secondaryText(t("selectPersonas")), cols));
  if (!state.personas.length) {
    lines.push(sectionBodyLine(t("noPersonas"), cols), sectionBlankLine(cols));
    return lines;
  }
  const active = bottomMetaPersona(state);
  const entries = state.personas.map((persona) => {
    const id = personaID(persona) ?? t("unknown");
    const marker = id === active ? t("active") : (persona.summary?.source ?? "");
    const description = personaDescription(persona);
    const style = personaCommunicationStyle(persona);
    return [
      id,
      [marker, description, style ? style.replace(/\s+/g, " ") : undefined]
        .filter(Boolean)
        .join("  "),
    ] as [string, string];
  });
  const width = menuLabelWidthFor(
    entries.map(([label]) => label),
    cols,
  );
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
  const width = menuLabelWidthFor(
    entries.map(([label]) => label),
    cols,
  );
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

function personaID(persona: AppState["personas"][number]): string | undefined {
  const configName = persona.config?.persona_name;
  return persona.summary?.id ?? (typeof configName === "string" ? configName : undefined);
}

function stringOrUndefined(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
