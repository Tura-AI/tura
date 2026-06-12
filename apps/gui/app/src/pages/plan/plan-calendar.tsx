import { type Session, type TaskManagement } from "@tura/gateway-sdk";
import ChevronLeft from "lucide-solid/icons/chevron-left";
import ChevronRight from "lucide-solid/icons/chevron-right";
import { For, Show, createMemo, createSignal, onCleanup } from "solid-js";
import { t } from "../../i18n";
import { classNames } from "../../state/format";
import { sessionTitle } from "../../state/global-store";

import {
  PlanDragGhost,
  beginPlanPointerDrag,
  dateWithPointerMinutes,
  pointerRatio,
  pointerScheduleFromElement,
  type PlanDragState,
} from "../../features/plan/drag";
import {
  formatCalendarEventTime,
  planInitialCalendarDate,
  planSessionStatus,
  planTimedSessions,
  taskNonceId,
  taskPlanStatus,
  taskStartCondition,
  taskSummaryText,
  timedSessionTasks,
  timedTaskDisplayDate,
  type PlanCalendarMode,
} from "../../features/plan/tasks";
import {
  DAY_MS,
  calendarGridDays,
  calendarWeekDays,
  formatCalendarWeekTitle,
  hourStartIso,
  sameCalendarDay,
  startOfDay,
} from "../../features/plan/timeline";
export function PlanCalendarView(props: {
  sessions: Session[];
  selectedSessionId?: string;
  onOpenSession: (session: Session) => void;
  onCreateAt: (startAt: string) => void;
  onSchedule: (session: Session, task: TaskManagement, startAt: string) => void;
}) {
  const [dragState, setDragState] = createSignal<PlanDragState>();
  const [nowMs, setNowMs] = createSignal(Date.now());
  const refreshTimer = window.setInterval(() => setNowMs(Date.now()), 30_000);
  onCleanup(() => window.clearInterval(refreshTimer));
  const timedSessions = createMemo(() => planTimedSessions(props.sessions));
  const timedOccurrences = createMemo(() =>
    timedSessions()
      .flatMap((session) =>
        timedSessionTasks(session).flatMap((task) => {
          const date = timedTaskDisplayDate(task, nowMs());
          return date ? [{ session, task, date }] : [];
        }),
      )
      .sort((left, right) => left.date.getTime() - right.date.getTime()),
  );
  const [calendarView, setCalendarView] = createSignal<PlanCalendarMode>("month");
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
  function isPastCalendarDay(day: Date): boolean {
    return startOfDay(day).getTime() < startOfDay(new Date()).getTime();
  }
  function isPastCalendarHour(day: Date, hour: number): boolean {
    const hourEnd = new Date(day);
    hourEnd.setHours(hour + 1, 0, 0, 0);
    return hourEnd.getTime() <= Date.now();
  }
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
  function occurrencesForDay(day: Date): PlanCalendarOccurrence[] {
    return timedOccurrences().filter((occurrence) => {
      return sameCalendarDay(occurrence.date, day);
    });
  }
  function occurrencesForDayHour(day: Date, hour: number): PlanCalendarOccurrence[] {
    return occurrencesForDay(day).filter((occurrence) => {
      return occurrence.date.getHours() === hour;
    });
  }
  function dropOnDay(event: DragEvent, day: Date) {
    event.preventDefault();
    const session = timedSessions().find(
      (item) => item.id === event.dataTransfer?.getData("text/session-id"),
    );
    const task = session ? timedSessionTasks(session)[0] : undefined;
    if (session && task) {
      props.onSchedule(
        session,
        task,
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
    const task = session ? timedSessionTasks(session)[0] : undefined;
    if (session && task) {
      const next = new Date(day);
      const minuteRatio = pointerRatio(event.currentTarget as HTMLElement, event.clientY, "y");
      next.setHours(hour, Math.round(minuteRatio * 59), 0, 0);
      props.onSchedule(session, task, next.toISOString());
    }
  }
  function beginCalendarDrag(event: PointerEvent | MouseEvent, occurrence: PlanCalendarOccurrence) {
    beginPlanPointerDrag({
      event,
      session: occurrence.session,
      setDragState,
      onOpen: () => props.onOpenSession(occurrence.session),
      onSchedule: (startAt) => props.onSchedule(occurrence.session, occurrence.task, startAt),
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
  function openDayFromBlank(event: MouseEvent, day: Date) {
    if ((event.target as HTMLElement).closest(".plan-calendar-event")) {
      return;
    }
    setCalendarCursor(day);
    setCalendarView("day");
  }
  function createDraftFromWeek(event: MouseEvent, day: Date, hour: number) {
    if ((event.target as HTMLElement).closest(".plan-calendar-event")) {
      return;
    }
    const start = new Date(day);
    start.setHours(
      hour,
      Math.round(pointerRatio(event.currentTarget as HTMLElement, event.clientY, "y") * 59),
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
    setCalendarCursor(new Date(cursor.getFullYear(), cursor.getMonth() + amount, 1));
  }
  function switchCalendarView(view: PlanCalendarMode) {
    setCalendarCursor(startOfDay(new Date()));
    setCalendarView(view);
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
            onClick={() => switchCalendarView("month")}
          >
            月
          </button>
          <button
            type="button"
            class={classNames(calendarView() === "week" && "selected")}
            onClick={() => switchCalendarView("week")}
          >
            周
          </button>
          <button
            type="button"
            class={classNames(calendarView() === "day" && "selected")}
            onClick={() => switchCalendarView("day")}
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
                      isPastCalendarDay(day) && "past",
                      sameCalendarDay(day, new Date()) && "today",
                    )}
                    onClick={(event) => openDayFromBlank(event, day)}
                    onDragOver={(event) => event.preventDefault()}
                    onDrop={(event) => dropOnDay(event, day)}
                    data-plan-day={day.toISOString()}
                  >
                    <header>
                      <span>{day.getDate()}</span>
                    </header>
                    <For each={occurrencesForDay(day)}>
                      {(occurrence) => (
                        <PlanCalendarEvent
                          occurrence={occurrence}
                          selected={props.selectedSessionId === occurrence.session.id}
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
          class={classNames("plan-calendar-week", calendarView() === "day" && "day-mode")}
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
                    isPastCalendarDay(day) && "past",
                    sameCalendarDay(day, new Date()) && "today",
                    sameCalendarDay(day, calendarCursor()) && "selected",
                  )}
                  onClick={() => setCalendarCursor(day)}
                  onDblClick={() => setCalendarView("day")}
                >
                  <small>{day.toLocaleDateString(undefined, { weekday: "short" })}</small>
                  <strong>{day.getDate()}</strong>
                </button>
              )}
            </For>
          </div>
          <div class="plan-calendar-week-grid" ref={hourGridEl}>
            <For each={weekHours}>
              {(hour) => (
                <>
                  <span class="plan-calendar-hour-label">{String(hour).padStart(2, "0")}:00</span>
                  <For each={activeHourDays()}>
                    {(day) => (
                      <section
                        class={classNames(
                          "plan-calendar-hour-cell",
                          isPastCalendarHour(day, hour) && "past",
                        )}
                        onClick={(event) => createDraftFromWeek(event, day, hour)}
                        onDragOver={(event) => event.preventDefault()}
                        onDrop={(event) => dropOnDayHour(event, day, hour)}
                        data-plan-hour-start={hourStartIso(day, hour)}
                      >
                        <For each={occurrencesForDayHour(day, hour)}>
                          {(occurrence) => (
                            <PlanCalendarEvent
                              occurrence={occurrence}
                              selected={props.selectedSessionId === occurrence.session.id}
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

type PlanCalendarOccurrence = {
  session: Session;
  task: TaskManagement;
  date: Date;
};

export function PlanCalendarEvent(props: {
  occurrence: PlanCalendarOccurrence;
  selected?: boolean;
  onOpenSession: (session: Session) => void;
  onPointerDragStart: (
    event: PointerEvent | MouseEvent,
    occurrence: PlanCalendarOccurrence,
  ) => void;
}) {
  return (
    <button
      class={classNames(
        "plan-calendar-event",
        `status-${taskPlanStatus(props.occurrence.task) ?? planSessionStatus(props.occurrence.session)}`,
        `trigger-${taskStartCondition(props.occurrence.task)}`,
        props.selected && "selected",
      )}
      type="button"
      onPointerDown={(event) => props.onPointerDragStart(event, props.occurrence)}
      onMouseDown={(event) => props.onPointerDragStart(event, props.occurrence)}
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
      }}
      title={[taskSummaryText(props.occurrence.task), sessionTitle(props.occurrence.session)]
        .filter(Boolean)
        .join("\n")}
      data-task-nonce={taskNonceId(props.occurrence.task)}
      data-task-occurrence={props.occurrence.date.toISOString()}
    >
      <span>
        {taskSummaryText(props.occurrence.task) || sessionTitle(props.occurrence.session)}
      </span>
      <small>{formatCalendarEventTime(props.occurrence.date.toISOString())}</small>
    </button>
  );
}
