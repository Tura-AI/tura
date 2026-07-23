import type { Session } from "../types/session.js";
import { isDraftSession } from "../types/session.js";
import { parseLanguage, setLanguage, t } from "../i18n.js";
import { settingOptions } from "./render.js";
import { settingPatch } from "./logic/selection.js";
import type { AboutOpenTarget, AboutUiAction } from "../types/about.js";
import type { AppState } from "./reducer.js";
import {
  fetchAuthSurface,
  type TuiDispatch,
  type TuiGatewayClient,
  type TuiGetState,
} from "./runtime.js";
import { openExternalUrl } from "../utils/external-url.js";

export async function applySelectedSetting(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  onExit?: () => void,
): Promise<void> {
  const state = getState();
  const detail = state.settingDetail;
  if (!detail) return;
  const selected = settingOptions(state)[state.selectedSettingOptionIndex];
  if (!selected) return;
  const value = selected[2];
  if (detail === "about" && typeof value === "string") {
    await applyAboutAction(client, getState, dispatch, value as AboutUiAction, onExit);
    return;
  }
  if (detail === "model") {
    if (typeof value !== "string") return;
    const config = await client.patchSessionConfig(settingPatch(detail, value) ?? { model: value });
    completeSettingSelection(dispatch, config, detail, selected[0]);
    return;
  }
  if (detail === "agent") {
    if (typeof value !== "string") return;
    const config = await client.patchSessionConfig({ active_agent: value });
    completeSettingSelection(dispatch, config, detail, selected[0]);
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
  completeSettingSelection(dispatch, config, detail, selected[0]);
}

function completeSettingSelection(
  dispatch: TuiDispatch,
  config: AppState["sessionConfig"],
  detail: Exclude<AppState["settingDetail"], undefined>,
  value: string,
): void {
  if (!config) return;
  dispatch({ type: "session-config", value: config });
  dispatch({ type: "close-panels" });
  dispatch({
    type: "notice",
    value: t("settingApplied", { setting: settingLabel(detail), value }),
  });
}

function settingLabel(detail: Exclude<AppState["settingDetail"], undefined>): string {
  const labels: Record<Exclude<AppState["settingDetail"], undefined>, string> = {
    model: t("settingModel"),
    provider: t("settingProvider"),
    providerAuth: t("settingProvider"),
    agent: t("settingAgent"),
    persona: t("settingPersona"),
    language: t("settingLanguage"),
    session: t("settingSession"),
    variant: t("settingReasoning"),
    priority: t("settingPriority"),
    validator: t("settingValidator"),
    stallGuard: t("settingStallGuard"),
    about: t("settingAbout"),
  };
  return labels[detail];
}

export async function applyAboutAction(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  action: AboutUiAction,
  onExit?: () => void,
): Promise<void> {
  if (action === "addStar") {
    const result = await client.starTuraRepository();
    dispatch({
      type: "notice",
      value: result.outcome === "starred" ? t("aboutStarred") : t("aboutStarOpened"),
    });
    return;
  }
  const target = aboutOpenTarget(action);
  if (target) {
    await client.openAboutTarget(target);
    dispatch({ type: "notice", value: t("aboutLinkOpened", { target }) });
    return;
  }
  if (action === "update") {
    dispatch({ type: "notice", value: t("aboutCheckingUpdate") });
    const result = await client.checkTuraUpdate();
    dispatch({ type: "about-update", value: result.update });
    dispatch({ type: "notice", value: result.update ? undefined : t("aboutNoUpdate") });
    return;
  }
  if (action === "cancelUpdate") {
    dispatch({ type: "about-update", value: undefined });
    dispatch({ type: "notice", value: t("aboutUpdateCancelled") });
    return;
  }
  const update = getState().aboutUpdate;
  if (action !== "confirmUpdate" || !update) return;
  dispatch({ type: "notice", value: t("aboutUpdating") });
  const result = await client.installTuraUpdate(update.latest_version, getState().session?.id);
  dispatch({ type: "about-update", value: undefined });
  dispatch({ type: "notice", value: t("aboutUpdated", { version: result.version }) });
  onExit?.();
}

function aboutOpenTarget(action: AboutUiAction): AboutOpenTarget | undefined {
  if (action === "reportBug") return "report_bug";
  if (action === "contribute") return "contribute";
  if (action === "contact") return "contact";
  return undefined;
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
    modelConfig: state.modelConfig,
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
    await startProviderOauthLogin(client, getState, dispatch, providerID, action.method ?? 0);
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
    dispatch({ type: "notice", value: undefined });
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

export async function startProviderOauthLogin(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  providerID: string,
  methodIndex = 0,
): Promise<void> {
  const auth = await client.providerOauthAuthorize(providerID, methodIndex);
  const openResult = auth.url ? await openExternalUrl(auth.url) : undefined;
  const status = await client.providerAuthStatus(providerID).catch(() => undefined);
  dispatchAuthStatus(dispatch, getState, providerID, status);
  dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID });
  dispatch({
    type: "setting-input",
    value: {
      kind: "oauth-callback",
      providerID,
      method: methodIndex,
      oauthUrl: auth.url || undefined,
      prompt:
        auth.method === "auto"
          ? t("waitingOauthCallback")
          : providerID === "github-copilot"
            ? auth.instructions
            : t("oauthCallbackInputHint"),
    },
  });
  dispatch({
    type: "notice",
    value: [auth.instructions, openResult && !openResult.ok ? openResult.reason : undefined]
      .filter(Boolean)
      .join(" "),
  });
  if (auth.method === "auto") {
    void waitForProviderAuthenticated(client, getState, dispatch, providerID);
  } else if (providerID === "github-copilot") {
    void completeProviderOauthCallback(client, getState, dispatch, providerID, methodIndex, "");
  }
}

async function waitForProviderAuthenticated(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  providerID: string,
  timeoutMs = 5 * 60_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const status = await client.providerAuthStatus(providerID).catch(() => undefined);
    if (status) {
      dispatchAuthStatus(dispatch, getState, providerID, status);
      if (status.authenticated) {
        dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID });
        dispatch({ type: "setting-input", value: undefined });
        dispatch({ type: "composer", value: "" });
        dispatch({ type: "notice", value: t("connected") });
        return;
      }
    }
    await sleep(1000);
  }
  dispatch({ type: "notice", value: t("loginPending") });
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
    const method = (getState().authMethods?.[input.providerID] ?? []).find((item) =>
      /key|token|api/i.test(
        [item.type, item.kind, item.login, item.label].filter(Boolean).join(" "),
      ),
    );
    const validation = await client.providerAuthValidate(input.providerID, {
      type: method?.type ?? "api_key",
      kind: method?.kind ?? "api_key",
      login: method?.login ?? "api",
      token_env: method?.token_env,
      key: value,
      access: value,
    });
    if (!validation.ok) {
      dispatchAuthStatus(dispatch, getState, input.providerID, validation.status);
      dispatch({ type: "notice", value: validation.message });
      return;
    }
    await client.setProviderAuth(input.providerID, { type: "api_key", key: value });
    const status = await client.providerAuthStatus(input.providerID).catch(() => validation.status);
    dispatchAuthStatus(dispatch, getState, input.providerID, status);
    dispatch({ type: "setting-input", value: undefined });
    dispatch({ type: "composer", value: "" });
    dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID: input.providerID });
    dispatch({ type: "notice", value: validation.message });
    return;
  } else {
    await completeProviderOauthCallback(
      client,
      getState,
      dispatch,
      input.providerID,
      input.method ?? 0,
      value,
    );
    return;
  }
}

async function completeProviderOauthCallback(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  providerID: string,
  method: number,
  code: string,
): Promise<void> {
  const result = await client.providerOauthCallback(providerID, {
    method,
    code: code.trim() || undefined,
  });
  dispatchAuthStatus(dispatch, getState, providerID, result.status);
  dispatch({ type: "notice", value: result.ok ? t("connected") : result.message });
  if (!result.ok) return;
  dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID });
  dispatch({ type: "setting-input", value: undefined });
  dispatch({ type: "composer", value: "" });
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function dispatchAuthStatus(
  dispatch: TuiDispatch,
  getState: TuiGetState,
  providerID: string,
  status: Awaited<ReturnType<TuiGatewayClient["providerAuthStatus"]>> | null | undefined,
): void {
  dispatch({
    type: "auth",
    statuses: status
      ? { ...getState().authStatuses, [providerID]: status }
      : getState().authStatuses,
    open: false,
  });
}
