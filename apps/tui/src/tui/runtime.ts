import { setTimeout as delay } from "node:timers/promises";
import type { GatewayClient } from "../gateway/client.js";
import type { GatewayEventEnvelope } from "../types/event.js";
import { sameDirectory } from "../gateway/directory.js";
import { userFacingError } from "../gateway/errors.js";
import type { MockGatewayClient } from "../gateway/mock-client.js";
import type { ProviderAuthStatus } from "../types/provider.js";
import type { Message, MessagePart, Session } from "../types/session.js";
import { isDraftSession, sessionUpdatedAt } from "../types/session.js";
import { t } from "../i18n.js";
import { reducer, type AppAction, type AppState } from "./reducer.js";
import { createDraftSession } from "./session-state.js";

const LIVE_CACHE_HANDOFF_RETRY_MS = 50;
const LIVE_CACHE_HANDOFF_MAX_ATTEMPTS = 40;

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
  getState: TuiGetState,
  signal: AbortSignal,
  dispatch: TuiDispatch,
): Promise<void> {
  const handoffRefreshes = new Map<string, LiveCacheHandoffRefresh>();

  const scheduleLiveCacheHandoffRefresh = (sessionID: string, messageIDs: Iterable<string>) => {
    if (signal.aborted) return;
    let refresh = handoffRefreshes.get(sessionID);
    if (!refresh) {
      refresh = { messageIDs: new Set(), attempts: 0, running: false };
      handoffRefreshes.set(sessionID, refresh);
    }
    for (const messageID of messageIDs) refresh.messageIDs.add(messageID);
    if (!refresh.messageIDs.size || refresh.running || refresh.timer) return;
    refresh.timer = globalThis.setTimeout(() => {
      refresh.timer = undefined;
      void runLiveCacheHandoffRefresh(sessionID, refresh);
    }, 0);
  };

  const runLiveCacheHandoffRefresh = async (
    sessionID: string,
    refresh: LiveCacheHandoffRefresh,
  ) => {
    if (signal.aborted || refresh.running) return;
    const activeSession = getState().session;
    if (!activeSession || activeSession.id !== sessionID || isDraftSession(activeSession)) {
      handoffRefreshes.delete(sessionID);
      return;
    }

    refresh.running = true;
    refresh.attempts += 1;
    try {
      const [messages, session] = await Promise.all([
        client.listMessages(sessionID).catch(() => undefined),
        client.getSession(sessionID).catch(() => undefined),
      ]);
      if (signal.aborted) return;
      if (messages) dispatch({ type: "messages-incremental", sessionID, messages, session });
    } finally {
      refresh.running = false;
    }

    const remaining = liveMessageIDsForSession(getState(), sessionID, refresh.messageIDs);
    if (!remaining.size || refresh.attempts >= LIVE_CACHE_HANDOFF_MAX_ATTEMPTS || signal.aborted) {
      handoffRefreshes.delete(sessionID);
      return;
    }
    refresh.messageIDs = remaining;
    refresh.timer = globalThis.setTimeout(() => {
      refresh.timer = undefined;
      void runLiveCacheHandoffRefresh(sessionID, refresh);
    }, LIVE_CACHE_HANDOFF_RETRY_MS);
  };

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
        const completedLiveIDs = completedLiveMessageIDsAfterEvent(getState(), sessionID, event);
        if (completedLiveIDs.size) scheduleLiveCacheHandoffRefresh(sessionID, completedLiveIDs);
      }
    } catch (error) {
      if (signal.aborted) return;
      if (sessionController.signal.aborted) continue;
      dispatch({
        type: "notice",
        value: t("eventStreamReconnecting", {
          error: userFacingError(error),
        }),
      });
      await delay(1000, undefined, { signal }).catch(() => undefined);
    } finally {
      const refresh = handoffRefreshes.get(sessionID);
      if (refresh?.timer) clearTimeout(refresh.timer);
      handoffRefreshes.delete(sessionID);
      clearInterval(sessionWatcher);
      signal.removeEventListener("abort", abortSessionStream);
    }
  }
}

interface LiveCacheHandoffRefresh {
  messageIDs: Set<string>;
  attempts: number;
  running: boolean;
  timer?: ReturnType<typeof globalThis.setTimeout>;
}

function completedLiveMessageIDsAfterEvent(
  state: AppState,
  sessionID: string,
  event: GatewayEventEnvelope,
): Set<string> {
  const payload = event.payload;
  if (!payload) return new Set();
  if (payload.type === "session.status") {
    const status = readStatus(payload.properties);
    return status && status !== "busy" ? liveMessageIDsForSession(state, sessionID) : new Set();
  }
  if (payload.type === "session.updated") {
    const session = (payload.properties as { info?: Session } | undefined)?.info;
    return session?.id === sessionID && session.status && session.status !== "busy"
      ? liveMessageIDsForSession(state, sessionID)
      : new Set();
  }
  if (payload.type === "message.updated") {
    const message = (payload.properties as { info?: Message } | undefined)?.info;
    if (!message || !liveMessageIDsForSession(state, sessionID).has(message.id)) return new Set();
    return liveMessageIsFinished(state, message.id) ? new Set([message.id]) : new Set();
  }
  if (payload.type === "message.part.updated") {
    const part = (payload.properties as { part?: MessagePart } | undefined)?.part;
    const messageID = messageIDForPart(part);
    if (!messageID || !liveMessageIDsForSession(state, sessionID).has(messageID)) return new Set();
    return liveMessageIsFinished(state, messageID) ? new Set([messageID]) : new Set();
  }
  return new Set();
}

function liveMessageIDsForSession(
  state: AppState,
  sessionID: string,
  onlyIDs?: Set<string>,
): Set<string> {
  const ids = new Set<string>();
  for (const stream of Object.values(state.liveStreams)) {
    if (stream.sessionID && stream.sessionID !== sessionID) continue;
    if (onlyIDs && !onlyIDs.has(stream.messageID)) continue;
    ids.add(stream.messageID);
  }
  return ids;
}

function liveMessageIsFinished(state: AppState, messageID: string): boolean {
  const message = state.messages.find((item) => item.id === messageID);
  return !message || !messageHasRunningPart(message);
}

function readStatus(properties: unknown): Session["status"] | undefined {
  if (!properties || typeof properties !== "object") return undefined;
  const value = (properties as Record<string, unknown>).status;
  return value === "idle" || value === "busy" || value === "error" ? value : undefined;
}

function messageHasRunningPart(message: Message): boolean {
  return (message.parts ?? []).some((part) => partIsRunning(part));
}

function messageIDForPart(part: MessagePart | undefined): string | undefined {
  if (!part) return undefined;
  const direct = (part as { messageID?: unknown; message_id?: unknown }).messageID;
  if (typeof direct === "string") return direct;
  const snake = (part as { message_id?: unknown }).message_id;
  return typeof snake === "string" ? snake : undefined;
}

function partIsRunning(part: MessagePart): boolean {
  if (part.tool !== "command_run" && part.type !== "tool") return false;
  const state =
    part.state && typeof part.state === "object" ? (part.state as Record<string, unknown>) : {};
  const status = typeof state.status === "string" ? state.status : "";
  return /run|progress|pending|busy|question|in[_ -]?progress|exec(?:ute|uting|uted|ution)?|start/i.test(
    status,
  );
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
