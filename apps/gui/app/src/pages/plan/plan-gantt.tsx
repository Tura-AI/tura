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
  pointerScheduleFromElement,
  type PlanDragState,
} from "../../features/plan/drag";
import {
  DAY_MS,
  HOUR_MS,
  formatCalendarWeekTitle,
  formatGanttDayTitle,
  formatGanttMarkBottom,
  formatGanttMarkTop,
  planSessionDate,
  planTimelineDays,
  planTimelineMarks,
  planTimelineStart,
  planTimelineWeeks,
  type PlanGanttMode,
} from "../../features/plan/timeline";
import {
  planSessionStatus,
  planTaskTitle,
  planTimedSessions,
  planTriggerClass,
  sessionTaskState,
  shortSessionId,
  taskSummaryText,
  timedSessionTasks,
  timedTaskPatch,
} from "../../features/plan/tasks";
export function PlanGanttView(props: {
  sessions: Session[];
  onOpenSession: (session: Session) => void;
  onSchedule: (session: Session, startAt: string) => void;
}) {
  const [dragState, setDragState] = createSignal<PlanDragState>();
  const [timelineMode, setTimelineMode] = createSignal<PlanGanttMode>("week");
  const timedSessions = createMemo(() => planTimedSessions(props.sessions));
  const [timelineCursor, setTimelineCursor] = createSignal(
    planTimelineStart(timedSessions()),
  );
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
          `${session.id}:${planSessionDate(session)?.toISOString() ?? ""}`,
      )
      .join("|");
    if (key !== timelineSessionsKey) {
      timelineSessionsKey = key;
      setTimelineCursor(planTimelineStart(timedSessions()));
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
      .map((session) => ({ session, tasks: timedSessionTasks(session) }))
      .filter((row) => row.tasks.length > 0),
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
  function sessionTimelineStyle(session: Session): JSX.CSSProperties {
    const date = planSessionDate(session);
    const marks = timelineMarks();
    if (!date || marks.length === 0) {
      return { display: "none" };
    }
    const windowStart = marks[0]!.getTime();
    const windowEnd = windowStart + timelineWindowMs();
    const time = date.getTime();
    if (time < windowStart || time >= windowEnd) {
      return { display: "none" };
    }
    const position = ((time - windowStart) / (windowEnd - windowStart)) * 100;
    return {
      left: `${position}%`,
      "--plan-bar-width": "min(160px, calc(100% - 8px))",
    };
  }
  function timelinePointerDate(point: { x: number }): string | undefined {
    const marks = timelineMarks();
    if (!timelineEl || marks.length === 0) {
      return undefined;
    }
    const rect = timelineEl.getBoundingClientRect();
    const axis = timelineEl.querySelector<HTMLElement>(
      ".plan-timeline-left-head",
    );
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
      const startAt =
        timelinePointerDate({ x: event.clientX }) ??
        dateWithPointerMinutes(day, event.currentTarget as HTMLElement, {
          axis: "x",
          x: event.clientX,
          y: event.clientY,
        }).toISOString();
      props.onSchedule(session, startAt);
    }
  }
  function beginGanttDrag(event: PointerEvent | MouseEvent, session: Session) {
    beginPlanPointerDrag({
      event,
      session,
      setDragState,
      onOpen: () => props.onOpenSession(session),
      onSchedule: (startAt) => props.onSchedule(session, startAt),
      onMove: (point) => scrollTimelineAtEdge(point),
      resolveSchedule: (point) =>
        timelinePointerDate(point) ?? pointerScheduleFromElement(point, "x"),
    });
  }
  function moveTimelineMinutes(minutesDelta: number) {
    setTimelineCursor(
      (cursor) => new Date(cursor.getTime() + minutesDelta * 60_000),
    );
  }
  function moveTimelineWindow(direction: number) {
    moveTimelineMinutes(
      direction * Math.round(timelineWindowMs() / 60_000 / 30),
    );
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
  function beginTimelineHold(
    event: PointerEvent | MouseEvent,
    direction: number,
  ) {
    event.preventDefault();
    event.stopPropagation();
    stopTimelineHold();
    moveTimelineWindow(direction);
    holdScrollTimer = window.setInterval(
      () => moveTimelineWindow(direction),
      100,
    );
    window.addEventListener("pointerup", stopTimelineHold, { once: true });
    window.addEventListener("pointercancel", stopTimelineHold, { once: true });
    window.addEventListener("mouseup", stopTimelineHold, { once: true });
  }
  function timelineTrackWidth(): number {
    if (!timelineEl) {
      return 0;
    }
    const rect = timelineEl.getBoundingClientRect();
    const axis = timelineEl.querySelector<HTMLElement>(
      ".plan-timeline-left-head",
    );
    const leftWidth = axis?.getBoundingClientRect().width ?? 0;
    return Math.max(0, rect.width - leftWidth);
  }
  function moveTimelineByPixels(deltaX: number) {
    const width = timelineTrackWidth();
    if (width <= 0 || deltaX === 0) {
      return;
    }
    const rawMinutes =
      (-deltaX / width) * (timelineWindowMs() / 60_000) + pixelMinuteRemainder;
    const minutes =
      rawMinutes < 0 ? Math.ceil(rawMinutes) : Math.floor(rawMinutes);
    pixelMinuteRemainder = rawMinutes - minutes;
    if (minutes !== 0) {
      moveTimelineMinutes(minutes);
    }
  }
  function wheelTimeline(event: WheelEvent) {
    const delta =
      Math.abs(event.deltaX) > Math.abs(event.deltaY)
        ? event.deltaX
        : event.deltaY;
    if (delta === 0) {
      return;
    }
    event.preventDefault();
    moveTimelineByPixels(delta);
  }
  function scrollTimelineAtEdge(point: { x: number }) {
    if (!timelineEl) {
      return;
    }
    const rect = timelineEl.getBoundingClientRect();
    const edge = 56;
    const now = Date.now();
    if (now - lastEdgeMoveAt < 60) {
      return;
    }
    if (point.x < rect.left + edge) {
      moveTimelineByPixels(-(rect.left + edge - point.x));
      lastEdgeMoveAt = now;
    } else if (point.x > rect.right - edge) {
      moveTimelineByPixels(point.x - (rect.right - edge));
      lastEdgeMoveAt = now;
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
                onClick={() => setTimelineMode("week")}
              >
                {t("week")}
              </button>
              <button
                type="button"
                class={classNames(timelineMode() === "day" && "selected")}
                onPointerDown={(event) => event.stopPropagation()}
                onMouseDown={(event) => event.stopPropagation()}
                onClick={() => setTimelineMode("day")}
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
          <i
            class="plan-today-line"
            style={{ "--today": String(todayPosition()) }}
          />
        </Show>
        <For each={ganttRows()}>
          {(row) => {
            const session = row.session;
            const topTask = () => row.tasks[0] ?? sessionTaskState(session);
            const barStyle = createMemo(() => sessionTimelineStyle(session));
            return (
              <div class="plan-timeline-row">
                <span>
                  <strong>{sessionTitle(session)}</strong>
                  <small>{shortSessionId(session.id)}</small>
                </span>
                <div class="plan-timeline-track">
                  <For each={row.tasks.slice(1, 4)}>
                    {(_, index) => (
                      <i
                        class="plan-timeline-stack-card"
                        style={{
                          ...barStyle(),
                          "--plan-stack-offset": `${(index() + 1) * 4}px`,
                        }}
                      />
                    )}
                  </For>
                  <button
                    class={classNames(
                      "plan-timeline-bar",
                      `status-${planSessionStatus(session)}`,
                      planTriggerClass(session),
                    )}
                    style={barStyle()}
                    onPointerDown={(event) => beginGanttDrag(event, session)}
                    onMouseDown={(event) => beginGanttDrag(event, session)}
                    onClick={(event) => event.preventDefault()}
                    title={sessionTitle(session)}
                  >
                    <strong>
                      {taskSummaryText(topTask()) || planTaskTitle(session)}
                    </strong>
                  </button>
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
