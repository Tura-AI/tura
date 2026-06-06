import { GatewayError, type Session } from "@tura/gateway-sdk";
import {
  sessionHasDisplayName,
  systemThemeMode,
  type AppState,
  type ThemeMode,
} from "./state/global-store";
import { providerIdFromAuthError, providerIdFromModel } from "./utils/settings";

const LAST_SESSION_OPENED_STORAGE_KEY = "last_session_opened";
const LEGACY_LAST_SESSION_OPENED_STORAGE_KEY = "last cession oppend";
let lastSessionOpenedMemory: string | undefined;

export function readLastSessionOpened(): string | undefined {
  let stored: string | undefined;
  if (typeof window === "undefined") {
    return lastSessionOpenedMemory;
  }
  try {
    stored =
      window.localStorage.getItem(LAST_SESSION_OPENED_STORAGE_KEY)?.trim() ||
      window.localStorage.getItem(LEGACY_LAST_SESSION_OPENED_STORAGE_KEY)?.trim() ||
      undefined;
    if (stored) {
      window.localStorage.setItem(LAST_SESSION_OPENED_STORAGE_KEY, stored);
      window.localStorage.removeItem(LEGACY_LAST_SESSION_OPENED_STORAGE_KEY);
    }
  } catch {
    stored = undefined;
  }
  return stored ?? lastSessionOpenedMemory;
}

export function writeLastSessionOpened(sessionId: string) {
  lastSessionOpenedMemory = sessionId;
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(LAST_SESSION_OPENED_STORAGE_KEY, sessionId);
    window.localStorage.removeItem(LEGACY_LAST_SESSION_OPENED_STORAGE_KEY);
  } catch {
    // Memory fallback keeps tab navigation deterministic when storage is blocked.
  }
}

export function clearLastSessionOpened() {
  lastSessionOpenedMemory = undefined;
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.removeItem(LAST_SESSION_OPENED_STORAGE_KEY);
    window.localStorage.removeItem(LEGACY_LAST_SESSION_OPENED_STORAGE_KEY);
  } catch {
    // Nothing else to clear when storage is blocked.
  }
}

export function providerIssueIdFromError(error: unknown, state: AppState): string | undefined {
  const authProvider = providerIdFromAuthError(error, state);
  if (authProvider) {
    return authProvider;
  }
  if (!(error instanceof GatewayError)) {
    return undefined;
  }
  const bodyText = JSON.stringify(error.body ?? {}).toLowerCase();
  const messageText = error.message.toLowerCase();
  const billingLike =
    error.status === 402 ||
    /\b(billing|payment|quota|credit|balance|insufficient|subscription|rate_limit|rate limit|limit exceeded)\b/u.test(
      `${bodyText} ${messageText}`,
    );
  return billingLike ? providerIdFromModel(state.selectedModel) : undefined;
}

export function mergeSessions(remoteSessions: Session[], localSessions: Session[]) {
  const byId = new Map<string, Session>();
  for (const session of remoteSessions) {
    byId.set(session.id, session);
  }
  for (const session of localSessions) {
    const remote = byId.get(session.id);
    if (!remote) {
      byId.set(session.id, session);
    } else if (!sessionHasDisplayName(remote) && sessionHasDisplayName(session)) {
      byId.set(session.id, {
        ...remote,
        name: session.name,
        session_display_name: session.session_display_name,
        plan_summary: session.plan_summary,
      });
    }
  }
  return [...byId.values()].sort((a, b) => (b.updated_at ?? 0) - (a.updated_at ?? 0));
}

export function normalizeThemeMode(value: string | null | undefined): ThemeMode {
  return value === "light" ||
    value === "dark" ||
    value === "caral" ||
    value === "uruk" ||
    value === "liangzhu"
    ? value
    : systemThemeMode();
}

export function clampNumber(
  value: number | null | undefined,
  min: number,
  max: number,
  fallback: number,
): number {
  return Math.min(max, Math.max(min, Number.isFinite(value) ? value! : fallback));
}
