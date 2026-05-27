import { createSignal, type Accessor, type Setter } from "solid-js";
import {
  errorMessage,
  type PollInterval,
  type Session,
  type TaskManagement,
  type PlanStatus,
} from "@tura/gateway-sdk";
import type { GatewayClient } from "@tura/gateway-sdk";
import type { AppState } from "../state/global-store";
import { sessionTitle } from "../state/global-store";
import {
  composerFileToken,
  composerImageToken,
} from "../conversation/conversation-view";
import {
  applyTaskPatchToSession,
  defaultLocalStartAt,
  defaultPollInterval,
  firstRunnableTask,
  formatTicketTime,
  localDateTimeToUtcIso,
  materializeComposerContent,
  normalizePollInterval,
  planSessionStatus,
  sessionAttentionKey,
  sessionTaskState,
  taskDisplayText,
  taskNonceId,
  taskPollInterval,
  taskStartAt,
  taskStartCondition,
  timedTaskPatch,
  utcIsoToLocalDateTime,
} from "../features/plan/tasks";

type PlanActionsOptions = {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  directoryClient: Accessor<GatewayClient>;
  e2eFixture?: string;
  openSession: (sessionId: string) => Promise<void>;
  createSessionPayload: () => Parameters<GatewayClient["createSession"]>[0];
  refreshSessions: () => Promise<void>;
  handleProviderAuthError: (error: unknown) => boolean;
};

export function usePlanActions(options: PlanActionsOptions) {
  const {
    state,
    setState,
    directoryClient,
    e2eFixture,
    openSession,
    createSessionPayload,
    refreshSessions,
    handleProviderAuthError,
  } = options;
  const [acknowledgedAttentionSessions, setAcknowledgedAttentionSessions] =
    createSignal(new Set<string>());

  async function openPlanSession(session: Session) {
    acknowledgeSessionAttention(session.id);
    setState((previous) => ({
      ...previous,
      planPreviewSessionId: session.id,
      selectedSessionId: session.id,
      planDraftLane: undefined,
      planDraftSessionId: undefined,
      editingTask: undefined,
      error: undefined,
    }));
    await openSession(session.id);
  }

  async function selectDraftSession(planDraftSessionId: string | undefined) {
    setState((previous) => ({
      ...previous,
      planDraftSessionId,
      planPreviewSessionId: planDraftSessionId,
      selectedSessionId: planDraftSessionId ?? previous.selectedSessionId,
      editingTask: undefined,
      error: undefined,
    }));
    if (planDraftSessionId) {
      await openSession(planDraftSessionId);
    }
  }

  function acknowledgeSessionAttention(sessionId: string) {
    const session = state().sessions.find((item) => item.id === sessionId);
    const key = session ? sessionAttentionKey(session) : undefined;
    if (!key) {
      return;
    }
    setAcknowledgedAttentionSessions((previous) => {
      const next = new Set(previous);
      next.add(key);
      return next;
    });
  }

  function sessionAttentionAcknowledged(session: Session): boolean {
    const key = sessionAttentionKey(session);
    return key ? acknowledgedAttentionSessions().has(key) : false;
  }

  async function updatePlanTicketStatus(session: Session, status: PlanStatus) {
    const currentStatus = planSessionStatus(session);
    if (status === "question") {
      await openPlanSession(session);
      return;
    }
    if (status === "doing") {
      if (currentStatus !== "todo" || !firstRunnableTask(session)) {
        await openPlanSession(session);
        setState((previous) => ({
          ...previous,
          composerText: firstRunnableTask(session)
            ? taskDisplayText(firstRunnableTask(session)!)
            : sessionTitle(session),
        }));
        return;
      }
      await startPlanTicketDoing(session);
      return;
    }
    await updatePlanTicketTask(session, { status: status });
  }

  async function startPlanTicketDoing(session: Session) {
    const task = firstRunnableTask(session);
    if (!task) {
      await openPlanSession(session);
      setState((previous) => ({
        ...previous,
        composerText: sessionTitle(session),
      }));
      return;
    }
    await updatePlanTicketTask(session, { status: "doing" });
    if (e2eFixture) {
      return;
    }
    try {
      await directoryClient().promptAsync(session.id, {
        parts: [{ type: "text", text: taskDisplayText(task) }],
        model: state().selectedModel,
        variant: state().modelVariant,
        model_acceleration_enabled: state().accelerationEnabled,
      });
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  async function updatePlanTicketTask(
    session: Session,
    patch: Partial<
      TaskManagement & {
        status: PlanStatus;
        start_at: string;
        poll_interval: PollInterval;
      }
    >,
  ) {
    if (
      patch.status &&
      !["todo", "doing", "question", "done", "archived"].includes(patch.status)
    ) {
      setState((previous) => ({
        ...previous,
        error: `Unsupported task status: ${patch.status}`,
      }));
      return;
    }
    setState((previous) => ({
      ...previous,
      sessions: previous.sessions.map((item) =>
        item.id === session.id ? applyTaskPatchToSession(item, patch) : item,
      ),
      error: undefined,
    }));
    if (e2eFixture) {
      return;
    }
    try {
      const updated = await directoryClient().updateSessionTaskManagement(
        session.id,
        patch,
      );
      setState((previous) => ({
        ...previous,
        sessions: previous.sessions.map((item) =>
          item.id === session.id ? { ...item, ...updated } : item,
        ),
      }));
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
      await refreshSessions();
    }
  }

  async function deletePlanTask(session: Session, task: TaskManagement) {
    await updatePlanTicketTask(session, {
      nonce_id: taskNonceId(task),
      status: "archived",
    });
  }

  async function createSessionFromPlanTask(
    session: Session,
    task: TaskManagement,
  ) {
    const title =
      taskDisplayText(task).split("\n")[0]?.trim() || sessionTitle(session);
    const patch = {
      ...task,
      nonce_id: `${session.id}:${Date.now()}`,
      status: "todo" as PlanStatus,
    };
    if (e2eFixture) {
      const next: Session = {
        ...session,
        id: `plan-task-session-${Date.now()}`,
        name: title,
        plan_summary: title,
        session_display_name: title,
        task_management: patch,
      };
      setState((previous) => ({
        ...previous,
        sessions: [next, ...previous.sessions],
        selectedSessionId: next.id,
        planPreviewSessionId: next.id,
        error: undefined,
      }));
      return;
    }
    const created = await directoryClient().createSession({
      ...createSessionPayload(),
      task_management: patch,
    });
    setState((previous) => ({
      ...previous,
      sessions: [created, ...previous.sessions],
      selectedSessionId: created.id,
      planPreviewSessionId: created.id,
      error: undefined,
    }));
  }

  async function updateEditingTaskFromComposer(): Promise<boolean> {
    const editing = state().editingTask;
    if (!editing) {
      return false;
    }
    const session = state().sessions.find(
      (item) => item.id === editing.sessionId,
    );
    if (!session) {
      return false;
    }
    const text = state().composerText.trim();
    if (!text) {
      return true;
    }
    const [summaryLine = "", ...deliveryLines] = text.split(/\r?\n/u);
    await updatePlanTicketTask(session, {
      nonce_id: editing.nonce_id,
      task_summary: summaryLine.trim(),
      delivery: deliveryLines.join("\n").trim(),
    });
    setState((previous) => ({
      ...previous,
      composerText: "",
      editingTask: undefined,
      error: undefined,
    }));
    return true;
  }

  async function createPlanTicket() {
    const title = state().composerText.trim();
    if (!title || !state().planDraftLane) {
      return;
    }
    const existingSession = state().planDraftSessionId
      ? state().sessions.find(
          (session) => session.id === state().planDraftSessionId,
        )
      : undefined;
    const startAt = localDateTimeToUtcIso(state().planDraftStartAt);
    const timingPatch = timedTaskPatch(
      state().planDraftStartCondition,
      startAt,
      state().planDraftPollInterval,
    );
    const taskState = {
      plan_summary: title,
      task_summary: title,
      ...(state().planDraftLane === "todo" || !state().planDraftLane
        ? {}
        : { status: state().planDraftLane }),
      ...timingPatch,
    };
    if (e2eFixture) {
      const session: Session = existingSession
        ? {
            ...existingSession,
            name: title,
            updated_at: Date.now(),
            plan_summary: title,
            session_display_name: title,
            task_management: {
              ...(existingSession.task_management ?? {}),
              ...taskState,
            },
          }
        : {
            id: `plan-local-${Date.now()}`,
            name: title,
            directory: state().directory,
            status: "idle",
            created_at: Date.now(),
            updated_at: Date.now(),
            plan_summary: title,
            session_display_name: title,
            task_management: taskState,
          };
      setState((previous) => ({
        ...previous,
        sessions: [
          session,
          ...previous.sessions.filter((item) => item.id !== session.id),
        ],
        selectedSessionId: session.id,
        planPreviewSessionId: session.id,
        composerText: "",
        planDraftLane: undefined,
        planDraftSessionId: undefined,
        planDraftStartAt: "",
        planDraftStartCondition: "user_action",
        planDraftPollInterval: defaultPollInterval(),
        error: undefined,
      }));
      return;
    }
    try {
      let session: Session | undefined;
      if (existingSession) {
        session = await directoryClient().updateSession(existingSession.id, {
          task_management: taskState,
        } as Partial<Session>);
      } else {
        session = await directoryClient().createSession({
          ...createSessionPayload(),
          task_management: taskState,
        });
      }
      setState((previous) => ({
        ...previous,
        sessions: session
          ? [
              session,
              ...previous.sessions.filter((item) => item.id !== session!.id),
            ]
          : previous.sessions,
        selectedSessionId: session?.id ?? previous.selectedSessionId,
        planPreviewSessionId: session?.id ?? previous.planPreviewSessionId,
        composerText: "",
        planDraftLane: undefined,
        planDraftSessionId: undefined,
        planDraftStartAt: "",
        planDraftStartCondition: "user_action",
        planDraftPollInterval: defaultPollInterval(),
        error: undefined,
      }));
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  return {
    openPlanSession,
    selectDraftSession,
    sessionAttentionAcknowledged,
    updatePlanTicketStatus,
    updatePlanTicketTask,
    deletePlanTask,
    createSessionFromPlanTask,
    acknowledgeSessionAttention,
    updateEditingTaskFromComposer,
    createPlanTicket,
  };
}
