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
} from "../conversation/conversation-view";
import { applyGatewayEvent } from "../state/event-reducer";
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
} from "../state/global-store";
import { classNames, truncate } from "../state/format";
import { t, type TextKey } from "../i18n";

import {
  formatTicketTime,
  normalizeIntervalPart,
  sessionTaskState,
  taskPollInterval,
  taskStartAt,
  taskStartCondition,
} from "../features/plan/tasks";
export function copyText(value: string): void {
  if (typeof navigator !== "undefined" && navigator.clipboard) {
    void navigator.clipboard.writeText(value);
  }
}

export function providerSourceLabel(source?: string | null): string {
  const normalized = source?.toLowerCase();
  if (normalized === "config") {
    return t("config");
  }
  if (normalized === "env") {
    return t("env");
  }
  return source || t("notConfigured");
}

export function authStatusText(
  status?: AppState["providerAuthStatus"][string],
): string {
  if (!status) {
    return "--";
  }
  if (status.authenticated) {
    return t("connected");
  }
  if (status.expired) {
    return t("expired");
  }
  if (status.configured) {
    return t("configured");
  }
  return t("notConfigured");
}

export function formatModelLimit(value?: number): string {
  if (!value) {
    return "--";
  }
  if (value >= 1_000_000) {
    return `${Math.round(value / 1_000_000)}M`;
  }
  if (value >= 1_000) {
    return `${Math.round(value / 1_000)}K`;
  }
  return String(value);
}

export function eventBelongsToState(
  state: AppState,
  directory?: string | null,
): boolean {
  if (!directory || directory === "global") {
    return true;
  }
  if (!state.directory) {
    return true;
  }
  return samePath(directory, state.directory);
}

export function samePath(left?: string | null, right?: string | null): boolean {
  if (!left || !right) {
    return false;
  }
  return normalizePath(left) === normalizePath(right);
}

export function normalizePath(value: string): string {
  const normalized = value.replaceAll("\\", "/").replace(/\/+$/, "");
  return /^[A-Za-z]:$/u.test(normalized)
    ? `${normalized}/`.toLowerCase()
    : normalized.toLowerCase();
}

export function parentPath(path: string): string {
  const parts = path.replaceAll("\\", "/").split("/").filter(Boolean);
  parts.pop();
  return parts.join("/");
}

export function shortPathLabel(path?: string | null): string | undefined {
  if (!path) {
    return undefined;
  }
  const parts = path.replaceAll("\\", "/").split("/").filter(Boolean);
  return parts.at(-1) ?? path;
}

export function shortWorkspaceLabel(path?: string | null): string {
  return shortPathLabel(path) ?? t("noWorkspace");
}

export function fixtureFiles(
  fixture: string | undefined,
  path = "",
): FileInfo[] {
  if (fixture !== "plan-sessions") {
    return [];
  }
  const root = "C:\\Users\\liuliu\\Documents\\tura";
  const makeFile = (
    name: string,
    relativePath: string,
    type: "directory" | "file",
    size = type === "directory" ? null : 128,
  ): FileInfo => ({
    name,
    path: relativePath,
    type,
    absolute: `${root}\\${relativePath.replaceAll("/", "\\")}`,
    ignored: false,
    git_status: "not_git",
    size_bytes: size,
    modified_at: Date.now() - 12_000,
  });
  const tree: Record<string, FileInfo[]> = {
    "": [
      makeFile("apps", "apps", "directory"),
      makeFile("crates", "crates", "directory"),
      makeFile("README.md", "README.md", "file"),
      makeFile("package.json", "package.json", "file"),
    ],
    apps: [
      makeFile("gui", "apps/gui", "directory"),
      makeFile("tui", "apps/tui", "directory"),
      makeFile("app.config.ts", "apps/app.config.ts", "file"),
    ],
    "apps/gui": [
      makeFile("app", "apps/gui/app", "directory"),
      makeFile("e2e", "apps/gui/e2e", "directory"),
      makeFile("package.json", "apps/gui/package.json", "file"),
    ],
    crates: [
      makeFile("gateway", "crates/gateway", "directory"),
      makeFile("runtime", "crates/runtime", "directory"),
      makeFile("Cargo.toml", "crates/Cargo.toml", "file"),
    ],
  };
  return tree[path] ?? [];
}

export function shortSessionTitle(title: string): string {
  return title.length > 24 ? `${title.slice(0, 21)}...` : title;
}

export function relativeSessionTime(session: Session): string {
  const updated = sessionUpdatedAt(session);
  if (!updated) {
    return "";
  }
  const delta = Math.max(0, Date.now() - normalizeTimeMs(updated));
  const minutes = Math.max(1, Math.floor(delta / 60_000));
  if (minutes < 60) {
    return `${minutes}分钟`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `${hours}小时`;
  }
  return `${Math.floor(hours / 24)}天`;
}

export function sessionHoverTitle(session: Session): string {
  const schedule = sessionScheduleHoverText(session);
  return schedule
    ? `${sessionTitle(session)}\n${schedule}`
    : sessionTitle(session);
}

export function sessionScheduleHoverText(session: Session): string | undefined {
  const task = sessionTaskState(session);
  const condition = taskStartCondition(task);
  if (condition === "scheduled_task") {
    return `${t("scheduledTask")}: ${formatTicketTime(taskStartAt(task))}`;
  }
  if (condition !== "polling_task") {
    return undefined;
  }
  const next = nextPollingTime(taskStartAt(task), taskPollInterval(task));
  return `${t("pollingTask")}: ${next ? formatTicketTime(next) : formatTicketTime(taskStartAt(task))}`;
}

export function nextPollingTime(
  startAt: string | number | undefined,
  interval: PollInterval,
): string | undefined {
  if (!startAt) {
    return undefined;
  }
  const start = new Date(startAt).getTime();
  if (Number.isNaN(start)) {
    return undefined;
  }
  const step =
    normalizeIntervalPart(interval.d) * 86_400_000 +
    normalizeIntervalPart(interval.h) * 3_600_000 +
    normalizeIntervalPart(interval.m) * 60_000 +
    normalizeIntervalPart(interval.s) * 1_000;
  if (step <= 0) {
    return new Date(start).toISOString();
  }
  const now = Date.now();
  if (start > now) {
    return new Date(start).toISOString();
  }
  return new Date(start + Math.ceil((now - start) / step) * step).toISOString();
}

export function normalizeTimeMs(value: number): number {
  return value > 10_000_000_000 ? value : value * 1000;
}

export function readConfigString(
  config: Record<string, unknown>,
  key: string,
): string | undefined {
  const value = config[key];
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

export function readConfigBoolean(
  config: Record<string, unknown>,
  key: string,
): boolean | undefined {
  const value = config[key];
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (["true", "1", "yes", "on"].includes(normalized)) {
      return true;
    }
    if (["false", "0", "no", "off"].includes(normalized)) {
      return false;
    }
  }
  return undefined;
}

export function inputHeight(value: string): string {
  const lines = Math.min(
    12,
    Math.max(
      3,
      value.split(/\r\n|\r|\n/u).length + Math.floor(value.length / 72),
    ),
  );
  return `${lines * 24 + 36}px`;
}

export function fileGitRemark(file: FileInfo): string {
  const status = file.git_status ?? (file.ignored ? "ignored" : "not_git");
  switch (status) {
    case "added":
      return t("added");
    case "changed":
      return t("changed");
    case "copied":
      return t("copied");
    case "deleted":
      return t("deleted");
    case "ignored":
      return t("ignored");
    case "modified":
      return t("modified");
    case "renamed":
      return t("renamed");
    case "untracked":
      return t("untracked");
    case "not_git":
      return t("notGit");
    default:
      return t("clean");
  }
}

export function formatFileSize(file: FileInfo): string {
  if (file.type === "directory") {
    return "--";
  }
  const bytes = file.size_bytes;
  if (bytes === undefined || bytes === null) {
    return "--";
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${unit}`;
}

export function formatModifiedTime(value?: number | null): string {
  if (!value) {
    return "--";
  }
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

export function readSearchParam(name: string): string | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }
  return new URLSearchParams(window.location.search).get(name) ?? undefined;
}

export function readBooleanSearchParam(name: string): boolean {
  const value = readSearchParam(name);
  return value === "1" || value === "true" || value === "yes";
}

export function readMainTabSearchParam(): MainTab | undefined {
  const tab = readSearchParam("tab");
  return tab === "plan" ||
    tab === "new" ||
    tab === "conversation" ||
    tab === "files" ||
    tab === "settings"
    ? tab
    : undefined;
}

export function withInitialOverrides(
  state: AppState,
  overrides: {
    activeTab?: MainTab;
    selectedSessionId?: string;
    selectedModel?: string;
    selectedAgent?: string;
  },
): AppState {
  const activeTab = overrides.activeTab ?? state.activeTab;
  return {
    ...state,
    activeTab,
    previousMainTab:
      activeTab === "settings"
        ? state.previousMainTab
        : activeTab === "conversation"
          ? "new"
          : activeTab,
    selectedSessionId: overrides.selectedSessionId ?? state.selectedSessionId,
    selectedModel: overrides.selectedModel ?? state.selectedModel,
    selectedAgent: overrides.selectedAgent ?? state.selectedAgent,
  };
}
