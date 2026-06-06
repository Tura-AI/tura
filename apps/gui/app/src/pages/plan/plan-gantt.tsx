import { type Session, type TaskManagement } from "@tura/gateway-sdk";
import ChevronLeft from "lucide-solid/icons/chevron-left";
import ChevronRight from "lucide-solid/icons/chevron-right";
import { For, Show, createEffect, createMemo, createSignal, onCleanup, type JSX } from "solid-js";
import { t } from "../../i18n";
import { classNames } from "../../state/format";
import { sessionTitle } from "../../state/global-store";

import {
  PlanDragGhost,
  beginPlanPointerDrag,
  dateWithPointerMinutes,
  pointerScheduleFromElement,
  type PlanDragState,
} from "../../features/plan/drag";
import {
  planSessionStatus,
  planTaskTitle,
  planTimedSessions,
  shortSessionId,
  taskNonceId,
  taskPlanStatus,
  taskPollInterval,
  taskStartAt,
  taskStartCondition,
  taskSummaryText,
  timedSessionTasks,
} from "../../features/plan/tasks";
import {
  DAY_MS,
  HOUR_MS,
  formatCalendarWeekTitle,
  formatGanttDayTitle,
  formatGanttMarkBottom,
  formatGanttMarkTop,
  planTimelineMarks,
  startOfDay,
  type PlanGanttMode,
} from "../../features/plan/timeline";

type GanttTaskOccurrence = {
  task: TaskManagement;
  startAt: Date;
  sequence: number;
};

export function PlanGanttView(props: {
  sessions: Session[];
  selectedSessionId?: string;
  selectedTaskNonceId?: string;
  onOpenSession: (session: Session) => void;
  onEditTask: (session: Session, task: TaskManagement) => void;
  onSchedule: (session: Session, task: TaskManagement, startAt: string) => void;
}) {
  function currentTimelineStart(mode: PlanGanttMode): Date {
    const now = new Date();
    if (mode === "day") {
      now.setSeconds(0, 0);
      return now;
    }
    return startOfDay(now);
  }
  const [dragState, setDragState] = createSignal<PlanDragState>();
  const [timelineMode, setTimelineMode] = createSignal<PlanGanttMode>("week");
  const timedSessions = createMemo(() => planTimedSessions(props.sessions));
  const [timelineCursor, setTimelineCursor] = createSignal(currentTimelineStart("week"));
  const [timelineWidth, setTimelineWidth] = createSignal(0);
  const dayHourCount = createMemo(() => {
    const width = timelineWidth();
    if (width <= 0) {
      return 6;
    }
    return Math.max(2, Math.min(12, Math.floor(width / 76)));
  });
  const timelineMarks = createMemo(() =>
    planTimelineMarks(timelineCursor(), timelineMode(), dayHourCount()),
  );
  const timelineTitle = createMemo(() =>
    timelineMode() === "day"
      ? formatGanttDayTitle(timelineMarks())
      : formatCalendarWeekTitle(timelineMarks()),
  );
  const timelineWindowMs = createMemo(() =>
    timelineMode() === "day" ? dayHourCount() * HOUR_MS : 7 * DAY_MS,
  );
  let timelineSessionsKey = "";
  createEffect(() => {
    const key = timedSessions()
      .map(
        (session) =>
          `${session.id}:${timedSessionTasks(session)
            .map((task) => `${taskNonceId(task) ?? ""}:${String(taskStartAt(task) ?? "")}`)
            .join(",")}`,
      )
      .join("|");
    if (key !== timelineSessionsKey) {
      timelineSessionsKey = key;
      setTimelineCursor(currentTimelineStart(timelineMode()));
    }
  });
  const todayPosition = createMemo(() => {
    const marks = timelineMarks();
    const start = marks[0]?.getTime();
    if (!start) {
      return undefined;
    }
    const ratio = (Date.now() - start) / timelineWindowMs();
    return ratio >= 0 && ratio <= 1 ? ratio : undefined;
  });
  let timelineEl: HTMLDivElement | undefined;
  let lastEdgeMoveAt = 0;
  let pixelMinuteRemainder = 0;
  let holdScrollTimer: number | undefined;
  const ganttRows = createMemo(() =>
    timedSessions()
      .map((session) => ({
        session,
        tasks: timedSessionTasks(session),
        occurrences: timedSessionTasks(session)
          .flatMap((task) => taskOccurrences(task))
          .sort((left, right) => left.startAt.getTime() - right.startAt.getTime()),
      }))
      .filter((row) => row.occurrences.length > 0),
  );
  createEffect(() => {
    if (!timelineEl) {
      return;
    }
    const updateWidth = () => setTimelineWidth(timelineTrackWidth());
    updateWidth();
    const observer = new ResizeObserver(updateWidth);
    observer.observe(timelineEl);
    window.addEventListener("resize", updateWidth);
    onCleanup(() => {
      observer.disconnect();
      window.removeEventListener("resize", updateWidth);
    });
  });
  function pollingIntervalMs(task: TaskManagement): number {
    const interval = taskPollInterval(task);
    return (
      (interval.d ?? 0) * DAY_MS +
      (interval.h ?? 0) * HOUR_MS +
      (interval.m ?? 0) * 60_000 +
      (interval.s ?? 0) * 1_000
    );
  }
  function taskOccurrences(task: TaskManagement): GanttTaskOccurrence[] {
    const raw = taskStartAt(task);
    const first = raw ? new Date(raw) : undefined;
    const marks = timelineMarks();
    const windowStart = marks[0]?.getTime();
    if (
      !first ||
      Number.isNaN(first.getTime()) ||
      windowStart === undefined ||
      marks.length === 0
    ) {
      return [];
    }
    const windowEnd = windowStart + timelineWindowMs();
    const intervalMs = taskStartCondition(task) === "polling_task" ? pollingIntervalMs(task) : 0;
    if (intervalMs <= 0) {
      const time = first.getTime();
      return time >= windowStart && time < windowEnd ? [{ task, startAt: first, sequence: 0 }] : [];
    }
    const firstTime = first.getTime();
    const now = Date.now();
    const nextTime =
      firstTime > now
        ? firstTime
        : firstTime + Math.ceil((now - firstTime) / intervalMs) * intervalMs;
    if (nextTime < windowStart || nextTime >= windowEnd) {
      return [];
    }
    return [
      {
        task,
        startAt: new Date(nextTime),
        sequence: Math.max(0, Math.round((nextTime - firstTime) / intervalMs)),
      },
    ];
  }
  function occurrenceTimelineStyle(
    occurrence: GanttTaskOccurrence,
    index: number,
    total: number,
  ): JSX.CSSProperties {
    const marks = timelineMarks();
    const windowStart = marks[0]?.getTime();
    if (windowStart === undefined || marks.length === 0) {
      return { display: "none" };
    }
    const windowEnd = windowStart + timelineWindowMs();
    const time = occurrence.startAt.getTime();
    if (time < windowStart || time >= windowEnd) {
      return { display: "none" };
    }
    const position = ((time - windowStart) / (windowEnd - windowStart)) * 100;
    return {
      left: `${position}%`,
      "--plan-ticket-z": String(Math.max(1, total - index)),
      "--plan-bar-width": "min(160px, calc(100% - 8px))",
    };
  }
  function taskTriggerClass(task: TaskManagement): string {
    return `trigger-${taskStartCondition(task)}`;
  }
  function timelinePointerDate(point: { x: number }): string | undefined {
    const marks = timelineMarks();
    if (!timelineEl || marks.length === 0) {
      return undefined;
    }
    const rect = timelineEl.getBoundingClientRect();
    const axis = timelineEl.querySelector<HTMLElement>(".plan-timeline-left-head");
    const start = axis?.getBoundingClientRect().right ?? rect.left;
    const width = rect.width - (start - rect.left);
    if (width <= 0) {
      return undefined;
    }
    const ratio = Math.max(0, Math.min(1, (point.x - start) / width));
    const windowStart = marks[0]!.getTime();
    const minutes = Math.round((ratio * timelineWindowMs()) / 60_000);
    const next = new Date(windowStart + minutes * 60_000);
    return Number.isNaN(next.getTime()) ? undefined : next.toISOString();
  }
  function dropOnDay(event: DragEvent, day: Date) {
    event.preventDefault();
    const session = props.sessions.find(
      (item) => item.id === event.dataTransfer?.getData("text/session-id"),
    );
    if (session) {
      const task = timedSessionTasks(session)[0];
      if (!task) {
        return;
      }
      const startAt =
        timelinePointerDate({ x: event.clientX }) ??
        dateWithPointerMinutes(day, event.currentTarget as HTMLElement, {
          axis: "x",
          x: event.clientX,
          y: event.clientY,
        }).toISOString();
      props.onSchedule(session, task, startAt);
    }
  }
  function beginGanttTaskDrag(
    event: PointerEvent | MouseEvent,
    session: Session,
    task: TaskManagement,
  ) {
    beginPlanPointerDrag({
      event,
      session,
      setDragState,
      onOpen: () => props.onEditTask(session, task),
      onSchedule: (startAt) => props.onSchedule(session, task, startAt),
      onMove: (point) => scrollTimelineAtEdge(point),
      resolveSchedule: (point) =>
        timelinePointerDate(point) ?? pointerScheduleFromElement(point, "x"),
    });
  }
  function moveTimelineMinutes(minutesDelta: number) {
    setTimelineCursor((cursor) => new Date(cursor.getTime() + minutesDelta * 60_000));
  }
  function moveTimelineWindow(direction: number) {
    moveTimelineMinutes(direction * Math.round(timelineWindowMs() / 60_000 / 30));
  }
  function stopTimelineHold() {
    if (holdScrollTimer !== undefined) {
      window.clearInterval(holdScrollTimer);
      holdScrollTimer = undefined;
    }
    window.removeEventListener("pointerup", stopTimelineHold);
    window.removeEventListener("pointercancel", stopTimelineHold);
    window.removeEventListener("mouseup", stopTimelineHold);
  }
  function beginTimelineHold(event: PointerEvent | MouseEvent, direction: number) {
    event.preventDefault();
    event.stopPropagation();
    stopTimelineHold();
    moveTimelineWindow(direction);
    holdScrollTimer = window.setInterval(() => moveTimelineWindow(direction), 100);
    window.addEventListener("pointerup", stopTimelineHold, { once: true });
    window.addEventListener("pointercancel", stopTimelineHold, { once: true });
    window.addEventListener("mouseup", stopTimelineHold, { once: true });
  }
  function timelineTrackWidth(): number {
    if (!timelineEl) {
      return 0;
    }
    const rect = timelineEl.getBoundingClientRect();
    const axis = timelineEl.querySelector<HTMLElement>(".plan-timeline-left-head");
    const leftWidth = axis?.getBoundingClientRect().width ?? 0;
    return Math.max(0, rect.width - leftWidth);
  }
  function moveTimelineByPixels(deltaX: number) {
    const width = timelineTrackWidth();
    if (width <= 0 || deltaX === 0) {
      return;
    }
    const rawMinutes = (-deltaX / width) * (timelineWindowMs() / 60_000) + pixelMinuteRemainder;
    const minutes = rawMinutes < 0 ? Math.ceil(rawMinutes) : Math.floor(rawMinutes);
    pixelMinuteRemainder = rawMinutes - minutes;
    if (minutes !== 0) {
      moveTimelineMinutes(minutes);
    }
  }
  function wheelTimeline(event: WheelEvent) {
    const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
    if (delta === 0) {
      return;
    }
    event.preventDefault();
    moveTimelineByPixels(delta);
  }
  function switchTimelineMode(mode: PlanGanttMode) {
    setTimelineCursor(currentTimelineStart(mode));
    setTimelineMode(mode);
  }
  function scrollTimelineAtEdge(point: { x: number }) {
    if (!timelineEl) {
      return;
    }
    const gridRect = timelineEl.getBoundingClientRect();
    const trackRect =
      timelineEl.querySelector<HTMLElement>(".plan-timeline-track")?.getBoundingClientRect() ??
      gridRect;
    const edge = 24;
    const now = Date.now();
    if (now - lastEdgeMoveAt < 60) {
      return;
    }
    if (point.x <= trackRect.left + edge) {
      const distance = trackRect.left + edge - point.x;
      const minutes = Math.max(
        1,
        Math.round(
          (Math.max(18, Math.min(48, distance)) / trackRect.width) * (timelineWindowMs() / 60_000),
        ),
      );
      moveTimelineMinutes(-minutes);
      lastEdgeMoveAt = now;
    } else if (point.x >= trackRect.right - edge) {
      const distance = point.x - (trackRect.right - edge);
      const minutes = Math.max(
        1,
        Math.round(
          (Math.max(18, Math.min(48, distance)) / trackRect.width) * (timelineWindowMs() / 60_000),
        ),
      );
      moveTimelineMinutes(minutes);
      lastEdgeMoveAt = now;
    } else {
      pixelMinuteRemainder = 0;
    }
  }
  function beginTimelinePan(event: PointerEvent | MouseEvent) {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    let lastX = event.clientX;
    const onMove = (move: PointerEvent | MouseEvent) => {
      const delta = move.clientX - lastX;
      moveTimelineByPixels(delta);
      lastX = move.clientX;
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp, { once: true });
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp, { once: true });
  }
  return (
    <section class="plan-gantt">
      <PlanDragGhost state={dragState()} />
      <div
        ref={timelineEl}
        class="plan-timeline-grid"
        style={{ "--plan-days": String(timelineMarks().length) }}
        onWheel={wheelTimeline}
      >
        <div
          class="plan-timeline-scale"
          onPointerDown={beginTimelinePan}
          onMouseDown={beginTimelinePan}
        >
          <header class="plan-calendar-title plan-timeline-title">
            <div class="plan-calendar-nav">
              <button
                class="icon-action"
                type="button"
                title={t("previous")}
                onPointerDown={(event) => beginTimelineHold(event, -1)}
                onMouseDown={(event) => beginTimelineHold(event, -1)}
              >
                <ChevronLeft size={16} />
              </button>
              <strong>{timelineTitle()}</strong>
              <button
                class="icon-action"
                type="button"
                title={t("next")}
                onPointerDown={(event) => beginTimelineHold(event, 1)}
                onMouseDown={(event) => beginTimelineHold(event, 1)}
              >
                <ChevronRight size={16} />
              </button>
            </div>
            <div class="plan-calendar-view-toggle plan-gantt-view-toggle">
              <button
                type="button"
                class={classNames(timelineMode() === "week" && "selected")}
                onPointerDown={(event) => event.stopPropagation()}
                onMouseDown={(event) => event.stopPropagation()}
                onClick={() => switchTimelineMode("week")}
              >
                {t("week")}
              </button>
              <button
                type="button"
                class={classNames(timelineMode() === "day" && "selected")}
                onPointerDown={(event) => event.stopPropagation()}
                onMouseDown={(event) => event.stopPropagation()}
                onClick={() => switchTimelineMode("day")}
              >
                {t("day")}
              </button>
            </div>
          </header>
          <span class="plan-timeline-left-head" aria-hidden="true"></span>
          <For each={timelineMarks()}>
            {(mark, index) => (
              <span
                style={{
                  "grid-column": String(index() + 2),
                }}
                class="plan-timeline-day"
                data-plan-timeline-day={mark.toISOString()}
              >
                <small>{formatGanttMarkTop(mark, timelineMode())}</small>
                <strong>{formatGanttMarkBottom(mark, timelineMode())}</strong>
              </span>
            )}
          </For>
        </div>
        <Show when={todayPosition() !== undefined}>
          <i class="plan-today-line" style={{ "--today": String(todayPosition()) }} />
        </Show>
        <For each={ganttRows()}>
          {(row) => {
            const session = row.session;
            return (
              <div
                class="plan-timeline-row"
                style={{
                  "--task-count": String(row.tasks.length),
                }}
              >
                <button
                  type="button"
                  class={classNames(
                    "plan-timeline-session",
                    props.selectedSessionId === session.id &&
                      !props.selectedTaskNonceId &&
                      "selected",
                  )}
                  onClick={() => props.onOpenSession(session)}
                  title={sessionTitle(session)}
                >
                  <strong>{sessionTitle(session)}</strong>
                  <small>{shortSessionId(session.id)}</small>
                </button>
                <div class="plan-timeline-track">
                  <For each={row.occurrences}>
                    {(occurrence, index) => (
                      <button
                        class={classNames(
                          "plan-timeline-bar",
                          `status-${taskPlanStatus(occurrence.task) ?? planSessionStatus(session)}`,
                          taskTriggerClass(occurrence.task),
                          props.selectedSessionId === session.id &&
                            props.selectedTaskNonceId === taskNonceId(occurrence.task) &&
                            "selected",
                        )}
                        style={occurrenceTimelineStyle(occurrence, index(), row.occurrences.length)}
                        onPointerDown={(event) =>
                          beginGanttTaskDrag(event, session, occurrence.task)
                        }
                        onMouseDown={(event) => beginGanttTaskDrag(event, session, occurrence.task)}
                        onClick={(event) => event.preventDefault()}
                        title={[
                          taskSummaryText(occurrence.task) || planTaskTitle(session),
                          sessionTitle(session),
                        ].join("\n")}
                        data-task-nonce={taskNonceId(occurrence.task)}
                        data-task-occurrence={occurrence.startAt.toISOString()}
                      >
                        <strong>
                          {taskSummaryText(occurrence.task) || planTaskTitle(session)}
                        </strong>
                      </button>
                    )}
                  </For>
                  <For each={timelineMarks()}>
                    {(day) => (
                      <button
                        class="plan-timeline-drop"
                        type="button"
                        title={day.toLocaleDateString()}
                        onDragOver={(event) => event.preventDefault()}
                        onDrop={(event) => dropOnDay(event, day)}
                        data-plan-timeline-day={day.toISOString()}
                      />
                    )}
                  </For>
                </div>
              </div>
            );
          }}
        </For>
      </div>
    </section>
  );
}
