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
import type { ComposerImage } from "../../state/global-store";
import { applyGatewayEvent } from "../../state/event-reducer";
import {
  activeSession,
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
import { nextPollingTime, normalizeTimeMs } from "../../utils/app-format";
import { planSessionDate } from "./timeline";

export function taskStateLabel(status: PlanStatus): string {
  switch (status) {
    case "doing":
      return t("doing");
    case "question":
      return t("question");
    case "done":
      return t("done");
    case "archived":
      return t("archived");
    case "todo":
    default:
      return t("todo");
  }
}

export function sessionTaskState(session: Session) {
  return session.task_management ?? {};
}

export function sessionTasks(session: Session): TaskManagement[] {
  const task = sessionTaskState(session);
  if (Array.isArray(task.tasks) && task.tasks.length > 0) {
    return task.tasks;
  }
  return [task];
}

export function sortedSessionTasks(session: Session): TaskManagement[] {
  const visible = sessionTasks(session).filter(
    (task) =>
      taskPlanStatus(task) !== "archived" && taskHasVisibleContent(task),
  );
  const queued = visible
    .filter((task) => !isTimedStartCondition(taskStartCondition(task)))
    .sort(compareTaskStep);
  const timed = visible
    .filter((task) => isTimedStartCondition(taskStartCondition(task)))
    .sort((left, right) => {
      const leftTime = new Date(taskStartAt(left) ?? 0).getTime();
      const rightTime = new Date(taskStartAt(right) ?? 0).getTime();
      return leftTime - rightTime || compareTaskStep(left, right);
    });
  return [...queued, ...timed];
}

export function hasVisibleSessionTasks(session: Session): boolean {
  return sessionTasks(session).some(
    (task) =>
      taskPlanStatus(task) !== "archived" && taskHasVisibleContent(task),
  );
}

export function taskHasVisibleContent(task: TaskManagement): boolean {
  return taskDisplayText(task).trim().length > 0;
}

export function compareTaskStep(
  left: TaskManagement,
  right: TaskManagement,
): number {
  const leftStep =
    typeof left.step === "number" ? left.step : Number.POSITIVE_INFINITY;
  const rightStep =
    typeof right.step === "number" ? right.step : Number.POSITIVE_INFINITY;
  return leftStep - rightStep;
}

export function taskNonceId(task: TaskManagement): string | undefined {
  return task.nonce_id;
}

export function taskPlanStatus(task: TaskManagement): PlanStatus | undefined {
  return task.status;
}

export function taskStartCondition(task: TaskManagement): StartCondition {
  if (task.start_condition) {
    return task.start_condition;
  }
  const status = task.status as string | undefined;
  if (isStartConditionStatus(status)) {
    return status;
  }
  if (hasPollInterval(task.poll_interval)) {
    return "polling_task";
  }
  return task.start_at ? "scheduled_task" : "user_action";
}

function isStartConditionStatus(
  value: string | undefined,
): value is StartCondition {
  return (
    value === "session_idle" ||
    value === "scheduled_task" ||
    value === "polling_task" ||
    value === "user_action"
  );
}

export function hasPollInterval(value: PollInterval | undefined): boolean {
  return Boolean(value && (value.m || value.d || value.h || value.s));
}

export function taskStartAt(task: TaskManagement): string | number | undefined {
  return task.start_at;
}

export function taskPollInterval(task: TaskManagement): PollInterval {
  return task.poll_interval ?? defaultPollInterval();
}

export function taskDisplayText(task: TaskManagement): string {
  const summary = (task.task_summary ?? "").trim();
  const delivery = (task.delivery ?? "").trim();
  return [summary, delivery].filter(Boolean).join("\n\n");
}

export function firstRunnableTask(
  session: Session,
): TaskManagement | undefined {
  return sortedSessionTasks(session).find((task) =>
    taskDisplayText(task).trim(),
  );
}

export function taskSummaryText(task: TaskManagement): string {
  return (
    (task.task_summary ?? task.delivery ?? "")
      .trim()
      .split(/\r?\n/u)[0]
      ?.trim() ?? ""
  );
}

export function formatTaskRemaining(task: TaskManagement): string {
  const condition = taskStartCondition(task);
  if (!isTimedStartCondition(condition)) {
    return "";
  }
  const startAt =
    condition === "polling_task"
      ? nextPollingTime(taskStartAt(task), taskPollInterval(task))
      : taskStartAt(task);
  if (!startAt) {
    return "";
  }
  const target = new Date(startAt).getTime();
  if (Number.isNaN(target)) {
    return "";
  }
  const seconds = Math.max(0, Math.ceil((target - Date.now()) / 1000));
  if (seconds >= 86_400) {
    return `${Math.ceil(seconds / 86_400)}${t("intervalDay")}`;
  }
  if (seconds >= 3_600) {
    return `${Math.ceil(seconds / 3_600)}${t("intervalHour")}`;
  }
  if (seconds >= 60) {
    return `${Math.ceil(seconds / 60)}${t("intervalMinute")}`;
  }
  return `${seconds}${t("intervalSecond")}`;
}

export function formatPollIntervalCompact(interval: PollInterval): string {
  const normalized = normalizePollInterval(interval);
  const minutes =
    (normalized.d ?? 0) * 1440 +
    (normalized.h ?? 0) * 60 +
    (normalized.m ?? 0) +
    Math.ceil((normalized.s ?? 0) / 60);
  if (minutes >= 1440) {
    return `${Math.ceil(minutes / 1440)}${t("intervalDay")}`;
  }
  if (minutes >= 60) {
    return `${Math.ceil(minutes / 60)}${t("intervalHour")}`;
  }
  return `${Math.max(1, minutes)}${t("intervalMinute")}`;
}

export function formatPollIntervalEveryCompact(interval: PollInterval): string {
  return `(${t("intervalEvery", {
    interval: formatPollIntervalCompact(interval),
  })})`;
}

export function formatPollingTaskTiming(task: TaskManagement): string {
  const remaining = formatTaskRemaining(task);
  if (!remaining) {
    return "";
  }
  return `${remaining}/${formatPollIntervalEveryCompact(taskPollInterval(task))}`;
}

export function applyTaskPatchToSession(
  session: Session,
  patch: Partial<TaskManagement>,
): Session {
  const current = sessionTaskState(session);
  const nonce = patch.nonce_id;
  if (Array.isArray(current.tasks)) {
    const tasks = sessionTasks(session);
    const index = nonce
      ? tasks.findIndex((task) => taskNonceId(task) === nonce)
      : tasks.length > 0
        ? 0
        : -1;
    const nextTasks =
      index >= 0
        ? tasks.map((task, itemIndex) =>
            itemIndex === index ? { ...task, ...patch } : task,
          )
        : [
            ...tasks,
            { ...patch, nonce_id: nonce ?? `${session.id}:${tasks.length}` },
          ];
    const nextManagement = { ...current, tasks: nextTasks };
    return {
      ...session,
      task_management: nextManagement,
    };
  }
  if (nonce) {
    const nextManagement = { ...current, ...patch };
    return {
      ...session,
      task_management: nextManagement,
    };
  }
  const nextManagement = { ...current, ...patch };
  return {
    ...session,
    task_management: nextManagement,
  };
}

export function appendTaskToSession(
  session: Session,
  task: Partial<TaskManagement>,
): Session {
  const current = sessionTaskState(session);
  const existingTasks = Array.isArray(current.tasks)
    ? sessionTasks(session)
    : taskHasVisibleContent(current)
      ? [current]
      : [];
  const nonce = task.nonce_id ?? `${session.id}:${existingTasks.length}`;
  const nextTask = {
    ...task,
    nonce_id: nonce,
    step: typeof task.step === "number" ? task.step : existingTasks.length,
  };
  const index = existingTasks.findIndex((item) => taskNonceId(item) === nonce);
  const nextTasks =
    index >= 0
      ? existingTasks.map((item, itemIndex) =>
          itemIndex === index ? { ...item, ...nextTask } : item,
        )
      : [...existingTasks, nextTask];
  return {
    ...session,
    task_management: {
      ...current,
      tasks: nextTasks,
    },
  };
}

export function planSessionStatus(session: Session): PlanStatus {
  const task = sessionTaskState(session);
  const status = task.status;
  if (
    status === "archived" ||
    status === "done" ||
    status === "question" ||
    status === "doing"
  ) {
    return status;
  }
  if (session.status === "busy") {
    return "doing";
  }
  if (status === "todo") {
    return "todo";
  }
  return "todo";
}

export function sessionAttentionKey(session: Session): string | undefined {
  const status = planSessionStatus(session);
  if (status !== "doing" && status !== "question" && status !== "done") {
    return undefined;
  }
  return `${session.id}:${status}:${normalizeTimeMs(sessionUpdatedAt(session) ?? 0)}`;
}

export function planStoredPlanStatus(session: Session): PlanStatus | undefined {
  const task = sessionTaskState(session);
  const status = task.status;
  if (
    status === "todo" ||
    status === "doing" ||
    status === "question" ||
    status === "done" ||
    status === "archived"
  ) {
    return status;
  }
  return undefined;
}

export type PlanCalendarMode = "month" | "week" | "day";

export function planSessionStartCondition(
  session: Session,
): StartCondition | undefined {
  const task = sessionTaskState(session);
  return taskStartCondition(task);
}

export function planTimedSessions(sessions: Session[]): Session[] {
  return sessions.filter((session) => {
    if (
      planStoredPlanStatus(session) !== "todo" &&
      timedSessionTasks(session).length === 0
    ) {
      return false;
    }
    const condition = planSessionStartCondition(session);
    return (
      (Boolean(planSessionDate(session)) &&
        (condition === "scheduled_task" || condition === "polling_task")) ||
      timedSessionTasks(session).length > 0
    );
  });
}

export function timedSessionTasks(session: Session): TaskManagement[] {
  return sortedSessionTasks(session).filter(
    (task) =>
      (taskPlanStatus(task) ?? planStoredPlanStatus(session)) === "todo" &&
      isTimedStartCondition(taskStartCondition(task)) &&
      Boolean(taskStartAt(task)),
  );
}

export function planTriggerClass(session: Session): string {
  const condition = planSessionStartCondition(session);
  return condition ? `trigger-${condition}` : "";
}

export function planTaskTitle(session: Session): string {
  const task = sessionTaskState(session);
  const title = task.task_summary ?? sessionTitle(session);
  return title.replace(/^执行(?:状态|任务)：/u, "");
}

export function planInitialCalendarDate(sessions: Session[]): Date {
  return sessions.map(planSessionDate).find(Boolean) ?? new Date();
}

export function shortSessionId(sessionId: string): string {
  return sessionId.slice(0, 8);
}

export function localDateTimeToUtcIso(value: string): string | undefined {
  if (!value) {
    return undefined;
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? undefined : date.toISOString();
}

export function utcIsoToLocalDateTime(
  value: string | number | undefined,
): string {
  if (!value) {
    return "";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
}

export function defaultLocalStartAt(): string {
  const date = new Date(Date.now() + 60 * 60_000);
  date.setSeconds(0, 0);
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
}

export function defaultPollInterval(): PollInterval {
  return { m: 0, d: 0, h: 1, s: 0 };
}

export function emptyPollInterval(): PollInterval {
  return { m: 0, d: 0, h: 0, s: 0 };
}

export function normalizeIntervalPart(
  value: string | number | undefined,
): number {
  const parsed = Number(value ?? 0);
  return Number.isFinite(parsed) && parsed > 0 ? Math.floor(parsed) : 0;
}

export function normalizePollInterval(
  value: PollInterval | undefined,
): PollInterval {
  const source = value ?? defaultPollInterval();
  const normalized = {
    m: normalizeIntervalPart(source.m),
    d: normalizeIntervalPart(source.d),
    h: normalizeIntervalPart(source.h),
    s: normalizeIntervalPart(source.s),
  };
  return normalized.m || normalized.d || normalized.h || normalized.s
    ? normalized
    : defaultPollInterval();
}

export function timedTaskPatch(
  startCondition: StartCondition,
  startAt: string | undefined,
  pollInterval: PollInterval | undefined,
): {
  start_condition?: StartCondition;
  start_at?: string;
  poll_interval?: PollInterval;
} {
  return {
    ...(startCondition !== "user_action"
      ? { start_condition: startCondition }
      : {}),
    ...(startCondition === "scheduled_task" || startCondition === "polling_task"
      ? startAt
        ? { start_at: startAt }
        : {}
      : {}),
    ...(startCondition === "polling_task"
      ? { poll_interval: normalizePollInterval(pollInterval) }
      : startCondition === "scheduled_task"
        ? { poll_interval: emptyPollInterval() }
        : {}),
  };
}

export function formatTicketTime(value: string | number | undefined): string {
  if (!value) {
    return t("notScheduled");
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return t("notScheduled");
  }
  return date.toLocaleString();
}

export function formatCalendarEventTime(
  value: string | number | undefined,
): string {
  if (!value) {
    return "";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return date.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatStartCondition(
  value: StartCondition | undefined,
): string {
  switch (value) {
    case "session_idle":
      return t("sessionIdle");
    case "scheduled_task":
      return t("scheduledTask");
    case "polling_task":
      return t("pollingTask");
    case "user_action":
    default:
      return t("runNow");
  }
}

export function isTimedStartCondition(
  value: StartCondition | undefined,
): value is "scheduled_task" | "polling_task" {
  return value === "scheduled_task" || value === "polling_task";
}

export function materializeComposerContent(
  text: string,
  images: ComposerImage[],
): string {
  const seen = new Set<string>();
  let index = 0;
  let content = text;
  for (const image of images) {
    const isImage = (image.kind ?? "image") === "image";
    const token = isImage
      ? composerImageToken(image.id)
      : composerFileToken(image.id);
    if (!content.includes(token)) {
      continue;
    }
    seen.add(image.id);
    index += 1;
    content = content.replaceAll(
      token,
      isImage
        ? `\n[Image ${index}: ${image.name}]\n[MEDIA:${image.dataUrl}:MEDIA]\n`
        : `\n[File ${index}: ${image.name}]\n`,
    );
  }
  const trailing = images.filter((image) => !seen.has(image.id));
  if (trailing.length > 0) {
    const appendix = trailing
      .map((image) => {
        const isImage = (image.kind ?? "image") === "image";
        index += 1;
        return isImage
          ? `[Image ${index}: ${image.name}]\n[MEDIA:${image.dataUrl}:MEDIA]`
          : `[File ${index}: ${image.name}]`;
      })
      .join("\n\n");
    content = `${content.trim()}\n\n${appendix}`;
  }
  return content.trim();
}

function composerImageToken(id: string): string {
  return `[[image:${id}]]`;
}

function composerFileToken(id: string): string {
  return `[[file:${id}]]`;
}
