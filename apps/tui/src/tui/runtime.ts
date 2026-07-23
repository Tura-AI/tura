import { setTimeout as delay } from "node:timers/promises";
import type { GatewayClient } from "../gateway/client.js";
import { userFacingError } from "../gateway/errors.js";
import type { MockGatewayClient } from "../gateway/mock-client.js";
import type { ProviderAuthStatus } from "../types/provider.js";
import type { Session } from "../types/session.js";
import { isDraftSession, sessionSortAt } from "../types/session.js";
import { t } from "../i18n.js";
import { reducer, type AppAction, type AppState } from "./reducer.js";
import { createDraftSession } from "./session-state.js";

export type TuiGatewayClient = GatewayClient | MockGatewayClient;
export type TuiDispatch = (action: AppAction) => void;
export type TuiGetState = () => AppState;

export async function pickInitialSession(
  client: TuiGatewayClient,
  cwd: string,
  initialSessionId?: string,
): Promise<Session> {
  if (initialSessionId) return client.getSession(initialSessionId);
  const sessions = await client.listSessions({ includeChildren: true, limit: 20 });
  sessions.sort((left, right) => sessionSortAt(right) - sessionSortAt(left));
  if (sessions[0]) return sessions[0];
  return client.createSession().catch(() => createDraftSession(cwd));
}

export async function hydrate(
  state: AppState,
  client: TuiGatewayClient,
  session: Session,
): Promise<AppState> {
  const draft = isDraftSession(session);
  const [messages, providers, sessionConfig, modelConfig, agents, personas] = await Promise.all([
    draft ? Promise.resolve([]) : client.listMessages(session.id).catch(() => []),
    client.listProviders().catch(() => undefined),
    client.getSessionConfig().catch(() => undefined),
    client.modelConfig().catch(() => undefined),
    client.listAgents().catch(() => []),
    client.listPersonas().catch(() => []),
  ]);
  const auth = providers
    ? await fetchAuthSurface(
        client,
        providers.all.map((provider) => provider.id),
      )
    : {};
  const sessions = await client.listSessions({ includeChildren: true }).catch(() => []);
  const hydrated = reducer(
    reducer(state, {
      type: "hydrate",
      session,
      messages,
      permissions: [],
      providers,
      agents,
      personas,
      sessions,
      authMethods: auth.methods,
      authStatuses: auth.statuses,
      sessionConfig,
      modelConfig,
    }),
    {
      type: "questions",
      value: [],
    },
  );
  return shouldOpenProviderSettingsOnStartup(hydrated)
    ? reducer(hydrated, { type: "open-setting-detail", detail: "provider" })
    : hydrated;
}

export async function eventLoop(
  client: TuiGatewayClient,
  getState: TuiGetState,
  signal: AbortSignal,
  dispatch: TuiDispatch,
): Promise<void> {
  while (!signal.aborted) {
    const session = getState().session;
    if (!session || isDraftSession(session)) {
      await delay(250, undefined, { signal }).catch(() => undefined);
      continue;
    }

    const sessionID = session.id;
    const sessionController = new AbortController();
    const abortSessionStream = () => sessionController.abort(signal.reason);
    signal.addEventListener("abort", abortSessionStream, { once: true });
    const sessionWatcher = setInterval(() => {
      const activeSession = getState().session;
      if (!activeSession || activeSession.id !== sessionID || isDraftSession(activeSession)) {
        sessionController.abort();
      }
    }, 250);

    try {
      for await (const event of client.streamSessionEvents(sessionID, sessionController.signal)) {
        const activeSession = getState().session;
        if (!activeSession || activeSession.id !== sessionID) continue;
        dispatch({ type: "event", event });
      }
    } catch (error) {
      if (signal.aborted) return;
      if (sessionController.signal.aborted) continue;
      dispatch({
        type: "notice",
        value: t("eventStreamReconnecting", {
          error: userFacingError(error),
        }),
        transient: true,
      });
      await delay(1000, undefined, { signal }).catch(() => undefined);
    } finally {
      clearInterval(sessionWatcher);
      signal.removeEventListener("abort", abortSessionStream);
    }
  }
}

export async function fetchAuthSurface(
  client: TuiGatewayClient,
  providerIDs: string[],
): Promise<{
  methods?: Awaited<ReturnType<TuiGatewayClient["listProviderAuthMethods"]>>;
  statuses?: Record<string, ProviderAuthStatus>;
}> {
  const [methods, statuses] = await Promise.all([
    client.listProviderAuthMethods().catch(() => undefined),
    Promise.all(
      providerIDs.map(
        async (providerID) =>
          [providerID, await client.providerAuthStatus(providerID).catch(() => undefined)] as const,
      ),
    ).then((items) =>
      Object.fromEntries(
        items.filter((item): item is readonly [string, ProviderAuthStatus] => Boolean(item[1])),
      ),
    ),
  ]);
  return { methods, statuses };
}

function shouldOpenProviderSettingsOnStartup(state: AppState): boolean {
  const providers = settingProviders(state);
  return Boolean(state.providers) && configuredProviderCount(state, providers) === 0;
}

function settingProviders(state: AppState): NonNullable<AppState["providers"]>["all"] {
  return (state.providers?.all ?? []).filter(isLlmProvider);
}

function configuredProviderCount(state: AppState, providers = settingProviders(state)): number {
  const ids = new Set<string>();
  for (const id of state.providers?.connected ?? []) ids.add(id);
  for (const [id, status] of Object.entries(state.authStatuses)) {
    if (status.configured || status.authenticated) ids.add(id);
  }
  return providers.filter((provider) => ids.has(provider.id)).length;
}

function isLlmProvider(provider: NonNullable<AppState["providers"]>["all"][number]): boolean {
  const domains = stringArrayField(provider.options, "domains");
  if (domains.length) return domains.some((domain) => domain.toLowerCase() === "llm");
  const capabilities = stringArrayField(provider.options, "capabilities");
  if (capabilities.some((capability) => capability.toLowerCase().startsWith("llm."))) {
    return true;
  }
  return Object.keys(provider.models ?? {}).length > 0;
}

function stringArrayField(value: Record<string, unknown> | undefined, key: string): string[] {
  const item = value?.[key];
  return Array.isArray(item)
    ? item.filter((entry): entry is string => typeof entry === "string")
    : [];
}
