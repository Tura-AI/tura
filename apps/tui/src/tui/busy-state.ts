import type { AppState } from "./reducer.js";
import { sessionHasQuestionStatus } from "../types/session.js";
import { commandStatusIsRunning } from "../types/event.js";

export function hasActiveAnimation(state: AppState): boolean {
  if (state.sessionLoading) return true;
  if (isBusyState(state)) return true;
  if (hasVisibleBusySession(state)) return true;
  if (state.questions.length || state.permissions.length) return true;
  return false;
}

function hasVisibleBusySession(state: AppState): boolean {
  return (
    state.sessionsOpen &&
    state.sessions.some(
      (session) =>
        session.status === "busy" ||
        sessionHasRunningCommand(state, session.id) ||
        sessionHasQuestionStatus(session),
    )
  );
}

export function sessionHasRunningCommand(state: AppState, sessionID: string): boolean {
  return Object.values(state.commandStatesBySession[sessionID] ?? {}).some((command) =>
    commandStatusIsRunning(command.status),
  );
}

export function isBusyState(state: AppState): boolean {
  if (state.status === "error" || state.session?.status === "error") return false;
  const listStatus = activeSessionListStatus(state);
  if (listStatus === "error") return false;
  const activeSessionID = state.session?.id;
  return (
    state.status === "busy" ||
    state.session?.status === "busy" ||
    listStatus === "busy" ||
    Boolean(activeSessionID && sessionHasRunningCommand(state, activeSessionID)) ||
    hasPendingUserTurn(state) ||
    (!hasExplicitIdleState(state, listStatus) &&
      (hasActiveLiveStream(state) || hasRunningCommand(state)))
  );
}

function hasExplicitIdleState(
  state: AppState,
  listStatus: AppState["status"] | undefined,
): boolean {
  return state.status === "idle" || state.session?.status === "idle" || listStatus === "idle";
}

function commandStatus(value: unknown): string {
  if (!value || typeof value !== "object") return "";
  const status = (value as { status?: unknown }).status;
  return typeof status === "string" ? status : "";
}

function activeSessionListStatus(state: AppState): AppState["status"] | undefined {
  const sessionID = state.session?.id;
  if (!sessionID) return undefined;
  return state.sessions.find((session) => session.id === sessionID)?.status;
}

function hasActiveLiveStream(state: AppState): boolean {
  const sessionID = state.session?.id;
  return Object.values(state.liveStreams).some(
    (stream) => !sessionID || !stream.sessionID || stream.sessionID === sessionID,
  );
}

function hasPendingUserTurn(state: AppState): boolean {
  return state.messages.at(-1)?.role === "user";
}

function hasRunningCommand(state: AppState): boolean {
  return state.messages.some((message) =>
    (message.parts ?? []).some((part) => {
      if (part.tool !== "command_run" && part.type !== "tool") return false;
      const status = commandStatus(part.state);
      return /run|progress|pending|busy|question|in[_ -]?progress|exec(?:ute|uting|uted|ution)?|start/i.test(
        status,
      );
    }),
  );
}
