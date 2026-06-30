import { isDraftSession, sessionSortAt, sessionTitle, type Session } from "../types/session.js";
import type { AppState } from "./reducer.js";
import { clearTerminalForSurfaceTransition } from "./draw.js";
import { lastMessagePreview } from "./services/message-preview.js";
import { hydrate, type TuiDispatch, type TuiGatewayClient, type TuiGetState } from "./runtime.js";
import { createAndSelectSession, loadAndSelectSession } from "./session-actions.js";
import { t } from "../i18n.js";

const SESSION_PREVIEW_FETCH_LIMIT = 8;
export const SESSION_PICKER_REFRESH_MS = 1000;
export const DELETE_SESSION_TIMEOUT_MS = 5000;

export async function openSessionPicker(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
): Promise<void> {
  const state = getState();
  if (state.sessionLoading) return;
  if (state.sessionsOpen) {
    clearTerminalForSurfaceTransition();
    dispatch({ type: "toggle-sessions" });
    return;
  }
  clearTerminalForSurfaceTransition();
  dispatch({
    type: "sessions",
    value: sortedSessions(state.sessions.length ? state.sessions : activeSessionList(state)),
    open: true,
  });
  void refreshOpenSessionPicker(client, getState, dispatch);
}

async function sessionPreviews(
  client: TuiGatewayClient,
  sessions: Session[],
  cachedPreviews: Record<string, string> = {},
): Promise<Record<string, string>> {
  const entries = await Promise.all(
    sessions.slice(0, SESSION_PREVIEW_FETCH_LIMIT).map(async (session) => {
      const cached = cachedPreviews[session.id];
      if (cached) return [session.id, cached] as const;
      const messages = await client.listMessages(session.id, { limit: 1 }).catch(() => []);
      const preview = lastMessagePreview(messages);
      return preview ? ([session.id, preview] as const) : undefined;
    }),
  );
  return Object.fromEntries(
    entries.filter((entry): entry is readonly [string, string] => Boolean(entry)),
  );
}

export async function refreshOpenSessionPicker(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
): Promise<void> {
  if (getState().sessionLoading) return;
  const sessions = sortedSessions(
    await client.listSessions({ includeChildren: true }).catch(() => getState().sessions),
  );
  if (!getState().sessionsOpen) return;
  dispatch({ type: "sessions", value: sessions, open: true });
  const previews = await sessionPreviews(client, sessions, getState().sessionPreviews);
  if (!getState().sessionsOpen) return;
  dispatch({ type: "session-previews", value: previews });
}

export async function forkSelectedSession(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
): Promise<void> {
  const target = selectedSession(getState());
  if (getState().sessionLoading) return;
  if (!target || isDraftSession(target)) {
    dispatch({ type: "notice", value: t("noSessionSelected") });
    return;
  }
  clearTerminalForSurfaceTransition();
  dispatch({
    type: "session-loading",
    value: { sessionID: target.id, title: sessionTitle(target) },
  });
  try {
    const session = await client.forkSession(target.id, { copy_context: true });
    const next = await hydrate(getState(), client, session);
    dispatch({
      type: "hydrate",
      session: next.session!,
      messages: next.messages,
      permissions: next.permissions,
      providers: next.providers,
      agents: next.agents,
      personas: next.personas,
      sessions: next.sessions,
      closePanels: true,
    });
    dispatch({ type: "questions", value: next.questions });
    dispatch({ type: "notice", value: t("sessionCopied") });
  } catch (error) {
    dispatch({ type: "session-loading", value: undefined });
    throw error;
  }
}

export async function deleteSelectedSession(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  timeoutMs = DELETE_SESSION_TIMEOUT_MS,
): Promise<void> {
  const state = getState();
  if (state.sessionLoading) return;
  const target = state.selectedSessionIndex > 0 ? selectedSession(state) : undefined;
  if (!target || isDraftSession(target)) {
    dispatch({ type: "notice", value: t("noSessionSelected") });
    return;
  }
  const wasActive = target.id === getState().session?.id;
  dispatch({
    type: "session-loading",
    value: { kind: "deleting", sessionID: target.id, title: sessionTitle(target) },
  });
  let deleteResult: "deleted" | "timed-out";
  try {
    deleteResult = await deleteSessionWithTimeout(client, target.id, timeoutMs);
  } finally {
    dispatch({ type: "session-loading", value: undefined });
  }
  if (deleteResult === "timed-out") {
    dispatch({ type: "notice", value: t("sessionDeleteTimeout") });
    return;
  }
  const sessions = sortedSessions(
    await client.listSessions({ includeChildren: true }).catch(() => []),
  );
  dispatch({ type: "sessions", value: sessions, open: true });
  dispatch({ type: "notice", value: t("sessionDeleted") });

  if (!wasActive) return;
  const replacement = sessions[0];
  if (replacement) {
    await loadAndSelectSession(client, getState, dispatch, replacement, false);
    const next = getState();
    dispatch({ type: "sessions", value: next.sessions, open: true });
  } else {
    await createAndSelectSession(client, getState, dispatch, false);
    dispatch({ type: "sessions", value: getState().sessions, open: true });
  }
}

async function deleteSessionWithTimeout(
  client: TuiGatewayClient,
  sessionID: string,
  timeoutMs: number,
): Promise<"deleted" | "timed-out"> {
  const deletePromise = client.deleteSession(sessionID).then(() => "deleted" as const);
  deletePromise.catch(() => undefined);
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      deletePromise,
      new Promise<"timed-out">((resolve) => {
        timeout = setTimeout(() => resolve("timed-out"), Math.max(0, timeoutMs));
      }),
    ]);
  } finally {
    if (timeout) clearTimeout(timeout);
  }
}

function activeSessionList(state: AppState): Session[] {
  return state.session && !isDraftSession(state.session) ? [state.session] : [];
}

function sortedSessions(sessions: Session[]): Session[] {
  return [...sessions].sort((left, right) => sessionSortAt(right) - sessionSortAt(left));
}

function selectedSession(state: AppState): Session | undefined {
  if (!state.sessionsOpen) return undefined;
  if (state.selectedSessionIndex === 0) return state.session;
  return state.sessions[state.selectedSessionIndex - 1];
}
