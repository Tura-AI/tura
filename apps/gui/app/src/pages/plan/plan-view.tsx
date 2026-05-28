import {
  For,
  Match,
  Show,
  Switch,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
  type Accessor,
  type JSX,
  type Setter,
} from "solid-js";
import { Portal } from "solid-js/web";
import ExternalLink from "lucide-solid/icons/external-link";
import LayoutList from "lucide-solid/icons/layout-list";
import ArrowLeft from "lucide-solid/icons/arrow-left";
import CalendarDays from "lucide-solid/icons/calendar-days";
import ChartGantt from "lucide-solid/icons/chart-gantt";
import Check from "lucide-solid/icons/check";
import ChevronDown from "lucide-solid/icons/chevron-down";
import ChevronLeft from "lucide-solid/icons/chevron-left";
import ChevronRight from "lucide-solid/icons/chevron-right";
import Columns3 from "lucide-solid/icons/columns-3";
import Copy from "lucide-solid/icons/copy";
import Edit3 from "lucide-solid/icons/pencil";
import FolderOpen from "lucide-solid/icons/folder-open";
import KeyRound from "lucide-solid/icons/key-round";
import MoreHorizontal from "lucide-solid/icons/ellipsis";
import Pin from "lucide-solid/icons/pin";
import Plus from "lucide-solid/icons/plus";
import Search from "lucide-solid/icons/search";
import Settings from "lucide-solid/icons/settings";
import Trash2 from "lucide-solid/icons/trash-2";
import {
  GatewayClient,
  GatewayError,
  connectGatewayEvents,
  defaultGatewayUrl,
  errorMessage,
  type Agent,
  type Command,
  type FileContentResponse,
  type FileInfo,
  type GatewayConfig,
  type Message,
  type ProviderAuthMethod,
  type ProductIssue,
  type Project,
  type PollInterval,
  type SdkProvider,
  type Session,
  type StartCondition,
  type TaskManagement,
  type PlanStatus,
} from "@tura/gateway-sdk";
import {
  Composer,
  ConversationView,
  composerFileToken,
  composerImageToken,
} from "../../conversation/conversation-view";
import { applyGatewayEvent } from "../../state/event-reducer";
import {
  activeSession,
  type ComposerImage,
  initialAppState,
  type MainTab,
  type PlanMode,
  sessionDirectory,
  sessionTitle,
  type AppState,
  type SettingsSection,
  type ThemeMode,
} from "../../state/global-store";
import { classNames, truncate } from "../../state/format";
import { t, type TextKey } from "../../i18n";

import { PlanGanttView } from "./plan-gantt";
import { PlanCalendarView } from "./plan-calendar";
import {
  PlanModeButtons,
  PlanComposerControls,
  PlanComposerTaskList,
  PlanDraftSessionPicker,
  PlanConversationFeedbackNotice,
  PlanScheduleDialog,
  PlanTicketMeta,
  shouldShowPlanFeedbackPrompt,
} from "./plan-composer";
import {
  defaultPollInterval,
  formatTicketTime,
  hasVisibleSessionTasks,
  localDateTimeToUtcIso,
  planSessionStatus,
  planTaskTitle,
  sessionTaskState,
  sessionTasks,
  shortSessionId,
  taskDisplayText,
  taskNonceId,
  taskPollInterval,
  taskStartAt,
  taskStartCondition,
  timedTaskPatch,
  utcIsoToLocalDateTime,
} from "../../features/plan/tasks";
import {
  PlanDragGhost,
  beginPlanPointerDrag,
  type PlanDragState,
} from "../../features/plan/drag";
import {
  relativeSessionTime,
  samePath,
  sessionHoverTitle,
  shortSessionTitle,
  shortWorkspaceLabel,
} from "../../utils/app-format";

const PLAN_PANEL_MIN_WIDTH = 320;
const PLAN_PANEL_MAX_WIDTH = 680;
const PLAN_PANEL_COLLAPSE_WIDTH = 260;
const PLAN_MAIN_MIN_WIDTH = 430;

export function PlanView(props: {
  state: AppState;
  previewSession?: Session;
  previewMessages: Message[];
  slashCommands: Command[];
  onPlanMode: (value: PlanMode) => void;
  onSearch: (value: string) => void;
  onDraftLane: (value: PlanStatus | undefined) => void;
  onDraftStartCondition: (value: StartCondition) => void;
  onDraftStartAt: (value: string) => void;
  onDraftPollInterval: (value: PollInterval) => void;
  onDraftSession: (value: string | undefined) => void;
  onCreateTicket: (sessionId?: string) => void;
  onStatus: (session: Session, status: PlanStatus) => void;
  attentionAcknowledged: (session: Session) => boolean;
  onTask: (
    session: Session,
    patch: Partial<
      TaskManagement & {
        status: PlanStatus;
        start_at: string;
        poll_interval: PollInterval;
      }
    >,
  ) => void;
  onEditTask: (
    session: Session,
    task: TaskManagement,
    composerText: string,
  ) => void;
  onDeleteTask: (session: Session, task: TaskManagement) => void;
  onRunTask: (session: Session, task: TaskManagement) => void;
  onCreateSessionFromTask: (session: Session, task: TaskManagement) => void;
  onOpenSession: (session: Session) => void;
  onComposerText: (text: string) => void;
  onComposerImages: (images: ComposerImage[]) => void;
  onSubmit: () => void;
  onOpenProviderSettings?: (providerId?: string) => void;
  onClosePanel: () => void;
}) {
  const workspaceSessions = createMemo(() =>
    props.state.sessions.filter((session) =>
      samePath(sessionDirectory(session), props.state.directory),
    ),
  );
  const visibleSessions = createMemo(() => {
    const query = props.state.issueSearch.trim().toLowerCase();
    const sessions = workspaceSessions().filter(
      (session) => planSessionStatus(session) !== "archived",
    );
    if (!query) {
      return sessions;
    }
    return sessions.filter(
      (session) =>
        sessionTitle(session).toLowerCase().includes(query) ||
        session.id.toLowerCase().includes(query),
    );
  });
  const panelOpen = createMemo(() =>
    Boolean(props.previewSession || props.state.planDraftLane),
  );
  const editingTask = createMemo(() => {
    const preview = props.previewSession;
    const editing = props.state.editingTask;
    if (!preview || !editing || editing.sessionId !== preview.id) {
      return undefined;
    }
    return sessionTasks(preview).find(
      (task) => taskNonceId(task) === editing.nonce_id,
    );
  });
  const composerTask = createMemo(() => {
    const preview = props.previewSession;
    if (!preview) {
      return undefined;
    }
    return (
      editingTask() ?? sessionTasks(preview)[0] ?? sessionTaskState(preview)
    );
  });
  const composerTaskNonce = createMemo(() => {
    const preview = props.previewSession;
    const task = composerTask();
    if (!preview || !task) {
      return undefined;
    }
    return editingTask() || Array.isArray(sessionTaskState(preview).tasks)
      ? taskNonceId(task)
      : undefined;
  });
  const draftTitle = createMemo(() => {
    const title = props.state.composerText.split(/\r?\n/u)[0]?.trim();
    return title || t("newTask");
  });
  function taskWithComposerText(task: TaskManagement): TaskManagement {
    const text = props.state.composerText.trim();
    if (!text) {
      return task;
    }
    const [summaryLine = "", ...deliveryLines] = text.split(/\r?\n/u);
    return {
      ...task,
      task_summary: summaryLine.trim(),
      delivery: deliveryLines.join("\n").trim(),
    };
  }
  const [panelWidth, setPanelWidth] = createSignal(430);
  const [workbenchWidth, setWorkbenchWidth] = createSignal(0);
  let workbenchEl: HTMLElement | undefined;
  let workbenchResizeObserver: ResizeObserver | undefined;
  const planPanelFullscreen = createMemo(
    () => panelOpen() && workbenchWidth() - panelWidth() < PLAN_MAIN_MIN_WIDTH,
  );

  onMount(() => {
    const updateWorkbenchWidth = () =>
      setWorkbenchWidth(
        workbenchEl?.getBoundingClientRect().width ?? window.innerWidth,
      );
    updateWorkbenchWidth();
    workbenchResizeObserver = new ResizeObserver(updateWorkbenchWidth);
    if (workbenchEl) {
      workbenchResizeObserver.observe(workbenchEl);
    }
    window.addEventListener("resize", updateWorkbenchWidth);
    onCleanup(() => {
      window.removeEventListener("resize", updateWorkbenchWidth);
      workbenchResizeObserver?.disconnect();
    });
  });

  function beginPanelResize(event: PointerEvent) {
    event.preventDefault();
    const workbench =
      (event.currentTarget as HTMLElement).closest(".plan-workbench") ??
      undefined;
    const workbenchWidth =
      workbench?.getBoundingClientRect().width ?? window.innerWidth;
    const startX = event.clientX;
    const startWidth = panelWidth();
    const onMove = (move: PointerEvent) => {
      const nextWidth = startWidth + startX - move.clientX;
      if (nextWidth <= PLAN_PANEL_COLLAPSE_WIDTH) {
        setPanelWidth(PLAN_PANEL_MIN_WIDTH);
        onUp();
        closePlanPanel();
        return;
      }
      const maxWidth = Math.max(
        PLAN_PANEL_MIN_WIDTH,
        Math.min(
          PLAN_PANEL_MAX_WIDTH,
          workbenchWidth - PLAN_MAIN_MIN_WIDTH - 12,
        ),
      );
      setPanelWidth(
        Math.min(maxWidth, Math.max(PLAN_PANEL_MIN_WIDTH, nextWidth)),
      );
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  function closePlanPanel() {
    props.onClosePanel();
  }

  function openDraft(lane: PlanStatus | undefined) {
    props.onDraftLane(lane);
    props.onDraftSession(undefined);
    props.onDraftStartCondition("user_action");
    props.onDraftStartAt("");
    props.onDraftPollInterval(defaultPollInterval());
    props.onComposerText("");
  }

  function openDraftAt(startAt: string) {
    props.onDraftLane("todo");
    props.onDraftSession(undefined);
    props.onDraftStartCondition("scheduled_task");
    props.onDraftStartAt(utcIsoToLocalDateTime(startAt));
    props.onDraftPollInterval(defaultPollInterval());
    props.onComposerText("");
  }

  function toggleSplitPanel() {
    if (panelOpen()) {
      closePlanPanel();
      return;
    }
    const session =
      workspaceSessions().find(
        (item) => item.id === props.state.selectedSessionId,
      ) ?? visibleSessions()[0];
    if (session) {
      void props.onOpenSession(session);
    }
  }
  function submitComposer() {
    if (props.state.editingTask) {
      props.onSubmit();
      return;
    }
    if (props.state.planDraftLane) {
      props.onCreateTicket();
      return;
    }
    if (props.previewSession && props.state.composerText.trim()) {
      props.onCreateTicket(props.previewSession.id);
      return;
    }
    props.onSubmit();
  }
  return (
    <section
      class={classNames(
        "product-workbench plan-workbench",
        panelOpen() && "plan-split-workbench",
        planPanelFullscreen() && "plan-panel-fullscreen",
      )}
      ref={workbenchEl}
      style={{
        "--plan-panel-width": `${panelWidth()}px`,
      }}
    >
      <div class="plan-main">
        <header class="page-head plan-head">
          <div class="page-title">
            <span>{t("plan")}</span>
            <h1>{shortWorkspaceLabel(props.state.directory)}</h1>
          </div>
          <div class="page-actions">
            <label class="search-box">
              <input
                value={props.state.issueSearch}
                onInput={(event) => props.onSearch(event.currentTarget.value)}
                placeholder={t("search")}
              />
            </label>
            <PlanModeButtons
              mode={props.state.planMode}
              splitOpen={panelOpen()}
              onMode={props.onPlanMode}
              onSplit={toggleSplitPanel}
            />
          </div>
        </header>

        <main
          class={classNames(
            "plan-board",
            props.state.planMode === "calendar" && "calendar-mode",
          )}
        >
          <Switch>
            <Match when={props.state.planMode === "gantt"}>
              <PlanGanttView
                sessions={visibleSessions()}
                selectedSessionId={props.state.planPreviewSessionId}
                selectedTaskNonceId={props.state.editingTask?.nonce_id}
                onOpenSession={props.onOpenSession}
                onEditTask={(session, task) => {
                  props.onOpenSession(session);
                  props.onEditTask(session, task, taskDisplayText(task));
                }}
                onSchedule={(session, task, startAt) =>
                  props.onTask(session, {
                    nonce_id: taskNonceId(task),
                    start_at: startAt,
                  })
                }
              />
            </Match>
            <Match when={props.state.planMode === "calendar"}>
              <PlanCalendarView
                sessions={visibleSessions()}
                selectedSessionId={props.state.planPreviewSessionId}
                onOpenSession={props.onOpenSession}
                onCreateAt={openDraftAt}
                onSchedule={(session, startAt) =>
                  props.onTask(session, {
                    start_at: startAt,
                  })
                }
              />
            </Match>
            <Match when={true}>
              <PlanBoard
                sessions={visibleSessions()}
                selectedSessionId={props.state.planPreviewSessionId}
                draftLane={props.state.planDraftLane}
                onDraftLane={openDraft}
                onStatus={props.onStatus}
                attentionAcknowledged={props.attentionAcknowledged}
                onOpenSession={props.onOpenSession}
              />
            </Match>
          </Switch>
        </main>
      </div>

      <Show when={panelOpen()}>
        <aside
          class="plan-conversation-panel"
          style={{
            width: `${panelWidth()}px`,
          }}
        >
          <div
            class="inspector-resize plan-panel-resize"
            role="separator"
            aria-orientation="vertical"
            onPointerDown={beginPanelResize}
          />
          <header class="plan-panel-topbar">
            <div class="plan-panel-title">
              <span>{t("conversation")}</span>
              <strong>
                {props.state.planDraftLane
                  ? draftTitle()
                  : props.previewSession
                    ? sessionTitle(props.previewSession)
                    : t("conversation")}
              </strong>
            </div>
            <button
              class="inspector-close"
              title={t("close")}
              onClick={closePlanPanel}
            >
              ×
            </button>
          </header>
          <ConversationView
            state={props.state}
            session={props.previewSession}
            messages={props.previewMessages}
            slashCommands={props.slashCommands}
            onComposerText={props.onComposerText}
            onComposerImages={props.onComposerImages}
            onSubmit={submitComposer}
            submitDisabled={
              Boolean(props.state.planDraftLane) &&
              props.state.composerText.trim().length === 0
            }
            composerToolbar={
              props.state.planDraftLane ? (
                <div class="plan-composer-tools">
                  <PlanDraftSessionPicker
                    sessions={workspaceSessions()}
                    selectedSessionId={props.state.planDraftSessionId}
                    onSession={props.onDraftSession}
                  />
                  <PlanComposerControls
                    startCondition={props.state.planDraftStartCondition}
                    startAt={props.state.planDraftStartAt}
                    pollInterval={props.state.planDraftPollInterval}
                    onStartCondition={props.onDraftStartCondition}
                    onStartAt={props.onDraftStartAt}
                    onPollInterval={props.onDraftPollInterval}
                  />
                </div>
              ) : props.previewSession && !props.state.editingTask ? (
                <PlanComposerControls
                  startCondition={props.state.planDraftStartCondition}
                  startAt={props.state.planDraftStartAt}
                  pollInterval={props.state.planDraftPollInterval}
                  onStartCondition={props.onDraftStartCondition}
                  onStartAt={props.onDraftStartAt}
                  onPollInterval={props.onDraftPollInterval}
                />
              ) : props.previewSession ? (
                <PlanComposerControls
                  startCondition={taskStartCondition(composerTask()!)}
                  startAt={utcIsoToLocalDateTime(taskStartAt(composerTask()!))}
                  pollInterval={taskPollInterval(composerTask()!)}
                  onStartCondition={(startCondition) => {
                    const currentTask = composerTask()!;
                    if (startCondition === "user_action") {
                      props.onRunTask(
                        props.previewSession!,
                        taskWithComposerText(currentTask),
                      );
                      return;
                    }
                    const startAt = localDateTimeToUtcIso(
                      utcIsoToLocalDateTime(taskStartAt(currentTask)),
                    );
                    props.onTask(props.previewSession!, {
                      nonce_id: composerTaskNonce(),
                      status: "todo",
                      ...timedTaskPatch(
                        startCondition,
                        startAt,
                        taskPollInterval(currentTask),
                      ),
                    });
                  }}
                  onStartAt={(value) => {
                    const start_at = localDateTimeToUtcIso(value);
                    if (start_at) {
                      props.onTask(props.previewSession!, {
                        nonce_id: composerTaskNonce(),
                        start_at,
                      });
                    }
                  }}
                  onPollInterval={(poll_interval) =>
                    props.onTask(props.previewSession!, {
                      nonce_id: composerTaskNonce(),
                      poll_interval,
                    })
                  }
                />
              ) : undefined
            }
            composerTaskList={
              props.previewSession &&
              !props.state.planDraftLane &&
              hasVisibleSessionTasks(props.previewSession) ? (
                <PlanComposerTaskList
                  session={props.previewSession}
                  selected_nonce_id={props.state.editingTask?.nonce_id}
                  pulseNonceId={
                    props.state.taskPulse?.sessionId === props.previewSession.id
                      ? props.state.taskPulse.nonce_id
                      : undefined
                  }
                  pulseToken={
                    props.state.taskPulse?.sessionId === props.previewSession.id
                      ? props.state.taskPulse.token
                      : undefined
                  }
                  onEdit={(task, composerText) =>
                    props.onEditTask(props.previewSession!, task, composerText)
                  }
                  onDelete={(task) =>
                    props.onDeleteTask(props.previewSession!, task)
                  }
                  onRun={(task) =>
                    props.onRunTask(props.previewSession!, task)
                  }
                  onCreateSession={(task) =>
                    props.onCreateSessionFromTask(props.previewSession!, task)
                  }
                />
              ) : undefined
            }
            conversationNotice={
              props.state.planNotice ? (
                <PlanConversationFeedbackNotice
                  message={props.state.planNotice.message}
                  code={props.state.planNotice.code}
                  providerId={props.state.planNotice.providerId}
                  onOpenProviderSettings={props.onOpenProviderSettings}
                />
              ) : props.previewSession &&
              shouldShowPlanFeedbackPrompt(
                props.previewSession,
                props.state.composerText,
              ) ? (
                <PlanConversationFeedbackNotice />
              ) : undefined
            }
            compact
            compactInspector
          />
        </aside>
      </Show>
    </section>
  );
}

export function PlanBoard(props: {
  sessions: Session[];
  selectedSessionId?: string;
  draftLane?: PlanStatus;
  onDraftLane: (value: PlanStatus | undefined) => void;
  onStatus: (session: Session, status: PlanStatus) => void;
  attentionAcknowledged: (session: Session) => boolean;
  onOpenSession: (session: Session) => void;
}) {
  const columns: Array<{ id: PlanStatus; label: string }> = [
    { id: "todo", label: t("todo") },
    { id: "doing", label: t("doing") },
    { id: "question", label: t("question") },
    { id: "done", label: t("done") },
  ];
  const [dragState, setDragState] = createSignal<PlanDragState>();
  function dragSession(event: DragEvent): Session | undefined {
    return props.sessions.find(
      (item) => item.id === event.dataTransfer?.getData("text/session-id"),
    );
  }
  function dropOnStatus(event: DragEvent, status: PlanStatus) {
    event.preventDefault();
    const session = dragSession(event);
    if (session) {
      props.onStatus(session, status);
    }
  }
  function beginBoardDrag(event: PointerEvent | MouseEvent, session: Session) {
    beginPlanPointerDrag({
      event,
      session,
      setDragState,
      onOpen: () => props.onOpenSession(session),
      onSchedule: () => undefined,
      resolveSchedule: () => undefined,
      onDrop: (point) => {
        const element = document.elementFromPoint(point.x, point.y) as
          | HTMLElement
          | undefined;
        const archive = element?.closest<HTMLElement>(".board-archive-zone");
        if (archive) {
          props.onStatus(session, "archived");
          return true;
        }
        const column = element?.closest<HTMLElement>("[data-plan-status]");
        const status = column?.dataset.planStatus as PlanStatus | undefined;
        if (status && ["todo", "doing", "question", "done"].includes(status)) {
          props.onStatus(session, status);
          return true;
        }
        return false;
      },
    });
  }
  return (
    <section class="board-shell">
      <PlanDragGhost state={dragState()} />
      <section class="board-grid">
        <For each={columns}>
          {(column) => {
            const sessions = createMemo(() =>
              props.sessions.filter(
                (session) => planSessionStatus(session) === column.id,
              ),
            );
            return (
              <section
                class="board-column"
                data-plan-status={column.id}
                onDragOver={(event) => event.preventDefault()}
                onDrop={(event) => dropOnStatus(event, column.id)}
              >
                <header>
                  <span class="board-column-title">
                    <span>{column.label}</span>
                  </span>
                  <Show when={column.id === "todo"}>
                    <button
                      class="icon-action small"
                      title={t("create")}
                      onClick={() => props.onDraftLane(column.id)}
                    >
                      <Plus size={15} />
                    </button>
                  </Show>
                </header>
                <div
                  class={classNames(
                    "board-cards",
                    props.draftLane === column.id && "draft-target",
                  )}
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={(event) => dropOnStatus(event, column.id)}
                >
                  <For each={sessions()}>
                    {(session) => (
                      <article
                        class={classNames(
                          "board-card",
                          props.selectedSessionId === session.id && "selected",
                        )}
                        draggable="true"
                        onPointerDown={(event) =>
                          beginBoardDrag(event, session)
                        }
                        onMouseDown={(event) => beginBoardDrag(event, session)}
                        onDragStart={(event) => {
                          event.dataTransfer?.setData(
                            "text/session-id",
                            session.id,
                          );
                          event.currentTarget.classList.add(
                            "plan-source-dragging",
                          );
                        }}
                        onDragEnd={(event) =>
                          event.currentTarget.classList.remove(
                            "plan-source-dragging",
                          )
                        }
                        onClick={() => props.onOpenSession(session)}
                        title={sessionTitle(session)}
                      >
                        <small>{shortSessionId(session.id)}</small>
                        <span class="board-card-title">
                          <strong>{sessionTitle(session)}</strong>
                          <Show
                            when={shouldShowSessionAttention(
                              session,
                              props.attentionAcknowledged(session),
                            )}
                          >
                            <PlanStatusIndicator
                              status={planSessionStatus(session)}
                            />
                          </Show>
                        </span>
                        <PlanTicketMeta session={session} />
                      </article>
                    )}
                  </For>
                </div>
              </section>
            );
          }}
        </For>
      </section>
      <div
        class={classNames("board-archive-zone", dragState() && "active")}
        aria-hidden="true"
        onDragOver={(event) => event.preventDefault()}
        onDrop={(event) => {
          event.preventDefault();
          const session = dragSession(event);
          if (session) {
            props.onStatus(session, "archived");
          }
        }}
      />
    </section>
  );
}

let activePlanPointerDrag = false;

export function PlanStatusIndicator(props: { status: PlanStatus }) {
  return (
    <Show
      when={
        props.status === "doing" ||
        props.status === "question" ||
        props.status === "done"
      }
    >
      <span
        class={classNames("plan-status-indicator", `status-${props.status}`)}
        aria-hidden="true"
      />
    </Show>
  );
}

export function shouldShowSessionAttention(
  session: Session,
  acknowledged: boolean,
): boolean {
  const status = planSessionStatus(session);
  return (
    !acknowledged &&
    (status === "doing" || status === "question" || status === "done")
  );
}

export function SessionRowMeta(props: {
  session: Session;
  attentionAcknowledged: boolean;
}) {
  const status = createMemo(() => planSessionStatus(props.session));
  return (
    <Show
      when={shouldShowSessionAttention(
        props.session,
        props.attentionAcknowledged,
      )}
      fallback={
        <small class="session-row-time">
          {relativeSessionTime(props.session)}
        </small>
      }
    >
      <span class="session-row-status">
        <PlanStatusIndicator status={status()} />
      </span>
    </Show>
  );
}
