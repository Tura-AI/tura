import type { CreateSessionRequest, Session } from "../types/session.js";
import { t } from "../i18n.js";
import type { promptRuntimeSelection } from "./logic/selection.js";
import type { AppState } from "./reducer.js";
import { isBusyState } from "./busy-state.js";

let draftSessionCounter = 0;

export function createDraftSession(cwd: string): Session {
  draftSessionCounter += 1;
  return {
    id: `draft-session-${Date.now()}-${draftSessionCounter}`,
    draft: true,
    name: t("newSession"),
    directory: cwd,
    status: "idle",
    updated_at: Date.now(),
    message_count: 0,
  };
}

export function createSessionRequest(
  runtimeSelection: ReturnType<typeof promptRuntimeSelection>,
): CreateSessionRequest {
  return {
    ...(runtimeSelection.model ? { model: runtimeSelection.model } : {}),
    ...(runtimeSelection.agent ? { agent: runtimeSelection.agent } : {}),
    ...(runtimeSelection.modelVariant ? { model_variant: runtimeSelection.modelVariant } : {}),
    ...(runtimeSelection.modelAccelerationEnabled !== undefined
      ? { model_acceleration_enabled: runtimeSelection.modelAccelerationEnabled }
      : {}),
  };
}

export function shouldApplyInitialHydrate(state: AppState, sessionID: string): boolean {
  if (isBusyState(state)) return state.session?.id === sessionID;
  return !state.session || state.session.id === sessionID;
}

export function upsertSessionLocal(sessions: Session[], session: Session): Session[] {
  return [...sessions.filter((item) => item.id !== session.id), session];
}
