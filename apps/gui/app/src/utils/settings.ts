import { GatewayError } from "@tura/gateway-sdk";
import { type AppState, type ThemeMode } from "../state/global-store";
import { t } from "../i18n";
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

export function configToDraft(
  config: AppState["config"],
): Record<string, string> {
  if (!config) {
    return {};
  }
  return {
    language: config.language ?? "",
    theme: config.theme ?? "",
    main_font: config.main_font ?? "",
    code_font: config.code_font ?? "",
    main_font_size: config.main_font_size
      ? String(config.main_font_size)
      : "",
    code_font_size: config.code_font_size
      ? String(config.code_font_size)
      : "",
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
    main_font: draft.main_font || null,
    code_font: draft.code_font || null,
    main_font_size: draft.main_font_size
      ? Number(draft.main_font_size)
      : null,
    code_font_size: draft.code_font_size
      ? Number(draft.code_font_size)
      : null,
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

function draftValue(value: unknown): string {
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

function parseDraftValue(value: string): unknown {
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

function normalizeErrorBody(body: unknown): Record<string, unknown> {
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
