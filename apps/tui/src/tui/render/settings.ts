import { t } from "../../i18n.js";
import { agentDescription } from "../../agent-display.js";
import type { SettingDetail, AppState } from "../reducer.js";
import { runtimeModelFromConfig } from "../model-config.js";
import { activeCapabilities, truncate, wrap } from "../render-terminal.js";
import { secondaryText } from "../styles/text.js";
import { SETTING_DETAILS } from "../settings-catalog.js";
import {
  menuEntryLines,
  menuLabelWidth,
  menuLabelWidthFor,
  sectionBlankLine,
  sectionBodyLine,
  sectionLines,
  settingValueEntries,
} from "./section-ui.js";

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
    {
      detail: "model",
      label: t("settingModel"),
      value: configuredModel(state) ?? t("unknown"),
    },
    { detail: "provider", label: t("settingProvider"), value: configuredProviderSummary(state) },
    { detail: "agent", label: t("settingAgent"), value: config.active_agent ?? t("unknown") },
    { detail: "persona", label: t("settingPersona"), value: config.active_persona ?? "tura" },
    { detail: "language", label: t("settingLanguage"), value: config.language ?? "en" },
    { detail: "variant", label: t("settingReasoning"), value: config.model_variant ?? "high" },
    {
      detail: "priority",
      label: t("settingPriority"),
      value: config.model_acceleration_enabled ?? false,
    },
  ];
}

export function settingsLines(state: AppState, cols: number, maxLines: number): string[] {
  const config = state.sessionConfig;
  const lines = sectionLines(settingTitle(state), cols);
  if (!config) {
    lines.push(sectionBodyLine(t("noSessionConfig"), cols));
    lines.push(sectionBlankLine(cols));
    return lines;
  }
  lines.push(sectionBodyLine(secondaryText(settingHint(state)), cols));
  if (state.settingInput) lines.push(...settingInputLines(state, cols));
  if (state.settingDetail) {
    lines.push(...settingDetailLines(state, cols, maxLines - lines.length - 1));
    lines.push(sectionBlankLine(cols));
    return lines.slice(0, maxLines);
  }
  const entries = settingValueEntries(
    settingsEntries(state).map((entry) => [entry.label, entry.value]),
  );
  const settingWidth = menuLabelWidth(cols);
  const visibleEntries = Math.max(1, maxLines - lines.length - 2);
  const start = pageStartForIndex(state.selectedSettingsIndex, visibleEntries, entries.length);
  for (const [offset, [label, value]] of entries.slice(start).entries()) {
    if (offset >= visibleEntries) break;
    const index = start + offset;
    const rendered = menuEntryLines(
      label,
      value,
      settingWidth,
      cols,
      index === state.selectedSettingsIndex,
    );
    if (lines.length + rendered.length > maxLines - 2) break;
    lines.push(...rendered);
  }
  lines.push(sectionBlankLine(cols));
  return lines.slice(0, maxLines);
}

function settingInputLines(state: AppState, cols: number): string[] {
  const input = state.settingInput;
  if (!input) return [];
  const lines = [sectionBodyLine(secondaryText(input.prompt), cols)];
  if (!input.oauthUrl) return lines;
  lines.push(sectionBodyLine(secondaryText(t("openUrl", { url: "" }).trim()), cols));
  lines.push(
    sectionBodyLine(secondaryText(terminalLink(input.oauthUrl, oauthLinkLabel(cols))), cols),
  );
  for (const line of wrap(input.oauthUrl, Math.max(24, cols - 4))) {
    lines.push(sectionBodyLine(secondaryText(line), cols));
  }
  return lines;
}

function oauthLinkLabel(cols: number): string {
  return truncate("Open complete OAuth URL", Math.max(24, cols - 4));
}

function terminalLink(url: string, label: string): string {
  const cleanUrl = url.replace(/[\x00-\x1f\x7f]/gu, "");
  if (!cleanUrl || activeCapabilities.level === "plain" || !activeCapabilities.osc8) return label;
  return `\x1b]8;;${cleanUrl}\x1b\\${label}\x1b]8;;\x1b\\`;
}

export function settingsPageInfo(
  state: AppState,
  maxLines: number,
): { label: string; current: number; total: number } {
  if (!state.sessionConfig) return { label: t("sessionSettingsPage"), current: 1, total: 1 };
  const headerLines = sectionLines(settingTitle(state), 80).length;
  const promptLines = 1 + settingInputLines(state, 80).length;
  if (state.settingDetail) {
    const pageSize = Math.max(1, maxLines - headerLines - promptLines - 1);
    return {
      label: settingPageLabel(state),
      ...pageInfoForIndex(state.selectedSettingOptionIndex, pageSize, settingOptions(state).length),
    };
  }
  const entries = settingValueEntries(
    settingsEntries(state).map((entry) => [entry.label, entry.value]),
  );
  const pageSize = Math.max(1, maxLines - headerLines - promptLines - 2);
  return {
    label: t("sessionSettingsPage"),
    ...pageInfoForIndex(state.selectedSettingsIndex, pageSize, entries.length),
  };
}

function settingPageLabel(state: AppState): string {
  const detail = state.settingDetail;
  if (!detail) return t("sessionSettingsPage");
  if (detail === "providerAuth" && state.selectedProviderID) {
    return t("settingDetailPage", { name: state.selectedProviderID });
  }
  const entry = settingStaticEntries().find((item) => item.detail === detail);
  return t("settingDetailPage", { name: entry?.label ?? t("settings") });
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
  if (detail === "language") return t("settingLanguageHint");
  if (detail === "session") return t("settingSessionHint");
  if (detail === "validator") return t("settingValidatorHint");
  if (detail === "stallGuard") return t("settingStallGuardHint");
  return t("settingDetailHint");
}

function settingStaticEntries(): Array<{ detail: SettingDetail; label: string }> {
  return [
    ...SETTING_DETAILS.map((detail) => ({ detail, label: settingLabel(detail) })),
    { detail: "providerAuth" as const, label: t("settingProvider") },
  ];
}

function settingLabel(detail: Exclude<SettingDetail, "providerAuth">): string {
  const labels: Record<Exclude<SettingDetail, "providerAuth">, string> = {
    model: t("settingModel"),
    provider: t("settingProvider"),
    agent: t("settingAgent"),
    persona: t("settingPersona"),
    language: t("settingLanguage"),
    session: t("settingSession"),
    variant: t("settingReasoning"),
    priority: t("settingPriority"),
    validator: t("settingValidator"),
    stallGuard: t("settingStallGuard"),
  };
  return labels[detail];
}

function settingCommandLabel(detail: SettingDetail): string {
  const labels: Record<SettingDetail, string> = {
    model: "/model <provider/model>",
    provider: "/provider <id>",
    providerAuth: "/provider <id>",
    agent: "/agent <name>",
    persona: "/persona <id>",
    language: "/language <zh-CN|en>",
    session: "/session <type>",
    variant: "/variant <name>",
    priority: "/priority <on/off>",
    validator: "/validator <on/off>",
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
      [agentDescription(agent), agent.summary?.source].filter(Boolean).join("  "),
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
  if (state.settingDetail === "language")
    return [
      ["en", t("languageEn"), "en"],
      ["zh-CN", t("languageZhCN"), "zh-CN"],
    ];
  if (state.settingDetail === "session")
    return ["coding", "business", "research", "planning"].map((value) => [value, "", value]);
  if (state.settingDetail === "variant")
    return ["low", "medium", "high", "xhigh"].map((value) => [value, "", value]);
  if (state.settingDetail === "priority")
    return [
      [t("on"), t("priority"), true],
      [t("off"), t("priority"), false],
    ];
  if (state.settingDetail === "validator")
    return [
      [t("on"), t("settingValidator"), true],
      [t("off"), t("settingValidator"), false],
    ];
  if (state.settingDetail === "stallGuard")
    return ["balanced_20s", "fast_10s", "patient_30s", "long_io_60s", "off"].map((value) => [
      value,
      "",
      value,
    ]);
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
  const width = menuLabelWidthFor(
    options.map(([label]) => label),
    cols,
  );
  const lines: string[] = [];
  const active = activeSettingValue(state);
  const visibleEntries = Math.max(1, maxLines);
  const start = pageStartForIndex(state.selectedSettingOptionIndex, visibleEntries, options.length);
  for (const [offset, [label, description, value]] of options.slice(start).entries()) {
    if (offset >= visibleEntries) break;
    const index = start + offset;
    const decoratedLabel = value === active ? `${label} ${activeMarker()}` : label;
    const rendered = menuEntryLines(
      decoratedLabel,
      description,
      width,
      cols,
      index === state.selectedSettingOptionIndex,
    );
    if (lines.length + rendered.length > maxLines) break;
    lines.push(...rendered);
  }
  return lines;
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

function activeSettingValue(state: AppState): unknown {
  const config = state.sessionConfig;
  if (state.settingDetail === "model") return configuredModel(state);
  if (state.settingDetail === "provider") return config?.active_provider;
  if (state.settingDetail === "providerAuth") return undefined;
  if (state.settingDetail === "agent") return config?.active_agent;
  if (state.settingDetail === "persona") return config?.active_persona ?? "tura";
  if (state.settingDetail === "language") return config?.language ?? "en";
  if (state.settingDetail === "session") return config?.session_type ?? "coding";
  if (state.settingDetail === "variant") return config?.model_variant ?? "high";
  if (state.settingDetail === "priority") return config?.model_acceleration_enabled ?? false;
  if (state.settingDetail === "validator") return Boolean(config?.validator_enabled);
  if (state.settingDetail === "stallGuard")
    return config?.command_run_stall_guard_profile ?? "balanced_20s";
  return undefined;
}

function configuredModel(state: AppState): string | undefined {
  return runtimeModelFromConfig(state.sessionConfig, state.modelConfig);
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

function personaID(persona: AppState["personas"][number] | undefined): string | undefined {
  const configName = persona?.config?.persona_name;
  return persona?.summary?.id ?? (typeof configName === "string" ? configName : undefined);
}

function storedAgentID(agent: AppState["agents"][number]): string | undefined {
  return agent.summary?.id ?? (agent as unknown as { name?: string }).name;
}

function stringField(value: Record<string, unknown> | undefined, key: string): string | undefined {
  const item = value?.[key];
  return typeof item === "string" && item.trim() ? item : undefined;
}

function stringArrayField(value: Record<string, unknown> | undefined, key: string): string[] {
  const item = value?.[key];
  if (!Array.isArray(item)) return [];
  return item.filter((entry): entry is string => typeof entry === "string");
}
