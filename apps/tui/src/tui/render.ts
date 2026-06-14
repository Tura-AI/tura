import { displayMessages, type AppState } from "./reducer.js";
import type { Message } from "../types/session.js";
import { messageText, sessionTitle } from "../types/session.js";
import { t } from "../i18n.js";
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
import { busyAnimationFrame } from "./render/busy-animation.js";
import {
  finalizeFrame,
  plainFrame,
  terminalRenderCols,
  type RenderedFrame,
} from "./render/frame.js";
import {
  transcriptLiveRenderLines,
  transcriptRenderLines,
  transcriptThinkingLines,
  type TranscriptRenderLine,
} from "./render/transcript.js";
import { secondaryText } from "./styles/text.js";
import { isBusyState } from "./busy-state.js";
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
import { settingsLines } from "./render/settings.js";

export { settingOptions, settingsCommandEntries, settingsEntries } from "./render/settings.js";

export type RenderedChatFrame = RenderedFrame & {
  renderCols: number;
  cacheFrame: string;
  liveFrame: string;
  liveRows: TranscriptRenderLine[];
  liveStreamKey: string;
  chromeFrame: string;
  chromeCursor?: RenderedFrame["cursor"];
  liveRegionFrame: string;
  liveRegionCursor?: RenderedFrame["cursor"];
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
  const renderCols = terminalRenderCols(cols);
  const panelMaxLines = Number.MAX_SAFE_INTEGER;
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
): RenderedChatFrame {
  setActiveCapabilities(capabilities);
  const cols = process.stdout.columns || 100;
  const renderCols = terminalRenderCols(cols);
  const cacheRows = transcriptRenderLines(state, renderCols);
  const liveRows = liveLinesWithCacheBoundary(
    cacheRows,
    transcriptLiveRenderLines(state, renderCols),
  );
  const cacheLines = cacheRows.map((line) => line.text);
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
  const cache = finalizeFrame(cacheLines, 0, renderCols);
  const live = finalizeFrame(liveLines, 0, renderCols);
  const chrome = finalizeFrame(chromeLines, 0, renderCols);
  const liveRegion = finalizeFrame([...liveLines, ...chromeLines], 0, renderCols);
  const rendered = finalizeFrame([...cacheLines, ...liveLines, ...chromeLines], 0, renderCols);
  return {
    ...rendered,
    renderCols,
    cacheFrame: cache.frame,
    liveFrame: live.frame,
    liveRows,
    liveStreamKey: activeLiveStreamKey(state),
    chromeFrame: chrome.frame,
    chromeCursor: chrome.cursor,
    liveRegionFrame: liveRegion.frame,
    liveRegionCursor: liveRegion.cursor,
  };
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
  const renderCols = terminalRenderCols(cols);
  const panelMaxLines = Number.MAX_SAFE_INTEGER;
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
  const title = state.session ? sessionTitle(state.session) : "tura";
  const color =
    activeCapabilities.level === "rich"
      ? richHighlight
      : activeCapabilities.level === "ansi"
        ? richHighlight
        : bold;
  return truncateAnsi(`${color}${title}${reset}`, cols);
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
  const model = [
    bottomMetaModel(state),
    state.session?.model_variant ?? state.sessionConfig?.model_variant,
    (state.session?.model_acceleration_enabled ?? state.sessionConfig?.model_acceleration_enabled)
      ? t("priority")
      : undefined,
  ]
    .filter(Boolean)
    .join(" ");
  return [statusIndicator(state), model || "-", tokenSummary(state)];
}

function bottomMetaModel(state: AppState): string | undefined {
  const sessionModel = stringOrUndefined(state.session?.model);
  if (sessionModel?.includes("/")) return sessionModel;

  const configuredModel = stringOrUndefined(state.sessionConfig?.model);
  if (configuredModel?.includes("/")) return configuredModel;

  const provider = stringOrUndefined(state.sessionConfig?.active_provider);
  const activeModel = stringOrUndefined(state.sessionConfig?.active_model);
  if (provider && activeModel) return `${provider}/${activeModel}`;
  if (provider && sessionModel) return `${provider}/${sessionModel}`;
  if (provider && configuredModel) return `${provider}/${configuredModel}`;
  return sessionModel ?? configuredModel ?? activeModel;
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
  return activeCapabilities.level === "plain" ? bold : richHighlight;
}

function sessionStateMarker(state: AppState, session: AppState["sessions"][number]): string {
  if (session.status === "busy") {
    return busyAnimationFrame(state.thinkingFrame, activeCapabilities.unicode);
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
  lines.push(
    sessionEntryLine(
      t("newSession"),
      t("createSession"),
      width,
      cols,
      state.selectedSessionIndex === 0,
    ),
  );
  if (!state.sessions.length) lines.push(sectionBodyLine(t("noSessions"), cols));
  for (const [index, [label, description]] of entries.entries()) {
    if (lines.length + 1 >= maxLines - 2) break;
    lines.push(
      sessionEntryLine(label, description, width, cols, index + 1 === state.selectedSessionIndex),
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

function stringOrUndefined(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
