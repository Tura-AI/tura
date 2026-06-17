import type { Session } from "../../types/session.js";
import { sessionUpdatedAt } from "../../types/session.js";
import type { ProviderListResponse } from "../../types/provider.js";
import type { SessionConfig } from "../../types/config.js";
import type { StoredAgent } from "../../types/agent.js";
import type { StoredPersona } from "../../types/gateway.js";
import type { AppState, SettingDetail } from "../reducer.js";
import { settingOptions, settingsEntries } from "../render/settings.js";
import { SETTING_DETAILS } from "../settings-catalog.js";

export const SESSION_CREATE_ENTRY_COUNT = 1;

export function settingOptionCount(state: AppState): number {
  return settingOptions(state).length;
}

export function settingsEntryCount(state: AppState): number {
  return settingsEntries(state).length || SETTING_DETAILS.length;
}

export function selectedSettingOptionIndex(state: AppState, detail: SettingDetail): number {
  const config = state.sessionConfig;
  if (detail === "model") return state.selectedModelIndex;
  if (detail === "provider") {
    const active = config?.active_provider;
    const index = settingProviders(state.providers).findIndex((provider) => provider.id === active);
    return index >= 0 ? index : 0;
  }
  if (detail === "providerAuth") return 0;
  if (detail === "agent") {
    const active = state.session?.agent ?? config?.active_agent;
    const index = state.agents.findIndex((agent) => storedAgentID(agent) === active);
    return index >= 0 ? index : 0;
  }
  if (detail === "persona") return state.selectedPersonaIndex;
  if (detail === "language")
    return Math.max(0, ["en", "zh-CN"].indexOf(String(config?.language ?? "en")));
  if (detail === "session")
    return Math.max(
      0,
      ["coding", "business", "research", "planning"].indexOf(String(config?.session_type)),
    );
  if (detail === "variant")
    return Math.max(
      0,
      ["low", "medium", "high", "xhigh"].indexOf(String(config?.model_variant ?? "high")),
    );
  if (detail === "priority") return (config?.model_acceleration_enabled ?? true) ? 0 : 1;
  if (detail === "commands") return config?.show_command_instructions !== false ? 0 : 1;
  if (detail === "validator") return config?.validator_enabled ? 0 : 1;
  if (detail === "stallGuard")
    return Math.max(
      0,
      ["balanced_20s", "fast_10s", "patient_30s", "long_io_60s", "off"].indexOf(
        String(config?.command_run_stall_guard_profile ?? "balanced_20s"),
      ),
    );
  return 0;
}

export function upsertSession(sessions: Session[], session: Session): Session[] {
  const next = sessions.filter((item) => item.id !== session.id);
  next.push(session);
  next.sort((left, right) => sessionUpdatedAt(right) - sessionUpdatedAt(left));
  return next;
}

export function seedSeenSessionCounts(
  current: Record<string, number>,
  sessions: Session[],
  activeSessionID: string | undefined,
): Record<string, number> {
  const next = { ...current };
  for (const session of sessions) {
    if (next[session.id] !== undefined && session.id !== activeSessionID) continue;
    next[session.id] = session.message_count ?? next[session.id] ?? 0;
  }
  return next;
}

export function boundedSessionIndex(index: number, sessions: Session[]): number {
  return wrapIndex(index, sessions.length + SESSION_CREATE_ENTRY_COUNT);
}

export function readString(
  properties: Record<string, unknown> | undefined,
  key: string,
): string | undefined {
  const value = properties?.[key];
  return typeof value === "string" ? value : undefined;
}

export function upsertById<T extends { id: string }>(items: T[], item: T): T[] {
  return [...items.filter((existing) => existing.id !== item.id), item];
}

export function selectedSessionIndex(sessions: Session[], sessionID: string | undefined): number {
  const index = sessions.findIndex((session) => session.id === sessionID);
  return index >= 0 ? index : 0;
}

export function selectedPersonaIndex(
  personas: StoredPersona[],
  agents: StoredAgent[],
  session: Session | undefined,
  config: SessionConfig | undefined,
): number {
  const active = activePersonaID(agents, session, config);
  if (!active) return 0;
  const index = personas.findIndex((persona) => personaID(persona) === active);
  return index >= 0 ? index : 0;
}

function personaID(persona: StoredPersona): string | undefined {
  const configName = persona.config?.persona_name;
  return persona.summary?.id ?? (typeof configName === "string" ? configName : undefined);
}

function activePersonaID(
  agents: StoredAgent[],
  session: Session | undefined,
  config: SessionConfig | undefined,
): string | undefined {
  const agentID = session?.agent ?? config?.active_agent;
  const agent = agents.find((item) => storedAgentID(item) === agentID);
  const first = Array.isArray(agent?.config?.agent_persona)
    ? agent?.config?.agent_persona[0]
    : undefined;
  if (!first || typeof first !== "object" || Array.isArray(first)) return undefined;
  const name = (first as Record<string, unknown>).persona_name;
  if (typeof name === "string" && name.trim()) return name.trim();
  const runtimePersonas = (agent as unknown as { options?: { personas?: StoredPersona[] } }).options
    ?.personas;
  return runtimePersonas?.[0] ? personaID(runtimePersonas[0]) : "tura";
}

function storedAgentID(agent: StoredAgent): string | undefined {
  return agent.summary?.id ?? (agent as unknown as { name?: string }).name;
}

export function wrapIndex(index: number, length: number): number {
  if (length <= 0) return 0;
  return ((index % length) + length) % length;
}

export function modelCount(providers: ProviderListResponse | undefined): number {
  return (
    providers?.all.reduce(
      (count, provider) => count + Object.keys(provider.models ?? {}).length,
      0,
    ) ?? 0
  );
}

function settingProviders(
  providers: ProviderListResponse | undefined,
): ProviderListResponse["all"] {
  return (providers?.all ?? []).filter(isLlmProvider);
}

function isLlmProvider(provider: ProviderListResponse["all"][number]): boolean {
  const domains = stringArrayField(provider.options, "domains");
  if (domains.length) return domains.some((domain) => domain.toLowerCase() === "llm");
  const capabilities = stringArrayField(provider.options, "capabilities");
  if (capabilities.some((capability) => capability.toLowerCase().startsWith("llm."))) return true;
  return Object.keys(provider.models ?? {}).length > 0;
}

function stringArrayField(value: Record<string, unknown> | undefined, key: string): string[] {
  const item = value?.[key];
  return Array.isArray(item)
    ? item.filter((entry): entry is string => typeof entry === "string")
    : [];
}
