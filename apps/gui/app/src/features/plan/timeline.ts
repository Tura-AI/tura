import { type Session } from "@tura/gateway-sdk";
import {
  isTimedStartCondition,
  sessionTaskState,
  sessionTasks,
  taskStartAt,
  taskStartCondition,
} from "./tasks";
export const HOUR_MS = 3_600_000;
export const DAY_MS = 86_400_000;

export function startOfDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

export function planSessionDate(session: Session): Date | undefined {
  const raw = sessionTaskState(session).start_at;
  const fallback = sessionTasks(session)
    .filter((task) => isTimedStartCondition(taskStartCondition(task)))
    .map((task) => taskStartAt(task))
    .find(Boolean);
  const date = raw ? new Date(raw) : fallback ? new Date(fallback) : undefined;
  return date && !Number.isNaN(date.getTime()) ? date : undefined;
}

export function planTimelineDays(sessions: Session[], count: number): Date[] {
  const first = sessions.map(planSessionDate).find(Boolean) ?? new Date();
  const start = startOfDay(new Date(first.getTime() - 2 * DAY_MS));
  return Array.from({ length: count }, (_, index) => new Date(start.getTime() + index * DAY_MS));
}

export function planTimelineStart(sessions: Session[]): Date {
  return planTimelineDays(sessions, 1)[0] ?? startOfDay(new Date());
}

export function planTimelineWindow(anchor: Date, count: number): Date[] {
  const start = new Date(anchor);
  return Array.from({ length: count }, (_, index) => new Date(start.getTime() + index * DAY_MS));
}

export type PlanGanttMode = "week" | "day";

export function planTimelineMarks(anchor: Date, mode: PlanGanttMode, dayHourCount = 6): Date[] {
  const start = new Date(new Date(anchor).setSeconds(0, 0));
  const count = mode === "day" ? dayHourCount : 7;
  const step = mode === "day" ? HOUR_MS : DAY_MS;
  return Array.from({ length: count }, (_, index) => new Date(start.getTime() + index * step));
}

export function formatGanttDayTitle(days: Date[]): string {
  const first = days[0];
  const last = days[days.length - 1];
  if (!first || !last) {
    return "";
  }
  const end = new Date(last.getTime() + HOUR_MS);
  const date = first.toLocaleDateString(undefined, {
    month: "long",
    day: "numeric",
    year: "numeric",
  });
  const time = (value: Date) =>
    value.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
    });
  return `${date} ${time(first)} - ${time(end)}`;
}

export function formatGanttMarkTop(date: Date, mode: PlanGanttMode): string {
  if (mode === "day") {
    if (date.getHours() !== 0) {
      return "";
    }
    return date.toLocaleDateString(undefined, {
      month: "numeric",
      day: "numeric",
    });
  }
  return date.toLocaleDateString(undefined, { weekday: "short" });
}

export function formatGanttMarkBottom(date: Date, mode: PlanGanttMode): string {
  if (mode === "day") {
    return date.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
    });
  }
  return date.toLocaleDateString(undefined, {
    month: "numeric",
    day: "numeric",
  });
}

export function planTimelineWeeks(days: Date[]): Array<{
  label: string;
  start: number;
  span: number;
}> {
  const weeks: Array<{ label: string; start: number; span: number }> = [];
  for (const [index, day] of days.entries()) {
    const week = calendarWeekDays(day);
    const label = `${week[0]!.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    })} - ${week[6]!.toLocaleDateString(undefined, { day: "numeric" })}`;
    const last = weeks[weeks.length - 1];
    if (last?.label === label) {
      last.span += 1;
    } else {
      weeks.push({ label, start: index, span: 1 });
    }
  }
  return weeks;
}

export function calendarGridDays(monthStart: Date): Date[] {
  const start = startOfDay(
    new Date(monthStart.getFullYear(), monthStart.getMonth(), 1 - monthStart.getDay()),
  );
  return Array.from({ length: 42 }, (_, index) => new Date(start.getTime() + index * DAY_MS));
}

export function calendarWeekDays(anchor: Date): Date[] {
  const start = startOfDay(
    new Date(anchor.getFullYear(), anchor.getMonth(), anchor.getDate() - anchor.getDay()),
  );
  return Array.from({ length: 7 }, (_, index) => new Date(start.getTime() + index * DAY_MS));
}

export function hourStartIso(day: Date, hour: number): string {
  const start = new Date(day);
  start.setHours(hour, 0, 0, 0);
  return start.toISOString();
}

export function formatCalendarWeekTitle(days: Date[]): string {
  const first = days[0];
  const last = days[days.length - 1];
  if (!first || !last) {
    return "";
  }
  const format = (date: Date) =>
    date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  return `${format(first)} - ${format(last)}`;
}

export function sameCalendarDay(left: Date, right: Date): boolean {
  return startOfDay(left).getTime() === startOfDay(right).getTime();
}
