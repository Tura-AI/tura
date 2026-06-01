import type { GatewayClient } from "@tura/gateway-sdk";
import {
  errorMessage,
  GatewayError,
  type Message,
  type PlanStatus,
  type PollInterval,
  type Session,
  type TaskManagement,
} from "@tura/gateway-sdk";
import { createSignal, type Accessor, type Setter } from "solid-js";
import {
  appendTaskToSession,
  applyTaskPatchToSession,
  defaultPollInterval,
  firstRunnableTask,
  localDateTimeToUtcIso,
  planSessionStatus,
  sessionAttentionKey,
  sessionTasks,
  taskDisplayText,
  taskNonceId,
  taskSummaryText,
  timedTaskPatch,
} from "../features/plan/tasks";
import { t } from "../i18n";
import type { AppState } from "../state/global-store";
import { sessionTitle } from "../state/global-store";
import {
  providerIdFromAuthError,
  providerIdFromModel,
} from "../utils/settings";

const PLAN_RUN_TIMEOUT_MS = 30_000;
const PLAN_RUN_TIMEOUT_CODE = "GATEWAY_NO_RESPONSE_30S";

function providerIssueIdFromError(
  error: unknown,
  state: AppState,
): string | undefined {
  const authProvider = providerIdFromAuthError(error, state);
  if (authProvider) {
    return authProvider;
  }
  if (!(error instanceof GatewayError)) {
    return undefined;
  }
  const bodyText = JSON.stringify(error.body ?? {}).toLowerCase();
  const messageText = error.message.toLowerCase();
  const billingLike =
    error.status === 402 ||
    /\b(billing|payment|quota|credit|balance|insufficient|subscription|rate_limit|rate limit|limit exceeded)\b/u.test(
      `${bodyText} ${messageText}`,
    );
  return billingLike ? providerIdFromModel(state.selectedModel) : undefined;
}

type PlanActionsOptions = {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  directoryClient: Accessor<GatewayClient>;
  e2eFixture?: string;
  openSession: (sessionId: string) => Promise<void>;
  createSessionPayload: () => Parameters<GatewayClient["createSession"]>[0];
  refreshSessions: () => Promise<void>;
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
      composerText: "",
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
        agent: state().selectedAgent,
      });
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
    }
  }

  async function runPlanTaskNow(session: Session, task: TaskManagement) {
    const text = taskDisplayText(task);
    const nonce = taskNonceId(task);
    const messageId = `plan-run:${session.id}:${nonce ?? Date.now()}`;
    const now = Date.now();
    const optimisticMessage: Message = {
      id: messageId,
      sessionID: session.id,
      session_id: session.id,
      role: "user",
      created_at: now,
      updated_at: now,
      time: { created: now, updated: now },
      parts: [
        {
          id: `${messageId}:text`,
          type: "text",
          text,
          metadata: {
            planRunPending: true,
            taskNonceId: nonce,
          },
        },
      ],
    };
    setState((previous) => ({
      ...previous,
      selectedSessionId: session.id,
      planPreviewSessionId: session.id,
      messagesBySession: {
        ...previous.messagesBySession,
        [session.id]: [
          ...(previous.messagesBySession[session.id] ?? []).filter(
            (message) => message.id !== messageId,
          ),
          optimisticMessage,
        ],
      },
      planNotice: undefined,
      error: undefined,
    }));
    if (e2eFixture) {
      await updatePlanTicketTask(session, {
        nonce_id: nonce,
        status: "archived",
      });
      const responseTime = Date.now();
      setState((previous) => ({
        ...previous,
        messagesBySession: {
          ...previous.messagesBySession,
          [session.id]: [
            ...(previous.messagesBySession[session.id] ?? []).map((message) =>
              message.id === messageId
                ? {
                    ...message,
                    updated_at: responseTime,
                    time: { ...message.time, updated: responseTime },
                    parts: message.parts.map((part) => ({
                      ...part,
                      metadata: {
                        ...(typeof part.metadata === "object" &&
                        part.metadata !== null
                          ? part.metadata
                          : {}),
                        planRunPending: false,
                      },
                    })),
                  }
                : message,
            ),
            {
              id: `${messageId}:gateway-response`,
              sessionID: session.id,
              session_id: session.id,
              role: "assistant",
              providerID: "mock",
              modelID: "gateway",
              created_at: responseTime + 1,
              updated_at: responseTime + 1,
              time: { created: responseTime + 1, updated: responseTime + 1 },
              parts: [
                {
                  id: `${messageId}:gateway-response:text`,
                  type: "text",
                  text: `Gateway 已接收立即执行任务：${taskSummaryText(task)}`,
                },
              ],
            },
          ],
        },
      }));
      return;
    }
    try {
      await Promise.race([
        directoryClient().promptAsync(session.id, {
          parts: [{ type: "text", text }],
          model: state().selectedModel,
          agent: state().selectedAgent,
        }),
        new Promise<never>((_, reject) =>
          window.setTimeout(
            () => reject(new Error(PLAN_RUN_TIMEOUT_CODE)),
            PLAN_RUN_TIMEOUT_MS,
          ),
        ),
      ]);
      await updatePlanTicketTask(session, {
        nonce_id: nonce,
        status: "archived",
      });
    } catch (error) {
      const timeout =
        error instanceof Error && error.message === PLAN_RUN_TIMEOUT_CODE;
      const responseTime = Date.now();
      setState((previous) => ({
        ...previous,
        planNotice: timeout
          ? {
              message: "Gateway 30 秒内没有响应立即执行请求。",
              code: PLAN_RUN_TIMEOUT_CODE,
            }
          : {
              message: errorMessage(error),
              code: "GATEWAY_RUN_FAILED",
              providerId: providerIssueIdFromError(error, previous),
            },
        messagesBySession: {
          ...previous.messagesBySession,
          [session.id]: (previous.messagesBySession[session.id] ?? []).map(
            (message) =>
              message.id === messageId
                ? {
                    ...message,
                    updated_at: responseTime,
                    time: { ...message.time, updated: responseTime },
                    parts: message.parts.map((part) => ({
                      ...part,
                      metadata: {
                        ...(typeof part.metadata === "object" &&
                        part.metadata !== null
                          ? part.metadata
                          : {}),
                        planRunPending: false,
                        planRunError: true,
                      },
                    })),
                  }
                : message,
          ),
        },
        error: undefined,
      }));
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
      taskPulse:
        patch.nonce_id && isTaskTimingPatch(patch)
          ? {
              sessionId: session.id,
              nonce_id: patch.nonce_id,
              token: Date.now(),
            }
          : previous.taskPulse,
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
          item.id === session.id
            ? mergeTaskUpdateResponse(item, updated, patch)
            : item,
        ),
      }));
    } catch (error) {
      setState((previous) => ({ ...previous, error: errorMessage(error) }));
      await refreshSessions();
    }
  }

  function mergeTaskUpdateResponse(
    current: Session,
    updated: Session,
    patch: Partial<
      TaskManagement & {
        status: PlanStatus;
        start_at: string;
        poll_interval: PollInterval;
      }
    >,
  ): Session {
    const nonce = patch.nonce_id;
    if (!nonce) {
      return { ...current, ...updated };
    }
    const patchKeys = new Set(Object.keys(patch));
    const currentTask = sessionTasks(current).find(
      (task) => taskNonceId(task) === nonce,
    );
    const updatedTask = sessionTasks(updated).find(
      (task) => taskNonceId(task) === nonce,
    );
    if (!currentTask || !updatedTask) {
      return applyTaskPatchToSession(current, patch);
    }
    const mergedTask: TaskManagement = { ...updatedTask };
    for (const [key, value] of Object.entries(currentTask)) {
      if (!patchKeys.has(key)) {
        (mergedTask as Record<string, unknown>)[key] = value;
      }
    }
    return applyTaskPatchToSession(current, mergedTask);
  }

  function isTaskTimingPatch(
    patch: Partial<
      TaskManagement & {
        status: PlanStatus;
        start_at: string;
        poll_interval: PollInterval;
      }
    >,
  ): boolean {
    return (
      "start_condition" in patch ||
      "start_at" in patch ||
      "poll_interval" in patch
    );
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
    const summary = taskSummaryText(task).trim();
    if (state().activeTab === "plan") {
      setState((previous) => ({
        ...previous,
        planDraftLane: "todo",
        planDraftSessionId: undefined,
        planPreviewSessionId: undefined,
        selectedSessionId: previous.selectedSessionId,
        composerText: summary || t("newTask"),
        editingTask: undefined,
        planDraftStartAt: "",
        planDraftStartCondition: "user_action",
        planDraftPollInterval: defaultPollInterval(),
        error: undefined,
      }));
      return;
    }
    const composerText = taskDisplayText(task);
    const title =
      summary || composerText.split("\n")[0]?.trim() || t("newTask");
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
        activeTab: "conversation",
        previousMainTab:
          previous.activeTab === "settings"
            ? previous.previousMainTab
            : previous.activeTab,
        composerText,
        editingTask: undefined,
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
      activeTab: "conversation",
      previousMainTab:
        previous.activeTab === "settings"
          ? previous.previousMainTab
          : previous.activeTab,
      composerText,
      editingTask: undefined,
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
      setState((previous) => ({
        ...previous,
        composerText: "",
        editingTask: undefined,
        error: undefined,
      }));
      return true;
    }
    const [summaryLine = "", ...deliveryLines] = text.split(/\r?\n/u);
    const pulseToken = Date.now();
    setState((previous) => ({
      ...previous,
      taskPulse: {
        sessionId: editing.sessionId,
        nonce_id: editing.nonce_id,
        token: pulseToken,
      },
    }));
    await updatePlanTicketTask(session, {
      nonce_id: editing.nonce_id,
      task_summary: summaryLine.trim(),
      delivery: deliveryLines.join("\n").trim(),
    });
    setState((previous) => ({
      ...previous,
      composerText: "",
      editingTask: undefined,
      taskPulse: {
        sessionId: editing.sessionId,
        nonce_id: editing.nonce_id,
        token: pulseToken,
      },
      error: undefined,
    }));
    return true;
  }

  async function createPlanTicket(sessionIdOverride?: string) {
    const title = state().composerText.trim();
    const draftLane = state().planDraftLane ?? "todo";
    if (!title || (!state().planDraftLane && !sessionIdOverride)) {
      return;
    }
    const draftSessionId = sessionIdOverride ?? state().planDraftSessionId;
    const existingSession = draftSessionId
      ? state().sessions.find((session) => session.id === draftSessionId)
      : undefined;
    const startAt = localDateTimeToUtcIso(state().planDraftStartAt);
    const timingPatch = timedTaskPatch(
      state().planDraftStartCondition,
      startAt,
      state().planDraftPollInterval,
    );
    const nonceId = existingSession
      ? `${existingSession.id}:${Date.now()}`
      : `plan-task:${Date.now()}`;
    const baseTaskState = {
      nonce_id: nonceId,
      step: existingSession ? sessionTasks(existingSession).length : 0,
      plan_summary: title,
      task_summary: title,
      ...(draftLane === "todo" ? {} : { status: draftLane }),
      ...timingPatch,
    };
    const taskState = baseTaskState;
    if (e2eFixture) {
      const session: Session = existingSession
        ? {
            ...appendTaskToSession(existingSession, taskState),
            updated_at: Date.now(),
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
        session = await directoryClient().updateSessionTaskManagement(
          existingSession.id,
          { tasks: [taskState] },
        );
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
    runPlanTaskNow,
    createSessionFromPlanTask,
    acknowledgeSessionAttention,
    updateEditingTaskFromComposer,
    createPlanTicket,
  };
}
