import type { CreateSessionRequest, Session } from "../types/session.js";
import { t } from "../i18n.js";
import type { promptRuntimeSelection } from "./logic/selection.js";
import type { AppState } from "./reducer.js";
import { isBusyState } from "./busy-state.js";

let draftSessionCounter = 0;

export function createDraftSession(cwd: string): Session {
  draftSessionCounter += 1;
  const now = Date.now();
  return {
    id: `draft-session-${now}-${draftSessionCounter}`,
    draft: true,
    name: t("newSession"),
    parent_id: null,
    created_at: now,
    directory: cwd,
    model: null,
    agent: null,
    session_type: "coding",
    auto_session_name: true,
    kill_processes_on_start: false,
    validator_enabled: false,
    force_planning: false,
    model_variant: null,
    model_acceleration_enabled: false,
    disable_permission_restrictions: false,
    status: "idle",
    updated_at: now,
    message_count: 0,
    task_management: {},
    plan_summary: null,
    session_display_name: t("newSession"),
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
