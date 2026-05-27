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
import Edit3 from "lucide-solid/icons/edit-3";
import FolderOpen from "lucide-solid/icons/folder-open";
import KeyRound from "lucide-solid/icons/key-round";
import MoreHorizontal from "lucide-solid/icons/more-horizontal";
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
  sessionUpdatedAt,
  sessionTitle,
  type AppState,
  type SettingsSection,
  type ThemeMode,
} from "../../state/global-store";
import { classNames, truncate } from "../../state/format";
import { t, type TextKey } from "../../i18n";

import {
  applyTaskPatchToSession,
  defaultLocalStartAt,
  defaultPollInterval,
  firstRunnableTask,
  formatStartCondition,
  formatTaskRemaining,
  formatTicketTime,
  hasVisibleSessionTasks,
  isTimedStartCondition,
  localDateTimeToUtcIso,
  normalizeIntervalPart,
  normalizePollInterval,
  planSessionStartCondition,
  planSessionStatus,
  sessionTaskState,
  sessionTasks,
  shortSessionId,
  sortedSessionTasks,
  taskDisplayText,
  taskNonceId,
  taskPollInterval,
  taskStartAt,
  taskStartCondition,
  taskStateLabel,
  taskSummaryText,
  timedTaskPatch,
  utcIsoToLocalDateTime,
} from "../../features/plan/tasks";
import { inputHeight } from "../../utils/app-format";
export function PlanModeButtons(props: {
  mode: PlanMode;
  splitOpen: boolean;
  onMode: (value: PlanMode) => void;
  onSplit: () => void;
}) {
  const modes: Array<{
    id: PlanMode | "split";
    label: string;
    icon: (props: { size?: number }) => JSX.Element;
  }> = [
    { id: "gantt", label: t("gantt"), icon: ChartGantt },
    { id: "calendar", label: t("calendar"), icon: CalendarDays },
    { id: "todo", label: t("todoList"), icon: LayoutList },
    { id: "split", label: t("splitCollaboration"), icon: Columns3 },
  ];
  return (
    <div class="plan-mode-actions">
      <For each={modes}>
        {(mode) => {
          const Icon = mode.icon;
          return (
            <button
              class={classNames(
                "icon-action",
                (mode.id === "split"
                  ? props.splitOpen
                  : props.mode === mode.id) && "selected",
              )}
              title={mode.label}
              onClick={() =>
                mode.id === "split" ? props.onSplit() : props.onMode(mode.id)
              }
            >
              <Icon size={17} />
            </button>
          );
        }}
      </For>
    </div>
  );
}

export function PlanTicketMeta(props: { session: Session }) {
  const task = createMemo(() => sessionTaskState(props.session));
  const condition = createMemo(() => taskStartCondition(task()));
  return (
    <div class="ticket-meta">
      <span>{formatStartCondition(condition())}</span>
      <Show when={isTimedStartCondition(condition())}>
        <span>{formatTicketTime(task().start_at)}</span>
      </Show>
    </div>
  );
}

export function PlanComposerTaskList(props: {
  session: Session;
  selected_nonce_id?: string;
  onEdit: (task: TaskManagement, value: string) => void;
  onDelete: (task: TaskManagement) => void;
  onCreateSession: (task: TaskManagement) => void;
}) {
  const [menuNonce, setMenuNonce] = createSignal<string>();
  const tasks = createMemo(() => sortedSessionTasks(props.session));
  const queuedTasks = createMemo(() =>
    tasks().filter((task) => !isTimedStartCondition(taskStartCondition(task))),
  );
  const timedTasks = createMemo(() =>
    tasks().filter((task) => isTimedStartCondition(taskStartCondition(task))),
  );
  return (
    <Show when={tasks().length > 0}>
      <section class="composer-task-list" aria-label={t("taskManagement")}>
        <For each={queuedTasks()}>
          {(task) => (
            <PlanTaskRow
              task={task}
              selected={props.selected_nonce_id === taskNonceId(task)}
              menuOpen={menuNonce() === taskNonceId(task)}
              onMenu={() =>
                setMenuNonce(
                  menuNonce() === taskNonceId(task)
                    ? undefined
                    : taskNonceId(task),
                )
              }
              onEdit={() => props.onEdit(task, taskDisplayText(task))}
              onDelete={() => props.onDelete(task)}
              onCreateSession={() => props.onCreateSession(task)}
            />
          )}
        </For>
        <Show when={queuedTasks().length > 0 && timedTasks().length > 0}>
          <div class="composer-task-divider" aria-hidden="true" />
        </Show>
        <For each={timedTasks()}>
          {(task) => (
            <PlanTaskRow
              task={task}
              selected={props.selected_nonce_id === taskNonceId(task)}
              menuOpen={menuNonce() === taskNonceId(task)}
              onMenu={() =>
                setMenuNonce(
                  menuNonce() === taskNonceId(task)
                    ? undefined
                    : taskNonceId(task),
                )
              }
              onEdit={() => props.onEdit(task, taskDisplayText(task))}
              onDelete={() => props.onDelete(task)}
              onCreateSession={() => props.onCreateSession(task)}
            />
          )}
        </For>
      </section>
    </Show>
  );
}

export function PlanConversationFeedbackNotice() {
  return (
    <div class="plan-feedback-prompt">
      <span aria-hidden="true" />
      <p>请输入命令或者反馈</p>
    </div>
  );
}

export function shouldShowPlanFeedbackPrompt(
  session: Session,
  composerText: string,
): boolean {
  const status = planSessionStatus(session);
  if (status === "question" || status === "done") {
    return true;
  }
  return (
    status === "todo" &&
    !firstRunnableTask(session) &&
    composerText.trim().length > 0
  );
}

export function PlanTaskRow(props: {
  task: TaskManagement;
  selected: boolean;
  menuOpen: boolean;
  onMenu: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onCreateSession: () => void;
}) {
  let moreButton: HTMLButtonElement | undefined;
  const [menuRect, setMenuRect] = createSignal({ left: 0, top: 0 });
  const scheduleText = createMemo(() => {
    const startCondition = taskStartCondition(props.task);
    return formatStartCondition(startCondition);
  });
  const remainingText = createMemo(() => formatTaskRemaining(props.task));
  function updateMenuPosition() {
    const rect = moreButton?.getBoundingClientRect();
    if (!rect) {
      return;
    }
    const width = 146;
    const left = Math.max(
      8,
      Math.min(rect.right - width, window.innerWidth - width - 8),
    );
    setMenuRect({ left, top: rect.top - 6 });
  }
  createEffect(() => {
    if (!props.menuOpen) {
      return;
    }
    updateMenuPosition();
    window.addEventListener("resize", updateMenuPosition);
    window.addEventListener("scroll", updateMenuPosition, true);
    onCleanup(() => {
      window.removeEventListener("resize", updateMenuPosition);
      window.removeEventListener("scroll", updateMenuPosition, true);
    });
  });
  return (
    <div class="composer-task-row-wrap">
      <button
        type="button"
        class={classNames("composer-task-row", props.selected && "selected")}
        onClick={props.onEdit}
      >
        <span>{taskSummaryText(props.task)}</span>
        <small
          class={classNames(
            "composer-task-meta",
            remainingText() && "has-countdown",
          )}
        >
          <Show when={scheduleText()}>
            <span class="composer-task-condition">{scheduleText()}</span>
          </Show>
          <Show when={remainingText()}>
            <span class="composer-task-countdown">{remainingText()}</span>
          </Show>
        </small>
      </button>
      <button
        ref={moreButton}
        class="composer-task-more"
        type="button"
        title="更多"
        onClick={(event) => {
          event.stopPropagation();
          updateMenuPosition();
          props.onMenu();
        }}
      >
        <MoreHorizontal size={15} />
      </button>
      <Show when={props.menuOpen}>
        <Portal>
          <div
            class="composer-task-menu"
            style={{
              left: `${menuRect().left}px`,
              top: `${menuRect().top}px`,
            }}
          >
            <button type="button" onClick={props.onDelete}>
              删除
            </button>
            <button type="button" onClick={props.onCreateSession}>
              创建新会话
            </button>
          </div>
        </Portal>
      </Show>
    </div>
  );
}

export function formatPollInterval(interval: PollInterval): string {
  const normalized = normalizePollInterval(interval);
  return (["d", "h", "m", "s"] as const)
    .map((part) => `${normalized[part] ?? 0}${part}`)
    .join(" ");
}

export function PlanDraftSessionPicker(props: {
  sessions: Session[];
  selectedSessionId?: string;
  onSession: (value: string | undefined) => void;
}) {
  let root: HTMLElement | undefined;
  const [open, setOpen] = createSignal(false);
  const [query, setQuery] = createSignal("");
  const selectedSession = createMemo(() =>
    props.selectedSessionId
      ? props.sessions.find((session) => session.id === props.selectedSessionId)
      : undefined,
  );
  const filteredSessions = createMemo(() => {
    const normalized = query().trim().toLowerCase();
    const sessions = props.sessions.filter(
      (session) => planSessionStatus(session) !== "archived",
    );
    if (!normalized) {
      return sessions.slice(0, 8);
    }
    return sessions
      .filter(
        (session) =>
          sessionTitle(session).toLowerCase().includes(normalized) ||
          session.id.toLowerCase().includes(normalized),
      )
      .slice(0, 8);
  });
  createEffect(() => {
    if (!open()) {
      return;
    }
    const closeOutside = (event: PointerEvent) => {
      if (!root?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    onCleanup(() => document.removeEventListener("pointerdown", closeOutside));
  });
  return (
    <section class="plan-session-picker" ref={root}>
      <button
        type="button"
        class="plan-session-button"
        onClick={() => setOpen(!open())}
        title={
          selectedSession() ? sessionTitle(selectedSession()!) : t("newSession")
        }
      >
        <FolderOpen size={15} strokeWidth={1.8} />
        <span>
          {selectedSession()
            ? sessionTitle(selectedSession()!)
            : t("newSession")}
        </span>
        <ChevronDown size={13} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div class="plan-session-menu">
          <label class="workspace-search-row">
            <Search size={14} strokeWidth={1.7} />
            <input
              class="workspace-search"
              value={query()}
              placeholder={`${t("sessionHistory")}...`}
              onInput={(event) => setQuery(event.currentTarget.value)}
            />
          </label>
          <button
            type="button"
            class={classNames(
              "workspace-pick-row",
              "session-pick-row",
              !props.selectedSessionId && "selected",
            )}
            onClick={() => {
              props.onSession(undefined);
              setOpen(false);
            }}
          >
            <Plus size={15} strokeWidth={1.7} />
            <span>{t("newSession")}</span>
            <Show when={!props.selectedSessionId}>
              <Check size={14} strokeWidth={1.8} />
            </Show>
          </button>
          <div class="workspace-picker-list plan-session-list">
            <For each={filteredSessions()}>
              {(session) => (
                <button
                  type="button"
                  class={classNames(
                    "workspace-pick-row",
                    "session-pick-row",
                    props.selectedSessionId === session.id && "selected",
                  )}
                  onClick={() => {
                    props.onSession(session.id);
                    setOpen(false);
                  }}
                  title={sessionTitle(session)}
                >
                  <FolderOpen size={15} strokeWidth={1.6} />
                  <span>{sessionTitle(session)}</span>
                  <Show when={props.selectedSessionId === session.id}>
                    <Check size={14} strokeWidth={1.8} />
                  </Show>
                </button>
              )}
            </For>
          </div>
        </div>
      </Show>
    </section>
  );
}

export function PlanComposerControls(props: {
  startCondition: StartCondition;
  startAt: string;
  pollInterval: PollInterval;
  onStartCondition: (value: StartCondition) => void;
  onStartAt: (value: string) => void;
  onPollInterval: (value: PollInterval) => void;
}) {
  let root: HTMLElement | undefined;
  const [open, setOpen] = createSignal(false);
  const [scheduleOpen, setScheduleOpen] = createSignal(false);
  const startConditions: Array<{ id: StartCondition; label: string }> = [
    { id: "user_action", label: t("runNow") },
    { id: "session_idle", label: t("sessionIdle") },
    { id: "scheduled_task", label: t("scheduledTask") },
    { id: "polling_task", label: t("pollingTask") },
  ];
  const selectedLabel = createMemo(() => {
    return (
      startConditions.find((condition) => condition.id === props.startCondition)
        ?.label ?? t("userAction")
    );
  });
  const selectCondition = (condition: StartCondition) => {
    props.onStartCondition(condition);
    if (
      (condition === "scheduled_task" || condition === "polling_task") &&
      !props.startAt
    ) {
      props.onStartAt(defaultLocalStartAt());
    }
    if (condition === "polling_task") {
      props.onPollInterval(normalizePollInterval(props.pollInterval));
    }
    setOpen(false);
    if (condition === "scheduled_task" || condition === "polling_task") {
      setScheduleOpen(true);
    }
  };
  createEffect(() => {
    if (!open()) {
      return;
    }
    const closeOutside = (event: PointerEvent) => {
      if (!root?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    onCleanup(() => document.removeEventListener("pointerdown", closeOutside));
  });
  return (
    <section class="plan-trigger-control" ref={root}>
      <button
        type="button"
        class="plan-trigger-button"
        onClick={() => setOpen(!open())}
      >
        <CalendarDays size={15} strokeWidth={1.8} />
        <span>{selectedLabel()}</span>
        <ChevronDown size={13} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div class="plan-session-menu plan-trigger-menu">
          <For each={startConditions}>
            {(condition) => (
              <button
                type="button"
                class={classNames(
                  "workspace-pick-row",
                  "no-icon",
                  "plan-trigger-option",
                  props.startCondition === condition.id && "selected",
                )}
                onClick={() => selectCondition(condition.id)}
              >
                <span>{condition.label}</span>
                <Show when={props.startCondition === condition.id}>
                  <Check size={14} strokeWidth={1.8} />
                </Show>
              </button>
            )}
          </For>
        </div>
      </Show>
      <Show when={scheduleOpen()}>
        <PlanScheduleDialog
          condition={props.startCondition}
          startAt={props.startAt || defaultLocalStartAt()}
          pollInterval={normalizePollInterval(props.pollInterval)}
          onCancel={() => setScheduleOpen(false)}
          onSave={(startAt, pollInterval) => {
            props.onStartAt(startAt);
            if (props.startCondition === "polling_task") {
              props.onPollInterval(normalizePollInterval(pollInterval));
            }
            setScheduleOpen(false);
          }}
        />
      </Show>
    </section>
  );
}

export function PlanScheduleDialog(props: {
  condition: StartCondition;
  startAt: string;
  pollInterval: PollInterval;
  onCancel: () => void;
  onSave: (startAt: string, pollInterval: PollInterval) => void;
}) {
  const [startAt, setStartAt] = createSignal(props.startAt);
  const [interval, setInterval] = createSignal(
    normalizePollInterval(props.pollInterval),
  );
  const setIntervalPart = (part: keyof PollInterval, value: string) =>
    setInterval((previous) => ({
      ...previous,
      [part]: normalizeIntervalPart(value.replace(/\D/gu, "")),
    }));
  const blockNonNumericInput = (event: InputEvent) => {
    if (event.data && !/^\d+$/u.test(event.data)) {
      event.preventDefault();
    }
  };
  const intervalParts: Array<{
    id: keyof PollInterval;
    label: TextKey;
    maxLength: number;
  }> = [
    { id: "d", label: "intervalDay", maxLength: 3 },
    { id: "h", label: "intervalHour", maxLength: 2 },
    { id: "m", label: "intervalMinute", maxLength: 2 },
    { id: "s", label: "intervalSecond", maxLength: 2 },
  ];
  return (
    <div class="modal-scrim" onMouseDown={props.onCancel}>
      <div
        class="name-dialog plan-schedule-dialog"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header>
          <div>
            <h2>
              {props.condition === "polling_task"
                ? t("pollingTask")
                : t("scheduledTask")}
            </h2>
          </div>
          <button type="button" onClick={props.onCancel}>
            ×
          </button>
        </header>
        <label class="field-row">
          <span>{t("startTime")}</span>
          <input
            type="datetime-local"
            value={startAt()}
            onInput={(event) => setStartAt(event.currentTarget.value)}
          />
        </label>
        <Show when={props.condition === "polling_task"}>
          <div class="field-row plan-schedule-interval">
            <span>{t("pollInterval")}</span>
            <div class="plan-schedule-interval-grid">
              <For each={intervalParts}>
                {(part) => (
                  <label class={`interval-part-${part.id}`}>
                    <input
                      type="text"
                      inputmode="numeric"
                      pattern="[0-9]*"
                      maxlength={part.maxLength}
                      value={String(interval()[part.id] ?? 0)}
                      onBeforeInput={blockNonNumericInput}
                      onInput={(event) => {
                        const value = event.currentTarget.value
                          .replace(/\D/gu, "")
                          .slice(0, part.maxLength);
                        event.currentTarget.value = value;
                        setIntervalPart(part.id, value);
                      }}
                    />
                    <span>{t(part.label)}</span>
                  </label>
                )}
              </For>
            </div>
          </div>
        </Show>
        <footer>
          <button type="button" class="secondary" onClick={props.onCancel}>
            {t("cancel")}
          </button>
          <button
            type="button"
            class="primary"
            disabled={!startAt()}
            onClick={() => props.onSave(startAt(), interval())}
          >
            {t("save")}
          </button>
        </footer>
      </div>
    </div>
  );
}
