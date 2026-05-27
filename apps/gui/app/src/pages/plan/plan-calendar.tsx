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
  PlanDragGhost,
  beginPlanPointerDrag,
  dateWithPointerMinutes,
  pointerRatio,
  pointerScheduleFromElement,
  type PlanDragState,
} from "../../features/plan/drag";
import {
  DAY_MS,
  calendarGridDays,
  calendarWeekDays,
  formatCalendarWeekTitle,
  hourStartIso,
  planSessionDate,
  sameCalendarDay,
  startOfDay,
} from "../../features/plan/timeline";
import {
  formatCalendarEventTime,
  planInitialCalendarDate,
  planSessionStatus,
  planTaskTitle,
  planTimedSessions,
  planTriggerClass,
  sessionTaskState,
  timedTaskPatch,
  type PlanCalendarMode,
} from "../../features/plan/tasks";
export function PlanCalendarView(props: {
  sessions: Session[];
  onOpenSession: (session: Session) => void;
  onCreateAt: (startAt: string) => void;
  onSchedule: (session: Session, startAt: string) => void;
}) {
  const [dragState, setDragState] = createSignal<PlanDragState>();
  const timedSessions = createMemo(() => planTimedSessions(props.sessions));
  const [calendarView, setCalendarView] =
    createSignal<PlanCalendarMode>("month");
  const [calendarCursor, setCalendarCursor] = createSignal(
    planInitialCalendarDate(timedSessions()),
  );
  const monthStart = createMemo(() => {
    const cursor = calendarCursor();
    return new Date(cursor.getFullYear(), cursor.getMonth(), 1);
  });
  const days = createMemo(() => calendarGridDays(monthStart()));
  const weekDays = createMemo(() => calendarWeekDays(calendarCursor()));
  const activeHourDays = createMemo(() =>
    calendarView() === "day" ? [startOfDay(calendarCursor())] : weekDays(),
  );
  const weekHours = Array.from({ length: 24 }, (_, index) => index);
  const calendarTitle = createMemo(() =>
    calendarView() === "day"
      ? calendarCursor().toLocaleDateString(undefined, {
          month: "long",
          day: "numeric",
          year: "numeric",
        })
      : calendarView() === "week"
        ? formatCalendarWeekTitle(weekDays())
        : monthStart().toLocaleDateString(undefined, {
            month: "long",
            year: "numeric",
          }),
  );
  let hourGridEl: HTMLDivElement | undefined;
  function sessionsForDay(day: Date): Session[] {
    return timedSessions().filter((session) => {
      const date = planSessionDate(session);
      return date ? sameCalendarDay(date, day) : false;
    });
  }
  function sessionsForDayHour(day: Date, hour: number): Session[] {
    return sessionsForDay(day).filter((session) => {
      const date = planSessionDate(session);
      return date ? date.getHours() === hour : false;
    });
  }
  function dropOnDay(event: DragEvent, day: Date) {
    event.preventDefault();
    const session = timedSessions().find(
      (item) => item.id === event.dataTransfer?.getData("text/session-id"),
    );
    if (session) {
      props.onSchedule(
        session,
        dateWithPointerMinutes(day, event.currentTarget as HTMLElement, {
          axis: "y",
          x: event.clientX,
          y: event.clientY,
        }).toISOString(),
      );
    }
  }
  function dropOnDayHour(event: DragEvent, day: Date, hour: number) {
    event.preventDefault();
    const session = timedSessions().find(
      (item) => item.id === event.dataTransfer?.getData("text/session-id"),
    );
    if (session) {
      const next = new Date(day);
      const minuteRatio = pointerRatio(
        event.currentTarget as HTMLElement,
        event.clientY,
        "y",
      );
      next.setHours(hour, Math.round(minuteRatio * 59), 0, 0);
      props.onSchedule(session, next.toISOString());
    }
  }
  function beginCalendarDrag(
    event: PointerEvent | MouseEvent,
    session: Session,
  ) {
    beginPlanPointerDrag({
      event,
      session,
      setDragState,
      onOpen: () => props.onOpenSession(session),
      onSchedule: (startAt) => props.onSchedule(session, startAt),
      onMove: (point) => scrollCalendarAtEdge(point),
      resolveSchedule: (point) => pointerScheduleFromElement(point, "y"),
    });
  }
  function scrollCalendarAtEdge(point: { x: number; y: number }) {
    if (!hourGridEl) {
      return;
    }
    const rect = hourGridEl.getBoundingClientRect();
    const edge = 58;
    const topDistance = point.y - rect.top;
    const bottomDistance = rect.bottom - point.y;
    if (topDistance < edge) {
      hourGridEl.scrollTop -= Math.max(1, edge - topDistance) * 0.38;
    } else if (bottomDistance < edge) {
      hourGridEl.scrollTop += Math.max(1, edge - bottomDistance) * 0.38;
    }
  }
  function openWeekFromBlank(event: MouseEvent, day: Date) {
    if ((event.target as HTMLElement).closest(".plan-calendar-event")) {
      return;
    }
    setCalendarCursor(day);
    setCalendarView("week");
  }
  function createDraftFromWeek(event: MouseEvent, day: Date, hour: number) {
    if ((event.target as HTMLElement).closest(".plan-calendar-event")) {
      return;
    }
    const start = new Date(day);
    start.setHours(
      hour,
      Math.round(
        pointerRatio(event.currentTarget as HTMLElement, event.clientY, "y") *
          59,
      ),
      0,
      0,
    );
    props.onCreateAt(start.toISOString());
  }
  function moveCalendar(amount: number) {
    const cursor = calendarCursor();
    if (calendarView() === "day") {
      setCalendarCursor(new Date(cursor.getTime() + amount * DAY_MS));
      return;
    }
    if (calendarView() === "week") {
      setCalendarCursor(new Date(cursor.getTime() + amount * 7 * DAY_MS));
      return;
    }
    setCalendarCursor(
      new Date(cursor.getFullYear(), cursor.getMonth() + amount, 1),
    );
  }
  return (
    <section class="plan-calendar">
      <PlanDragGhost state={dragState()} />
      <header class="plan-calendar-title">
        <div class="plan-calendar-nav">
          <button
            class="icon-action"
            type="button"
            title={t("previous")}
            onClick={() => moveCalendar(-1)}
          >
            <ChevronLeft size={16} />
          </button>
          <strong>{calendarTitle()}</strong>
          <button
            class="icon-action"
            type="button"
            title={t("next")}
            onClick={() => moveCalendar(1)}
          >
            <ChevronRight size={16} />
          </button>
        </div>
        <div class="plan-calendar-view-toggle">
          <button
            type="button"
            class={classNames(calendarView() === "month" && "selected")}
            onClick={() => setCalendarView("month")}
          >
            月
          </button>
          <button
            type="button"
            class={classNames(calendarView() === "week" && "selected")}
            onClick={() => setCalendarView("week")}
          >
            周
          </button>
          <button
            type="button"
            class={classNames(calendarView() === "day" && "selected")}
            onClick={() => setCalendarView("day")}
          >
            日
          </button>
        </div>
      </header>
      <Show
        when={calendarView() !== "month"}
        fallback={
          <>
            <div class="plan-calendar-weekdays">
              <For each={["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]}>
                {(day) => <span>{day}</span>}
              </For>
            </div>
            <div class="plan-calendar-grid">
              <For each={days()}>
                {(day) => (
                  <section
                    class={classNames(
                      "plan-calendar-cell",
                      day.getMonth() !== monthStart().getMonth() && "muted",
                      sameCalendarDay(day, new Date()) && "today",
                    )}
                    onClick={(event) => openWeekFromBlank(event, day)}
                    onDragOver={(event) => event.preventDefault()}
                    onDrop={(event) => dropOnDay(event, day)}
                    data-plan-day={day.toISOString()}
                  >
                    <header>
                      <span>{day.getDate()}</span>
                    </header>
                    <For each={sessionsForDay(day)}>
                      {(session) => (
                        <PlanCalendarEvent
                          session={session}
                          onOpenSession={props.onOpenSession}
                          onPointerDragStart={beginCalendarDrag}
                        />
                      )}
                    </For>
                  </section>
                )}
              </For>
            </div>
          </>
        }
      >
        <div
          class={classNames(
            "plan-calendar-week",
            calendarView() === "day" && "day-mode",
          )}
          style={{ "--calendar-days": String(activeHourDays().length) }}
        >
          <div class="plan-calendar-week-head">
            <span />
            <For each={activeHourDays()}>
              {(day) => (
                <button
                  type="button"
                  class={classNames(
                    "plan-calendar-week-day",
                    sameCalendarDay(day, new Date()) && "today",
                    sameCalendarDay(day, calendarCursor()) && "selected",
                  )}
                  onClick={() => setCalendarCursor(day)}
                  onDblClick={() => setCalendarView("day")}
                >
                  <small>
                    {day.toLocaleDateString(undefined, { weekday: "short" })}
                  </small>
                  <strong>{day.getDate()}</strong>
                </button>
              )}
            </For>
          </div>
          <div class="plan-calendar-week-grid" ref={hourGridEl}>
            <For each={weekHours}>
              {(hour) => (
                <>
                  <span class="plan-calendar-hour-label">
                    {String(hour).padStart(2, "0")}:00
                  </span>
                  <For each={activeHourDays()}>
                    {(day) => (
                      <section
                        class="plan-calendar-hour-cell"
                        onClick={(event) =>
                          createDraftFromWeek(event, day, hour)
                        }
                        onDragOver={(event) => event.preventDefault()}
                        onDrop={(event) => dropOnDayHour(event, day, hour)}
                        data-plan-hour-start={hourStartIso(day, hour)}
                      >
                        <For each={sessionsForDayHour(day, hour)}>
                          {(session) => (
                            <PlanCalendarEvent
                              session={session}
                              onOpenSession={props.onOpenSession}
                              onPointerDragStart={beginCalendarDrag}
                            />
                          )}
                        </For>
                      </section>
                    )}
                  </For>
                </>
              )}
            </For>
          </div>
        </div>
      </Show>
    </section>
  );
}

export function PlanCalendarEvent(props: {
  session: Session;
  onOpenSession: (session: Session) => void;
  onPointerDragStart: (
    event: PointerEvent | MouseEvent,
    session: Session,
  ) => void;
}) {
  return (
    <button
      class={classNames(
        "plan-calendar-event",
        `status-${planSessionStatus(props.session)}`,
        planTriggerClass(props.session),
      )}
      type="button"
      onPointerDown={(event) => props.onPointerDragStart(event, props.session)}
      onMouseDown={(event) => props.onPointerDragStart(event, props.session)}
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
      }}
    >
      <span>{sessionTitle(props.session)}</span>
      <small>
        {formatCalendarEventTime(sessionTaskState(props.session).start_at)}
      </small>
    </button>
  );
}
