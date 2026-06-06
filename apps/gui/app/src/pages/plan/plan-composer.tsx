import {
  type PollInterval,
  type Session,
  type StartCondition,
  type TaskManagement,
} from "@tura/gateway-sdk";
import CalendarClock from "lucide-solid/icons/calendar-clock";
import CalendarDays from "lucide-solid/icons/calendar-days";
import ChartGantt from "lucide-solid/icons/chart-gantt";
import Check from "lucide-solid/icons/check";
import ChevronDown from "lucide-solid/icons/chevron-down";
import Columns3 from "lucide-solid/icons/columns-3";
import MoreHorizontal from "lucide-solid/icons/ellipsis";
import GripVertical from "lucide-solid/icons/grip-vertical";
import LayoutList from "lucide-solid/icons/layout-list";
import Play from "lucide-solid/icons/play";
import Plus from "lucide-solid/icons/plus";
import Repeat2 from "lucide-solid/icons/repeat-2";
import ScrollText from "lucide-solid/icons/scroll-text";
import Search from "lucide-solid/icons/search";
import Timer from "lucide-solid/icons/timer";
import { For, Show, createEffect, createMemo, createSignal, onCleanup, type JSX } from "solid-js";
import { Dynamic, Portal } from "solid-js/web";
import { t, type TextKey } from "../../i18n";
import { classNames } from "../../state/format";
import { sessionTitle, type PlanMode } from "../../state/global-store";

import {
  defaultLocalStartAt,
  firstRunnableTask,
  formatPollIntervalEveryCompact,
  formatPollingTaskTiming,
  formatStartCondition,
  formatTaskRemaining,
  formatTicketTime,
  isTimedStartCondition,
  localDateTimeToUtcIso,
  normalizeIntervalPart,
  normalizePollInterval,
  planSessionStatus,
  sessionTaskState,
  sortedSessionTasks,
  taskDisplayText,
  taskNonceId,
  taskPlanStatus,
  taskStartCondition,
  taskSummaryText,
} from "../../features/plan/tasks";
import {
  beginComposerTaskPointerDrag,
  type ComposerTaskDragState,
  type TaskDropIndicator,
} from "./plan-composer-drag";
import { TaskRowText } from "./plan-task-row-text";
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
                (mode.id === "split" ? props.splitOpen : props.mode === mode.id) && "selected",
              )}
              title={mode.label}
              onClick={() => (mode.id === "split" ? props.onSplit() : props.onMode(mode.id))}
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
  const label = createMemo(() =>
    condition() === "user_action" ? t("sessionIdle") : formatStartCondition(condition()),
  );
  return (
    <div class="ticket-meta">
      <span>{label()}</span>
      <Show when={isTimedStartCondition(condition())}>
        <span>{formatTicketTime(task().start_at)}</span>
      </Show>
    </div>
  );
}

export function PlanComposerTaskList(props: {
  session: Session;
  selected_task_id?: string;
  pulseNonceId?: string;
  pulseToken?: number;
  onEdit: (task: TaskManagement, value: string) => void;
  onDelete: (task: TaskManagement) => void;
  onRun: (task: TaskManagement) => void;
  onCreateSession: (task: TaskManagement) => void;
  onReorder: (tasks: TaskManagement[]) => void;
}) {
  const [menuNonce, setMenuNonce] = createSignal<string>();
  const [dragNonce, setDragNonce] = createSignal<string>();
  const [dragGhost, setDragGhost] = createSignal<ComposerTaskDragState>();
  const [dropIndicator, setDropIndicator] = createSignal<TaskDropIndicator>();
  const [suppressedEditNonce, setSuppressedEditNonce] = createSignal<string>();
  const tasks = createMemo(() => sortedSessionTasks(props.session));
  function reorderTaskTo(
    sourceNonce: string,
    targetNonce: string,
    edge: TaskDropIndicator["edge"],
  ) {
    if (sourceNonce === targetNonce) {
      return;
    }
    const current = tasks();
    const sourceIndex = current.findIndex((task) => taskNonceId(task) === sourceNonce);
    const targetIndex = current.findIndex((task) => taskNonceId(task) === targetNonce);
    if (sourceIndex < 0 || targetIndex < 0) {
      return;
    }
    const next = [...current];
    const [source] = next.splice(sourceIndex, 1);
    const targetIndexAfterRemoval = sourceIndex < targetIndex ? targetIndex - 1 : targetIndex;
    const insertIndex = edge === "after" ? targetIndexAfterRemoval + 1 : targetIndexAfterRemoval;
    next.splice(insertIndex, 0, source);
    props.onReorder(next);
  }
  function finishDrag(sourceNonce: string, moved: boolean) {
    setDragNonce(undefined);
    setDragGhost(undefined);
    setDropIndicator(undefined);
    if (moved) {
      setSuppressedEditNonce(sourceNonce);
      window.setTimeout(() => setSuppressedEditNonce(undefined), 0);
    }
  }
  createEffect(() => {
    if (!menuNonce()) {
      return;
    }
    const closeMenu = (event: PointerEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest(".composer-task-menu") || target?.closest(".composer-task-more")) {
        return;
      }
      setMenuNonce(undefined);
    };
    document.addEventListener("pointerdown", closeMenu);
    onCleanup(() => document.removeEventListener("pointerdown", closeMenu));
  });
  return (
    <Show when={tasks().length > 0}>
      <section class="composer-task-list" aria-label={t("taskManagement")}>
        <Show when={dragGhost()}>
          {(ghost) => (
            <div
              class="plan-drag-ghost composer-task-drag-ghost"
              style={{
                left: `${ghost().x}px`,
                top: `${ghost().y}px`,
                width: `${ghost().width}px`,
                height: `${ghost().height}px`,
              }}
              innerHTML={ghost().html}
              aria-hidden="true"
            />
          )}
        </Show>
        <For each={tasks()}>
          {(task, index) => {
            const rowKey = () => taskNonceId(task) ?? `task:${index()}`;
            const indicator = () =>
              dropIndicator()?.nonce === taskNonceId(task) ? dropIndicator()?.edge : undefined;
            return (
              <PlanTaskRow
                pulseId={rowKey()}
                task={task}
                dragging={dragNonce() === taskNonceId(task)}
                dropIndicator={indicator()}
                pulseToken={props.pulseNonceId === taskNonceId(task) ? props.pulseToken : undefined}
                selected={Boolean(
                  props.selected_task_id && props.selected_task_id === taskNonceId(task),
                )}
                menuOpen={menuNonce() === rowKey()}
                onMenu={() => setMenuNonce(menuNonce() === rowKey() ? undefined : rowKey())}
                onEdit={() => {
                  if (suppressedEditNonce() === taskNonceId(task)) {
                    return;
                  }
                  props.onEdit(task, taskDisplayText(task));
                }}
                onDelete={() => {
                  setMenuNonce(undefined);
                  props.onDelete(task);
                }}
                onRun={() => {
                  setMenuNonce(undefined);
                  props.onRun(task);
                }}
                onCreateSession={() => {
                  setMenuNonce(undefined);
                  props.onCreateSession(task);
                }}
                onPointerDragStart={(event) => {
                  const nonce = taskNonceId(task);
                  if (!nonce) {
                    return;
                  }
                  beginComposerTaskPointerDrag({
                    event,
                    sourceNonce: nonce,
                    setDragNonce,
                    setDragGhost,
                    setDropIndicator,
                    onDrop: (drop) => reorderTaskTo(nonce, drop.nonce, drop.edge),
                    onFinish: (moved) => finishDrag(nonce, moved),
                  });
                }}
              />
            );
          }}
        </For>
      </section>
    </Show>
  );
}

export function PlanConversationFeedbackNotice(props: {
  message?: string;
  code?: string;
  providerId?: string;
  onOpenProviderSettings?: (providerId?: string) => void;
}) {
  return (
    <div class={classNames("plan-feedback-prompt", props.code && "error")}>
      <span aria-hidden="true" />
      <p>
        {props.message ?? "请输入命令或者反馈"}
        <Show when={props.code}>{(code) => <small>{code()}</small>}</Show>
        <Show when={props.providerId}>
          {(providerId) => (
            <button
              type="button"
              class="plan-feedback-provider-link"
              onClick={() => props.onOpenProviderSettings?.(providerId())}
            >
              查看供应商
            </button>
          )}
        </Show>
      </p>
    </div>
  );
}

export function shouldShowPlanFeedbackPrompt(session: Session, composerText: string): boolean {
  const status = planSessionStatus(session);
  if (status === "question" || status === "done") {
    return true;
  }
  return status === "todo" && !firstRunnableTask(session) && composerText.trim().length > 0;
}

const taskRowPulseSignatureCache = new Map<string, string>();
export function PlanTaskRow(props: {
  pulseId: string;
  task: TaskManagement;
  dragging?: boolean;
  dropIndicator?: TaskDropIndicator["edge"];
  pulseToken?: number;
  selected: boolean;
  menuOpen: boolean;
  onMenu: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onRun: () => void;
  onCreateSession: () => void;
  onPointerDragStart: (event: PointerEvent) => void;
}) {
  let moreButton: HTMLButtonElement | undefined;
  const [menuRect, setMenuRect] = createSignal({ left: 0, top: 0 });
  const [textPulse, setTextPulse] = createSignal(false);
  let textPulseTimer: number | undefined;
  let lastPulseToken: number | undefined;
  const summaryText = createMemo(() => taskSummaryText(props.task));
  const status = createMemo(() => taskPlanStatus(props.task));
  const activeStatus = createMemo(() => {
    const value = status();
    return value === "doing" || value === "question" ? value : undefined;
  });
  const taskPulseSignature = createMemo(() => `${summaryText()}\n${taskDisplayText(props.task)}`);
  const scheduleText = createMemo(() => {
    const startCondition = taskStartCondition(props.task);
    return startCondition === "user_action"
      ? t("sessionIdle")
      : formatStartCondition(startCondition);
  });
  const remainingText = createMemo(() => {
    if (taskStartCondition(props.task) !== "polling_task") {
      return formatTaskRemaining(props.task);
    }
    return formatPollingTaskTiming(props.task);
  });
  function updateMenuPosition() {
    const rect = moreButton?.getBoundingClientRect();
    if (!rect) {
      return;
    }
    const width = 146;
    const left = Math.max(8, Math.min(rect.right - width, window.innerWidth - width - 8));
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
  createEffect(() => {
    taskRowPulseSignatureCache.set(props.pulseId, taskPulseSignature());
  });
  createEffect(() => {
    const token = props.pulseToken;
    if (token === undefined || token === lastPulseToken) {
      return;
    }
    lastPulseToken = token;
    setTextPulse(false);
    if (textPulseTimer) {
      window.clearTimeout(textPulseTimer);
    }
    requestAnimationFrame(() => setTextPulse(true));
    textPulseTimer = window.setTimeout(() => setTextPulse(false), 700);
  });
  onCleanup(() => {
    if (textPulseTimer) {
      window.clearTimeout(textPulseTimer);
    }
  });
  return (
    <div
      class={classNames(
        "composer-task-row-wrap",
        props.dragging && "dragging",
        props.dropIndicator === "before" && "drop-before",
        props.dropIndicator === "after" && "drop-after",
      )}
      data-task-nonce={taskNonceId(props.task)}
      onPointerDown={props.onPointerDragStart}
    >
      <span class="composer-task-drag-handle" aria-hidden="true">
        <GripVertical size={14} />
      </span>
      <button
        type="button"
        class={classNames("composer-task-row", props.selected && "selected")}
        onClick={props.onEdit}
      >
        <span
          class={classNames(
            "composer-task-title",
            activeStatus() && "has-status",
            textPulse() && "task-text-pulse",
          )}
        >
          <Show when={activeStatus()}>
            {(status) => (
              <span
                class={classNames(
                  "plan-status-indicator",
                  "composer-task-status",
                  `status-${status()}`,
                )}
                aria-hidden="true"
              />
            )}
          </Show>
          <span class="composer-task-title-text">
            <TaskRowText text={summaryText()} />
          </span>
        </span>
        <small class={classNames("composer-task-meta", remainingText() && "has-countdown")}>
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
            <button type="button" onClick={props.onRun}>
              {t("runNow")}
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
  return (["d", "h", "m", "s"] as const).map((part) => `${normalized[part] ?? 0}${part}`).join(" ");
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
    const sessions = props.sessions.filter((session) => planSessionStatus(session) !== "archived");
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
        title={selectedSession() ? sessionTitle(selectedSession()!) : t("newSession")}
      >
        <ScrollText size={15} strokeWidth={1.8} />
        <span>{selectedSession() ? sessionTitle(selectedSession()!) : t("newSession")}</span>
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
                  <ScrollText size={15} strokeWidth={1.6} />
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
  const [scheduleCondition, setScheduleCondition] = createSignal<StartCondition>();
  const [scheduleDefaultStartAt, setScheduleDefaultStartAt] = createSignal(defaultLocalStartAt());
  const startConditions: Array<{
    id: StartCondition;
    label: string;
    icon: (props: { size?: number; strokeWidth?: number }) => JSX.Element;
  }> = [
    { id: "user_action", label: t("runNow"), icon: Play },
    { id: "session_idle", label: t("sessionIdle"), icon: Timer },
    { id: "scheduled_task", label: t("scheduledTask"), icon: CalendarClock },
    { id: "polling_task", label: t("pollingTask"), icon: Repeat2 },
  ];
  const selectedCondition = createMemo(
    () =>
      startConditions.find((condition) => condition.id === props.startCondition) ??
      startConditions[0]!,
  );
  const selectedLabel = createMemo(() => {
    return selectedCondition().label;
  });
  const SelectedIcon = createMemo(() => selectedCondition().icon);
  const conditionRemainingText = (condition: StartCondition) =>
    formatTaskRemaining({
      start_at: localDateTimeToUtcIso(props.startAt || scheduleDefaultStartAt()),
      poll_interval: condition === "polling_task" ? props.pollInterval : {},
    });
  const conditionMetaText = (condition: StartCondition) => {
    if (condition === "scheduled_task") {
      return conditionRemainingText(condition);
    }
    if (condition === "polling_task") {
      const remaining = conditionRemainingText(condition);
      if (!remaining) {
        return "";
      }
      return `${remaining}/${formatPollIntervalEveryCompact(props.pollInterval)}`;
    }
    return "";
  };
  const selectCondition = (condition: StartCondition) => {
    const openedDefaultStartAt = defaultLocalStartAt();
    setScheduleDefaultStartAt(openedDefaultStartAt);
    props.onStartCondition(condition);
    if ((condition === "scheduled_task" || condition === "polling_task") && !props.startAt) {
      props.onStartAt(openedDefaultStartAt);
    }
    if (condition === "polling_task") {
      props.onPollInterval(normalizePollInterval(props.pollInterval));
    }
    setOpen(false);
    if (condition === "scheduled_task" || condition === "polling_task") {
      setScheduleCondition(condition);
      setScheduleOpen(true);
    }
  };
  const closeSchedule = () => {
    setScheduleOpen(false);
    setScheduleCondition(undefined);
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
        onClick={() => {
          if (!open()) {
            setScheduleDefaultStartAt(defaultLocalStartAt());
          }
          setOpen(!open());
        }}
      >
        <Dynamic component={SelectedIcon()} size={15} strokeWidth={1.8} />
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
                  "plan-trigger-option",
                  props.startCondition === condition.id && "selected",
                )}
                onClick={() => selectCondition(condition.id)}
              >
                <Dynamic component={condition.icon} size={15} strokeWidth={1.7} />
                <span>{condition.label}</span>
                <Show when={conditionMetaText(condition.id)}>
                  {(meta) => <small>{meta()}</small>}
                </Show>
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
          condition={scheduleCondition() ?? props.startCondition}
          startAt={props.startAt || scheduleDefaultStartAt()}
          pollInterval={normalizePollInterval(props.pollInterval)}
          onCancel={closeSchedule}
          onSave={(startAt, pollInterval) => {
            const condition = scheduleCondition() ?? props.startCondition;
            props.onStartCondition(condition);
            props.onStartAt(startAt);
            if (condition === "polling_task") {
              props.onPollInterval(normalizePollInterval(pollInterval));
            }
            closeSchedule();
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
  const initialIntervalMinutes =
    normalizeIntervalPart(props.pollInterval.d) * 1440 +
    normalizeIntervalPart(props.pollInterval.h) * 60 +
    normalizeIntervalPart(props.pollInterval.m) +
    Math.ceil(normalizeIntervalPart(props.pollInterval.s) / 60);
  const [startAt, setStartAt] = createSignal(props.startAt);
  const [interval, setInterval] = createSignal<PollInterval>({
    d: Math.floor(initialIntervalMinutes / 1440),
    h: Math.floor((initialIntervalMinutes % 1440) / 60),
    m: initialIntervalMinutes % 60,
  });
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
  ];
  const dateValue = createMemo(() => startAt().slice(0, 10));
  const timeValue = createMemo(() => startAt().slice(11, 16));
  const setDatePart = (value: string) => {
    if (!value) {
      return;
    }
    setStartAt(`${value}T${timeValue() || "00:00"}`);
  };
  const setTimePart = (value: string) => {
    if (!value) {
      return;
    }
    setStartAt(`${dateValue() || defaultLocalStartAt().slice(0, 10)}T${value}`);
  };
  const quickTimes = createMemo(() => {
    const now = new Date();
    const items: Array<{ label: string; value: string }> = [
      { label: "1小时后", value: localDateTimeFromDate(addMinutes(now, 60)) },
      { label: "今晚", value: localDateTimeFromDate(atLocalTime(now, 20, 0)) },
      {
        label: "明早",
        value: localDateTimeFromDate(atLocalTime(addMinutes(now, 24 * 60), 9, 0)),
      },
    ];
    return items;
  });
  return (
    <Portal>
      <div class="modal-scrim" onMouseDown={props.onCancel}>
        <div
          class="name-dialog plan-schedule-dialog"
          onMouseDown={(event) => event.stopPropagation()}
        >
          <header>
            <div>
              <h2>{props.condition === "polling_task" ? t("pollingTask") : t("scheduledTask")}</h2>
            </div>
            <button type="button" onClick={props.onCancel}>
              ×
            </button>
          </header>
          <div class="plan-schedule-picker">
            <span>{t("startTime")}</span>
            <div class="plan-schedule-datetime">
              <label>
                <CalendarDays size={15} strokeWidth={1.7} />
                <input
                  type="date"
                  value={dateValue()}
                  onInput={(event) => setDatePart(event.currentTarget.value)}
                />
              </label>
              <label>
                <Timer size={15} strokeWidth={1.7} />
                <input
                  type="time"
                  value={timeValue()}
                  onInput={(event) => setTimePart(event.currentTarget.value)}
                />
              </label>
            </div>
            <div class="plan-schedule-presets">
              <For each={quickTimes()}>
                {(item) => (
                  <button type="button" onClick={() => setStartAt(item.value)}>
                    {item.label}
                  </button>
                )}
              </For>
            </div>
          </div>
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
    </Portal>
  );
}

function addMinutes(date: Date, minutes: number): Date {
  const next = new Date(date.getTime() + minutes * 60_000);
  next.setSeconds(0, 0);
  return next;
}

function atLocalTime(date: Date, hours: number, minutes: number): Date {
  const next = new Date(date);
  next.setHours(hours, minutes, 0, 0);
  return next;
}

function localDateTimeFromDate(date: Date): string {
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
}
