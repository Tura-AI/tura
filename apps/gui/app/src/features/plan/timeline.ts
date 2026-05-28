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
  sessionUpdatedAt,
  sessionTitle,
  type AppState,
  type SettingsSection,
  type ThemeMode,
} from "../../state/global-store";
import { classNames, truncate } from "../../state/format";
import { t, type TextKey } from "../../i18n";
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
  return Array.from(
    { length: count },
    (_, index) => new Date(start.getTime() + index * DAY_MS),
  );
}

export function planTimelineStart(sessions: Session[]): Date {
  return planTimelineDays(sessions, 1)[0] ?? startOfDay(new Date());
}

export function planTimelineWindow(anchor: Date, count: number): Date[] {
  const start = new Date(anchor);
  return Array.from(
    { length: count },
    (_, index) => new Date(start.getTime() + index * DAY_MS),
  );
}

export type PlanGanttMode = "week" | "day";

export function planTimelineMarks(
  anchor: Date,
  mode: PlanGanttMode,
  dayHourCount = 6,
): Date[] {
  const start = new Date(new Date(anchor).setSeconds(0, 0));
  const count = mode === "day" ? dayHourCount : 7;
  const step = mode === "day" ? HOUR_MS : DAY_MS;
  return Array.from(
    { length: count },
    (_, index) => new Date(start.getTime() + index * step),
  );
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
    new Date(
      monthStart.getFullYear(),
      monthStart.getMonth(),
      1 - monthStart.getDay(),
    ),
  );
  return Array.from(
    { length: 42 },
    (_, index) => new Date(start.getTime() + index * DAY_MS),
  );
}

export function calendarWeekDays(anchor: Date): Date[] {
  const start = startOfDay(
    new Date(
      anchor.getFullYear(),
      anchor.getMonth(),
      anchor.getDate() - anchor.getDay(),
    ),
  );
  return Array.from(
    { length: 7 },
    (_, index) => new Date(start.getTime() + index * DAY_MS),
  );
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
