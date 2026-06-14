import { displayMessages, type AppState, type SettingDetail } from "./reducer.js";
import type { Message } from "../types/session.js";
import { messageText, sessionTitle } from "../types/session.js";
import { t } from "../i18n.js";
import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";
import {
  activeCapabilities,
  bold,
  dim,
  opencodeBorder,
  opencodePrimary,
  opencodeText,
  opencodeTextWeak,
  pad,
  reset,
  rule,
  setActiveCapabilities,
  stripAnsi,
  truncate,
  truncateAnsi,
  visibleTextWidth,
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
import { transcriptLines, transcriptLiveLines } from "./render/transcript.js";
import { panelBlankLine, panelLine } from "./styles/panel.js";
import { secondaryText } from "./styles/text.js";

export type RenderedChatFrame = RenderedFrame & {
  cacheFrame: string;
  liveFrame: string;
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
  const cacheLines = transcriptLines(state, renderCols);
  const liveLines = transcriptLiveLines(state, renderCols);
  const chromeLines = [
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
    cacheFrame: cache.frame,
    liveFrame: live.frame,
    chromeFrame: chrome.frame,
    chromeCursor: chrome.cursor,
    liveRegionFrame: liveRegion.frame,
    liveRegionCursor: liveRegion.cursor,
  };
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
  const transcript = transcriptLines(state, cols);
  const liveLines = transcriptLiveLines(state, cols);
  return [...transcript, ...liveLines];
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
      ? opencodePrimary
      : activeCapabilities.level === "ansi"
        ? opencodePrimary
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
  return truncateAnsi(`${opencodeTextWeak}${pieces.join(bottomMetaDivider())}${reset}`, cols);
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

function isBusy(state: AppState): boolean {
  return state.status === "busy" || state.session?.status === "busy";
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
  return `${opencodeBorder} │ ${reset}${opencodeTextWeak}`;
}

function hintText(value: string): string {
  const color = activeCapabilities.level === "rich" ? opencodeTextWeak : dim;
  return `${color}${value}${reset}`;
}

function statusIndicator(state: AppState): string {
  if (activeCapabilities.unicode) {
    if (state.status === "error") return "x";
    if (isBusy(state)) return busyAnimationFrame(state.thinkingFrame, true);
    return "○";
  }
  if (state.status === "error") return "x";
  if (isBusy(state)) return busyAnimationFrame(state.thinkingFrame, false);
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
    sectionBodyLine(secondaryText(`${t("selectSessions")}  ${t("enterOpenSession")}`), cols),
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

function simpleBodyLine(line: string, role: string, _user: boolean, cols = 80): string {
  if (activeCapabilities.level === "plain") return `  ${stripAnsi(line)}`;
  return panelLine(line, cols, role);
}

function richContentLine(content: string, cols: number, role = "assistant"): string {
  return panelLine(content, cols, role);
}

function sectionBlankLine(cols: number): string {
  return activeCapabilities.level === "plain" ? "" : panelBlankLine("assistant", cols);
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
  const gapWidth = activeCapabilities.level === "plain" ? 2 : 3;
  const contentWidth = Math.max(20, cols - 4);
  const leftWidth = Math.min(Math.max(8, labelWidth), Math.max(8, contentWidth - gapWidth - 4));
  const rightWidth = Math.max(0, contentWidth - leftWidth - gapWidth);
  const left = truncateAnsi(`${marker}${label}`, leftWidth);
  const right = rightWidth > 0 ? truncateAnsi(description, rightWidth) : "";
  const gap = " ".repeat(gapWidth);
  const content =
    activeCapabilities.level === "plain"
      ? `${pad(left, leftWidth)}${gap}${right}`
      : `${opencodePrimary}${pad(left, leftWidth)}${reset}${gap}${secondaryText(right)}`;
  return sectionBodyLine(truncateAnsi(content, contentWidth), cols);
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

function sessionLabelWidth(labels: string[], cols: number): number {
  const markerWidth = 2;
  const maxLabelWidth = Math.max(6, ...labels.map((label) => visibleTextWidth(label)));
  const maxByTerminal = Math.max(8, Math.floor(cols * 0.45));
  return Math.max(8, Math.min(maxLabelWidth + markerWidth, maxByTerminal));
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

function stringArrayField(value: Record<string, unknown> | undefined, key: string): string[] {
  const item = value?.[key];
  return Array.isArray(item)
    ? item.filter((entry): entry is string => typeof entry === "string")
    : [];
}
