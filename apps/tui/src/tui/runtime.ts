import { setTimeout as delay } from "node:timers/promises";
import type { GatewayClient } from "../gateway/client.js";
import { sameDirectory } from "../gateway/directory.js";
import { userFacingError } from "../gateway/errors.js";
import type { MockGatewayClient } from "../gateway/mock-client.js";
import type { ProviderAuthStatus } from "../types/provider.js";
import type { Session } from "../types/session.js";
import { isDraftSession, sessionUpdatedAt } from "../types/session.js";
import { t } from "../i18n.js";
import { reducer, type AppAction, type AppState } from "./reducer.js";
import { createDraftSession } from "./session-state.js";

export type TuiGatewayClient = GatewayClient | MockGatewayClient;
export type TuiDispatch = (action: AppAction) => void;
export type TuiGetState = () => AppState;

export async function pickInitialSession(client: TuiGatewayClient, cwd: string): Promise<Session> {
  const sessions = await client.listSessions({ limit: 20 });
  sessions.sort((left, right) => sessionUpdatedAt(right) - sessionUpdatedAt(left));
  if (sessions[0]) return sessions[0];
  return client.createSession().catch(() => createDraftSession(cwd));
}

export async function hydrate(
  state: AppState,
  client: TuiGatewayClient,
  session: Session,
): Promise<AppState> {
  const draft = isDraftSession(session);
  const [messages, providers, sessionConfig, agents, personas] = await Promise.all([
    draft ? Promise.resolve([]) : client.listMessages(session.id).catch(() => []),
    client.listProviders().catch(() => undefined),
    client.getSessionConfig().catch(() => undefined),
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
  return reducer(
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
    }),
    {
      type: "questions",
      value: [],
    },
  );
}

export async function eventLoop(
  client: TuiGatewayClient,
  signal: AbortSignal,
  dispatch: TuiDispatch,
): Promise<void> {
  while (!signal.aborted) {
    try {
      for await (const event of client.streamEvents(signal)) {
        dispatch({ type: "event", event });
      }
    } catch (error) {
      if (signal.aborted) return;
      dispatch({
        type: "notice",
        value: t("eventStreamReconnecting", {
          error: userFacingError(error),
        }),
      });
      await delay(1000);
    }
  }
}

export async function pollingLoop(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  signal: AbortSignal,
): Promise<void> {
  while (!signal.aborted) {
    const sessionID = getState().session?.id;
    if (sessionID && !isDraftSession(getState().session)) {
      await refreshActiveMessages(client, getState, dispatch, sessionID);
    }
    await delay(1500);
  }
}

async function refreshActiveMessages(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  sessionID: string,
): Promise<void> {
  const state = getState();
  const session = state.session;
  if (!session || session.id !== sessionID) return;
  const cursor = state.refreshState[sessionID];
  const messages = await client
    .listMessages(sessionID, cursor?.lastFinalMessageID ? { after: cursor.lastFinalMessageID } : {})
    .catch(() => undefined);
  if (!messages) return;
  const current = getState();
  const active = current.session;
  if (!active || active.id !== sessionID) return;
  if (!messages.length) return;
  dispatch({ type: "messages-incremental", sessionID, messages, session: active });
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

export function eventMatchesWorkspace(directory: string, cwd: string): boolean {
  return directory === "global" || sameDirectory(directory, cwd);
}
