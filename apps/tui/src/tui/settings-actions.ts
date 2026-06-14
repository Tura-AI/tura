import type { Session } from "../types/session.js";
import { isDraftSession } from "../types/session.js";
import { parseLanguage, setLanguage, t } from "../i18n.js";
import { settingOptions } from "./render.js";
import { settingPatch } from "./logic/selection.js";
import type { AppState } from "./reducer.js";
import {
  fetchAuthSurface,
  type TuiDispatch,
  type TuiGatewayClient,
  type TuiGetState,
} from "./runtime.js";

export async function applySelectedSetting(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
): Promise<void> {
  const state = getState();
  const detail = state.settingDetail;
  if (!detail) return;
  const selected = settingOptions(state)[state.selectedSettingOptionIndex];
  if (!selected) return;
  const value = selected[2];
  if (detail === "model") {
    if (typeof value !== "string" || !state.session?.id) return;
    await updateActiveSession(client, getState, dispatch, { model: value });
    dispatch({ type: "notice", value: t("settingsUpdated") });
    return;
  }
  if (detail === "agent") {
    if (typeof value !== "string" || !state.session?.id) return;
    await updateActiveSession(client, getState, dispatch, { agent: value });
    dispatch({ type: "notice", value: t("settingsUpdated") });
    return;
  }
  if (detail === "persona" && typeof value === "string") {
    await applyPersonaToActiveAgent(client, getState, dispatch, value);
    dispatch({ type: "close-setting-detail" });
    dispatch({ type: "notice", value: t("settingsUpdated") });
    return;
  }
  if (detail === "provider" && typeof value === "string") {
    const auth = await fetchAuthSurface(client, [value]);
    dispatch({ type: "auth", methods: auth.methods, statuses: auth.statuses, open: false });
    dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID: value });
    return;
  }
  if (detail === "providerAuth") {
    await applyProviderAuthAction(client, getState, dispatch, value);
    return;
  }
  const patch = settingPatch(detail, value);
  if (!patch) return;
  const config = await client.patchSessionConfig(patch);
  if (detail === "language" && typeof value === "string") {
    setLanguage(parseLanguage(value));
  }
  dispatch({ type: "session-config", value: config, open: true });
  dispatch({ type: "open-setting-detail", detail });
  dispatch({ type: "notice", value: t("settingsUpdated") });
}

export async function updateActiveSession(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  patch: Partial<Session>,
): Promise<Session | undefined> {
  const state = getState();
  const active = state.session;
  if (!active) return undefined;
  const session = isDraftSession(active)
    ? { ...active, ...patch, updated_at: Date.now() }
    : await client.updateSession(active.id, patch);
  dispatchHydrateFromState(
    dispatch,
    state,
    session,
    await client.getSessionConfig().catch(() => state.sessionConfig),
  );
  return session;
}

function dispatchHydrateFromState(
  dispatch: TuiDispatch,
  state: AppState,
  session: Session,
  sessionConfig: AppState["sessionConfig"],
): void {
  dispatch({
    type: "hydrate",
    session,
    messages: state.messages,
    permissions: state.permissions,
    providers: state.providers,
    agents: state.agents,
    personas: state.personas,
    sessions: state.sessions,
    sessionConfig,
  });
}

async function applyProviderAuthAction(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  value: unknown,
): Promise<void> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return;
  const action = value as { action?: string; providerID?: string; method?: number };
  const providerID = action.providerID;
  if (!providerID) return;
  if (action.action === "oauth") {
    const auth = await client.providerOauthAuthorize(providerID, action.method ?? 0);
    const status = await client.providerAuthStatus(providerID).catch(() => undefined);
    dispatch({
      type: "auth",
      statuses: status
        ? { ...getState().authStatuses, [providerID]: status }
        : getState().authStatuses,
      open: false,
    });
    dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID });
    dispatch({
      type: "setting-input",
      value: {
        kind: "oauth-callback",
        providerID,
        prompt: t("oauthCallbackInputHint"),
      },
    });
    dispatch({
      type: "notice",
      value: [auth.instructions, auth.url ? t("openUrl", { url: auth.url }) : undefined]
        .filter(Boolean)
        .join(" "),
    });
    return;
  }
  if (action.action === "logout") {
    await client.providerLogout(providerID);
    const status = await client.providerAuthStatus(providerID).catch(() => undefined);
    dispatch({
      type: "auth",
      statuses: status
        ? { ...getState().authStatuses, [providerID]: status }
        : getState().authStatuses,
      open: false,
    });
    dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID });
    dispatch({ type: "notice", value: t("settingsUpdated") });
    return;
  }
  if (action.action === "api-key") {
    const method = (getState().authMethods?.[providerID] ?? []).find((item) =>
      /key|token|api/i.test(
        [item.type, item.kind, item.login, item.label].filter(Boolean).join(" "),
      ),
    );
    dispatch({
      type: "setting-input",
      value: {
        kind: "api-key",
        providerID,
        prompt: t("apiKeyInputHint", { provider: providerID }),
      },
    });
    dispatch({
      type: "notice",
      value: [
        method?.api_key_url ? t("openUrl", { url: method.api_key_url }) : undefined,
        method?.docs_url,
      ]
        .filter(Boolean)
        .join(" "),
    });
  }
}

export async function submitSettingInput(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
): Promise<void> {
  const state = getState();
  const input = state.settingInput;
  const value = state.composer.trim();
  if (!input || !value) return;
  if (input.kind === "api-key") {
    await client.setProviderAuth(input.providerID, { type: "api_key", key: value });
  } else {
    await client.setProviderAuth(input.providerID, {
      type: "oauth",
      access: value,
      metadata: { callback_url_or_token: value },
    });
  }
  const status = await client.providerAuthStatus(input.providerID).catch(() => undefined);
  dispatch({
    type: "auth",
    statuses: status
      ? { ...getState().authStatuses, [input.providerID]: status }
      : getState().authStatuses,
    open: false,
  });
  dispatch({ type: "setting-input", value: undefined });
  dispatch({ type: "composer", value: "" });
  dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID: input.providerID });
  dispatch({ type: "notice", value: t("settingsUpdated") });
}

function personaID(persona: AppState["personas"][number] | undefined): string | undefined {
  const configName = persona?.config?.persona_name;
  return persona?.summary?.id ?? (typeof configName === "string" ? configName : undefined);
}

export async function applyPersonaToActiveAgent(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  targetPersonaID: string,
): Promise<void> {
  const state = getState();
  const agentID = state.session?.agent ?? state.sessionConfig?.active_agent;
  if (!agentID) throw new Error("No active agent selected.");
  const persona =
    state.personas.find((item) => personaID(item) === targetPersonaID) ??
    (await client.getPersona(targetPersonaID));
  const stored = await client.getAgent(agentID);
  const config = {
    ...stored.config,
    agent_persona: [
      {
        persona_name: targetPersonaID,
        persona_directory:
          persona.config?.persona_directory ??
          persona.summary?.path ??
          `personas/src/${targetPersonaID}`,
      },
    ],
  };
  const updated = await client.updateAgent(agentID, { config, prompt: stored.prompt ?? undefined });
  const agents = state.agents.map((agent) => (storedAgentID(agent) === agentID ? updated : agent));
  dispatch({
    type: "agents",
    value: agents.length === state.agents.length ? agents : [updated, ...state.agents],
  });
  dispatch({
    type: "personas",
    value: await client.listPersonas().catch(() => state.personas),
    open: state.personasOpen,
  });
}

function storedAgentID(agent: AppState["agents"][number]): string | undefined {
  return agent.summary?.id ?? (agent as unknown as { name?: string }).name;
}
