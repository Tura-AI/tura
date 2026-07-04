import { GatewayError, type ProviderAuthMethod } from "@tura/gateway-sdk";
import { t } from "../i18n";
import { type AppState, type CornerRadiusMode, type ThemeMode } from "../state/global-store";
export { copyText } from "./app-format";

export type ProviderAuthDisplayLevel = "ok" | "warn" | "fail" | "neutral";

export type ProviderAuthDisplayState = {
  label: string;
  level: ProviderAuthDisplayLevel;
  configured: boolean;
};

export function defaultModel(providers: AppState["providers"]): string | undefined {
  if (!providers) {
    return "openai/gpt-5.5";
  }
  if (providers.all.some((provider) => provider.id === "openai" && provider.models["gpt-5.5"])) {
    return "openai/gpt-5.5";
  }
  const firstConnected = providers.connected[0];
  if (firstConnected && providers.default[firstConnected]) {
    return `${firstConnected}/${providers.default[firstConnected]}`;
  }
  const firstProvider = providers.all[0];
  const firstModel = firstProvider ? Object.keys(firstProvider.models)[0] : undefined;
  return firstProvider && firstModel ? `${firstProvider.id}/${firstModel}` : undefined;
}

export function configToDraft(config: AppState["config"]): Record<string, string> {
  if (!config) {
    return {};
  }
  return {
    theme: config.theme ?? "",
    corner_radius: config.corner_radius ?? "",
    main_font: config.main_font ?? "",
    code_font: config.code_font ?? "",
    main_font_size: config.main_font_size ? String(config.main_font_size) : "",
    code_font_size: config.code_font_size ? String(config.code_font_size) : "",
    skill_folders: (config.skill_folders ?? []).join(", "),
  };
}

export function configDraftToPatch(
  draft: Record<string, string>,
  themeMode: ThemeMode,
  cornerRadius: CornerRadiusMode,
): Partial<NonNullable<AppState["config"]>> {
  return {
    theme: themeMode,
    corner_radius: cornerRadius,
    main_font: draft.main_font || null,
    code_font: draft.code_font || null,
    main_font_size: draft.main_font_size ? Number(draft.main_font_size) : null,
    code_font_size: draft.code_font_size ? Number(draft.code_font_size) : null,
    skill_folders: draft.skill_folders
      ? draft.skill_folders
          .split(",")
          .map((item) => item.trim())
          .filter(Boolean)
      : [],
  };
}

export function recordToDraft(record: Record<string, unknown>): Record<string, string> {
  return Object.fromEntries(Object.entries(record).map(([key, value]) => [key, draftValue(value)]));
}

export function draftToRecord(draft: Record<string, string>): Record<string, unknown> {
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

export function providerAuthDisplayState(
  state: AppState,
  providerId: string,
): ProviderAuthDisplayState {
  const receipt = state.providerValidationReceipts[providerId];
  if (receipt) {
    if (receipt.level === "unsupported") {
      return {
        label: t("connected"),
        level: "neutral",
        configured: true,
      };
    }
    if (receipt.level === "warning") {
      return {
        label: t("validationWarning"),
        level: "warn",
        configured: true,
      };
    }
    if (receipt.ok || receipt.level === "valid") {
      return {
        label: t("validationValid"),
        level: "ok",
        configured: true,
      };
    }
    return {
      label: t("validationInvalid"),
      level: "fail",
      configured: false,
    };
  }
  const status = state.providerAuthStatus[providerId];
  if (status?.authenticated) {
    return {
      label: t("connected"),
      level: "neutral",
      configured: true,
    };
  }
  if (status?.expired) {
    return {
      label: t("expired"),
      level: "fail",
      configured: true,
    };
  }
  if (status?.configured) {
    return {
      label: t("configured"),
      level: "neutral",
      configured: true,
    };
  }
  return {
    label: t("notConfigured"),
    level: "neutral",
    configured: false,
  };
}

export function providerConfigured(state: AppState, providerId: string): boolean {
  return providerAuthDisplayState(state, providerId).configured;
}

export function providerAuthDraftKey(providerId: string, method: ProviderAuthMethod): string {
  return [providerId, method.token_env || method.login_env || method.kind].join("::");
}

export function providerAuthMethodForValidation(
  providerId: string,
  methods: ProviderAuthMethod[],
  authDrafts: Record<string, string>,
): ProviderAuthMethod | undefined {
  return methods.find((method) => authDrafts[providerAuthDraftKey(providerId, method)]?.trim());
}

export function providerIdFromAuthError(error: unknown, state: AppState): string | undefined {
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
  const direct = [body.provider_id, body.providerID, body.provider, body.llm_provider].find(
    (value): value is string => typeof value === "string",
  );
  if (direct && state.providers?.all.some((provider) => provider.id === direct)) {
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
