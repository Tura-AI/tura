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
import { providerSourceLabel } from "./app-format";
export { authStatusText, copyText, providerSourceLabel } from "./app-format";

export function defaultModel(
  providers: AppState["providers"],
): string | undefined {
  if (!providers) {
    return "openai/gpt-5.5";
  }
  if (
    providers.all.some(
      (provider) => provider.id === "openai" && provider.models["gpt-5.5"],
    )
  ) {
    return "openai/gpt-5.5";
  }
  const firstConnected = providers.connected[0];
  if (firstConnected && providers.default[firstConnected]) {
    return `${firstConnected}/${providers.default[firstConnected]}`;
  }
  const firstProvider = providers.all[0];
  const firstModel = firstProvider
    ? Object.keys(firstProvider.models)[0]
    : undefined;
  return firstProvider && firstModel
    ? `${firstProvider.id}/${firstModel}`
    : undefined;
}

export function settingsSections(): Array<{
  id: SettingsSection;
  label: string;
}> {
  return [
    { id: "general", label: t("general") },
    { id: "appearance", label: t("appearance") },
    { id: "providers", label: t("providers") },
    { id: "models", label: t("models") },
    { id: "auth", label: t("login") },
    { id: "runtime", label: t("runtime") },
    { id: "config", label: t("turaConfig") },
    { id: "workspace", label: t("workspaceConfig") },
    { id: "environment", label: t("environment") },
  ];
}

export function configFieldRows(
  state: AppState,
): Array<{ key: string; label: string }> {
  const keys = new Set([
    "language",
    "theme",
    "model",
    "agent",
    "skill_folders",
    ...Object.keys(state.configDraft),
  ]);
  return [...keys].map((key) => ({
    key,
    label: configFieldLabel(key),
  }));
}

export function configFieldLabel(key: string): string {
  const labels: Record<string, TextKey> = {
    agent: "agent",
    language: "language",
    model: "model",
    skill_folders: "skillFolders",
    theme: "theme",
  };
  return labels[key] ? t(labels[key]) : key.replaceAll("_", " ");
}

export function configToDraft(
  config: AppState["config"],
): Record<string, string> {
  if (!config) {
    return {};
  }
  return {
    language: config.language ?? "",
    theme: config.theme ?? "",
    model: config.model ?? "",
    agent: config.agent ?? "",
    skill_folders: (config.skill_folders ?? []).join(", "),
  };
}

export function configDraftToPatch(
  draft: Record<string, string>,
  themeMode: ThemeMode,
): Partial<NonNullable<AppState["config"]>> {
  return {
    language: draft.language || null,
    theme: themeMode,
    model: draft.model || null,
    agent: draft.agent || null,
    skill_folders: draft.skill_folders
      ? draft.skill_folders
          .split(",")
          .map((item) => item.trim())
          .filter(Boolean)
      : [],
  };
}

export function recordToDraft(
  record: Record<string, unknown>,
): Record<string, string> {
  return Object.fromEntries(
    Object.entries(record).map(([key, value]) => [key, draftValue(value)]),
  );
}

export function draftToRecord(
  draft: Record<string, string>,
): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(draft).map(([key, value]) => [key, parseDraftValue(value)]),
  );
}

export function draftValue(value: unknown): string {
  if (value === undefined || value === null) {
    return "";
  }
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

export function parseDraftValue(value: string): unknown {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }
  if (trimmed === "true") {
    return true;
  }
  if (trimmed === "false") {
    return false;
  }
  if (/^-?\d+(\.\d+)?$/u.test(trimmed)) {
    return Number(trimmed);
  }
  if (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  ) {
    try {
      return JSON.parse(trimmed);
    } catch {
      return value;
    }
  }
  return value;
}

export function parseModelRef(
  value?: string | null,
): { providerId: string; modelId: string } | undefined {
  if (!value) {
    return undefined;
  }
  const index = value.indexOf("/");
  if (index <= 0 || index >= value.length - 1) {
    return undefined;
  }
  return {
    providerId: value.slice(0, index),
    modelId: value.slice(index + 1),
  };
}

export function providerIdFromModel(value?: string | null): string | undefined {
  return parseModelRef(value)?.providerId;
}

export function modelRef(providerId?: string, modelId?: string): string {
  return providerId && modelId ? `${providerId}/${modelId}` : "";
}

export function providerStateLabel(
  state: AppState,
  providerId: string,
  source: string,
): string {
  const status = state.providerAuthStatus[providerId];
  if (status?.authenticated) {
    return t("connected");
  }
  if (status?.configured) {
    return t("configured");
  }
  if (state.providers?.connected.includes(providerId)) {
    return t("connected");
  }
  return source ? providerSourceLabel(source) : t("notConfigured");
}

export function providerConfigured(
  state: AppState,
  providerId: string,
): boolean {
  const status = state.providerAuthStatus[providerId];
  return Boolean(
    status?.authenticated ||
    status?.configured ||
    state.providers?.connected.includes(providerId),
  );
}

export function providerIdFromAuthError(
  error: unknown,
  state: AppState,
): string | undefined {
  if (!(error instanceof GatewayError)) {
    return undefined;
  }
  const body = normalizeErrorBody(error.body);
  const text = JSON.stringify(body).toLowerCase();
  const authLike =
    error.status === 401 ||
    error.status === 403 ||
    /\b(auth|oauth|token|credential|unauthorized|forbidden|expired|invalid_api_key|invalid key)\b/u.test(
      text,
    );
  if (!authLike) {
    return undefined;
  }
  const direct = [
    body.provider_id,
    body.providerID,
    body.provider,
    body.llm_provider,
  ].find((value): value is string => typeof value === "string");
  if (
    direct &&
    state.providers?.all.some((provider) => provider.id === direct)
  ) {
    return direct;
  }
  const fromText = state.providers?.all.find((provider) =>
    text.includes(provider.id.toLowerCase()),
  )?.id;
  return fromText ?? providerIdFromModel(state.selectedModel);
}

export function normalizeErrorBody(body: unknown): Record<string, unknown> {
  if (body && typeof body === "object" && !Array.isArray(body)) {
    return body as Record<string, unknown>;
  }
  if (typeof body !== "string") {
    return {};
  }
  try {
    const parsed = JSON.parse(body);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : { message: body };
  } catch {
    return { message: body };
  }
}
