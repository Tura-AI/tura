import {
  type PlanStatus,
  type Message,
  type PollInterval,
  type Session,
  type StartCondition,
  type TaskManagement,
} from "@tura/gateway-sdk";
import { sessionHasRunningCommand } from "../../conversation/session-animation";
import { t } from "../../i18n";
import type { ComposerImage } from "../../state/global-store";
import { sessionTitle, sessionUpdatedAt } from "../../state/global-store";
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
    case "waiting_user":
      return t("waitingUser");
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
  return sessionTasks(session).filter(taskIsVisibleInFrontend).sort(compareTaskStep);
}

export function hasVisibleSessionTasks(session: Session): boolean {
  return sessionTasks(session).some(taskIsVisibleInFrontend);
}

export function taskIsVisibleInFrontend(task: TaskManagement): boolean {
  const status = taskPlanStatus(task);
  return status !== "done" && status !== "archived" && taskHasVisibleContent(task);
}

export function taskHasVisibleContent(task: TaskManagement): boolean {
  return taskDisplayText(task).trim().length > 0;
}

export function compareTaskStep(left: TaskManagement, right: TaskManagement): number {
  const leftStep = typeof left.step === "number" ? left.step : Number.POSITIVE_INFINITY;
  const rightStep = typeof right.step === "number" ? right.step : Number.POSITIVE_INFINITY;
  return leftStep - rightStep;
}

export function taskNonceId(task: TaskManagement): string | undefined {
  return task.task_id;
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
  return task.start_at ? "scheduled_task" : "session_idle";
}

function isStartConditionStatus(value: string | undefined): value is StartCondition {
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
  const deliverable = (task.deliverable ?? "").trim();
  return [summary, deliverable].filter(Boolean).join("\n\n");
}

export function firstRunnableTask(session: Session): TaskManagement | undefined {
  return sortedSessionTasks(session).find((task) => isRunnableTask(task));
}

export function isRunnableTask(task: TaskManagement): boolean {
  const status = taskPlanStatus(task);
  return (
    taskDisplayText(task).trim().length > 0 &&
    (status === undefined || status === "todo" || status === "waiting_user" || status === "doing")
  );
}

export function taskSummaryText(task: TaskManagement): string {
  return (task.task_summary ?? task.deliverable ?? "").trim().split(/\r?\n/u)[0]?.trim() ?? "";
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

export function timedTaskDisplayDate(task: TaskManagement, nowMs = Date.now()): Date | undefined {
  const raw = taskStartAt(task);
  if (!raw) {
    return undefined;
  }
  const start = new Date(raw);
  if (Number.isNaN(start.getTime())) {
    return undefined;
  }
  if (taskStartCondition(task) !== "polling_task") {
    return start;
  }
  const interval = taskPollInterval(task);
  const intervalMs =
    (interval.d ?? 0) * 86_400_000 +
    (interval.h ?? 0) * 3_600_000 +
    (interval.m ?? 0) * 60_000 +
    (interval.s ?? 0) * 1_000;
  if (intervalMs <= 0) {
    return start;
  }
  const startMs = start.getTime();
  const nextMs =
    startMs > nowMs ? startMs : startMs + Math.ceil((nowMs - startMs) / intervalMs) * intervalMs;
  return new Date(nextMs);
}

export function applyTaskPatchToSession(session: Session, patch: Partial<TaskManagement>): Session {
  const current = sessionTaskState(session);
  const nonce = patch.task_id;
  if (Array.isArray(current.tasks)) {
    const tasks = sessionTasks(session);
    const index = nonce
      ? tasks.findIndex((task) => taskNonceId(task) === nonce)
      : tasks.length > 0
        ? 0
        : -1;
    const nextTasks =
      index >= 0
        ? tasks.map((task, itemIndex) => (itemIndex === index ? { ...task, ...patch } : task))
        : [...tasks, { ...patch, task_id: nonce ?? `${session.id}:${tasks.length}` }];
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

export function appendTaskToSession(session: Session, task: Partial<TaskManagement>): Session {
  const current = sessionTaskState(session);
  const existingTasks = Array.isArray(current.tasks)
    ? sessionTasks(session)
    : taskHasVisibleContent(current)
      ? [current]
      : [];
  const nonce = taskNonceId(task) ?? `${session.id}:${existingTasks.length}`;
  const nextTask = {
    ...task,
    task_id: nonce,
    step: typeof task.step === "number" && task.step > 0 ? task.step : existingTasks.length + 1,
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

export function reorderTasksInSession(session: Session, orderedTasks: TaskManagement[]): Session {
  const current = sessionTaskState(session);
  const nextTasks = orderedTasks.map((task, index) => ({
    ...task,
    step: index + 1,
  }));
  return {
    ...session,
    task_management: {
      ...current,
      tasks: nextTasks,
    },
  };
}

export function planSessionStatus(session: Session, messages: Message[] = []): PlanStatus {
  if (session.status === "busy" || sessionHasRunningCommand(messages)) {
    return "doing";
  }
  const task = sessionTaskState(session);
  if (task.status === "archived") {
    return "archived";
  }
  const tasks = sessionTasks(session);
  if (session.status === "idle") {
    if (tasks.some((task) => taskPlanStatus(task) === "question")) {
      return "question";
    }
    const visibleTasks = tasks.filter(taskHasVisibleContent);
    if (visibleTasks.length > 0 && visibleTasks.every((task) => taskPlanStatus(task) === "done")) {
      return "done";
    }
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

export function shouldShowSessionAttention(
  session: Session,
  acknowledged: boolean,
  messages: Message[] = [],
): boolean {
  const status = planSessionStatus(session, messages);
  return status === "doing" || status === "question" || (!acknowledged && status === "done");
}

export function planStoredPlanStatus(session: Session): PlanStatus | undefined {
  const task = sessionTaskState(session);
  const status = task.status;
  if (
    status === "todo" ||
    status === "waiting_user" ||
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

export function planSessionStartCondition(session: Session): StartCondition | undefined {
  const task = sessionTaskState(session);
  return taskStartCondition(task);
}

export function planTimedSessions(sessions: Session[]): Session[] {
  return sessions.filter((session) => {
    if (planStoredPlanStatus(session) !== "todo" && timedSessionTasks(session).length === 0) {
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
      (taskPlanStatus(task) ?? planStoredPlanStatus(session) ?? "todo") === "todo" &&
      isTimedStartCondition(taskStartCondition(task)) &&
      Boolean(taskStartAt(task)),
  );
}

export function queuedSessionTasks(session: Session): TaskManagement[] {
  return sortedSessionTasks(session);
}

export function planQueuedSessions(sessions: Session[]): Session[] {
  return sessions.filter((session) => queuedSessionTasks(session).length > 0);
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

export function utcIsoToLocalDateTime(value: string | number | undefined): string {
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

export function normalizeIntervalPart(value: string | number | undefined): number {
  const parsed = Number(value ?? 0);
  return Number.isFinite(parsed) && parsed > 0 ? Math.floor(parsed) : 0;
}

export function normalizePollInterval(value: PollInterval | undefined): PollInterval {
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
    ...(startCondition !== "user_action" ? { start_condition: startCondition } : {}),
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

export function formatCalendarEventTime(value: string | number | undefined): string {
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

export function formatStartCondition(value: StartCondition | undefined): string {
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

export function materializeComposerContent(text: string, images: ComposerImage[]): string {
  const seen = new Set<string>();
  let index = 0;
  let content = text;
  for (const image of images) {
    const isImage = (image.kind ?? "image") === "image";
    const token = isImage ? composerImageToken(image.id) : composerFileToken(image.id);
    if (!content.includes(token)) {
      continue;
    }
    seen.add(image.id);
    index += 1;
    content = content.replaceAll(
      token,
      isImage
        ? `\n[Image ${index}: ${image.name}]\n[MEDIA:${image.dataUrl}:MEDIA]\n`
        : `\n[File ${index}: ${image.name}]\n[MEDIA:${image.dataUrl}:MEDIA]\n`,
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
          : `[File ${index}: ${image.name}]\n[MEDIA:${image.dataUrl}:MEDIA]`;
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
