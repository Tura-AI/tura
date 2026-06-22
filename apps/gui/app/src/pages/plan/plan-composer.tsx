import {
  type PollInterval,
  type Session,
  type StartCondition,
} from "@tura/gateway-sdk";
import CalendarClock from "lucide-solid/icons/calendar-clock";
import CalendarDays from "lucide-solid/icons/calendar-days";
import Check from "lucide-solid/icons/check";
import ChevronDown from "lucide-solid/icons/chevron-down";
import Columns3 from "lucide-solid/icons/columns-3";
import GitBranch from "lucide-solid/icons/git-branch";
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
  formatStartCondition,
  formatTaskRemaining,
  formatTicketTime,
  isTimedStartCondition,
  localDateTimeToUtcIso,
  normalizeIntervalPart,
  normalizePollInterval,
  planSessionStatus,
  sessionTaskState,
  taskStartCondition,
} from "../../features/plan/tasks";
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
    { id: "gantt", label: t("gantt"), icon: GitBranch },
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
              data-plan-mode={mode.id}
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
