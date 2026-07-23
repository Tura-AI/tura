import { t } from "../i18n.js";

export class GatewayHttpError extends Error {
  constructor(
    public status: number,
    public url: string,
    message: string,
    public body?: string,
  ) {
    super(message);
  }
}

export function userFacingError(error: unknown): string {
  if (error instanceof GatewayHttpError) {
    if (error.status === 0) return networkErrorMessage(error);
    const details = gatewayErrorDetails(error.body);
    const status = httpStatusMessage(error.status);
    const bodyMessage = details.message || compactBody(error.body);
    const provider = details.provider
      ? t("gatewayErrorProvider", { provider: details.provider })
      : "";
    const code = details.code ? t("gatewayErrorCode", { code: details.code }) : "";
    const suffix = [provider, code, bodyMessage].filter(Boolean).join(" ");
    return suffix
      ? t("gatewayHttpErrorWithDetails", { status, details: suffix })
      : t("gatewayHttpError", { status });
  }
  return error instanceof Error ? error.message : String(error);
}

function networkErrorMessage(error: GatewayHttpError): string {
  const message = error.message || "";
  if (/abort|timeout|timed out/i.test(message)) {
    return t("gatewayTimeoutError");
  }
  if (/ECONNREFUSED|fetch failed|Failed to fetch|ECONNRESET|socket|network/i.test(message)) {
    return t("gatewayDisconnectedError");
  }
  return t("gatewayNetworkError", { error: message || t("unknown") });
}

function httpStatusMessage(status: number): string {
  if (status === 400) return t("gatewayBadRequest");
  if (status === 401) return t("gatewayUnauthorized");
  if (status === 403) return t("gatewayForbidden");
  if (status === 404) return t("gatewayNotFound");
  if (status === 408) return t("gatewayRequestTimeout");
  if (status === 409) return t("gatewayConflict");
  if (status === 422) return t("gatewayUnprocessable");
  if (status === 429) return t("gatewayRateLimited");
  if (status >= 500) return t("gatewayServerError", { status });
  return t("gatewayStatusError", { status });
}

function gatewayErrorDetails(body: string | undefined): {
  message?: string;
  code?: string;
  provider?: string;
} {
  if (!body) return {};
  try {
    const parsed = JSON.parse(body) as unknown;
    return detailsFromValue(parsed);
  } catch {
    return {};
  }
}

function detailsFromValue(value: unknown): { message?: string; code?: string; provider?: string } {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};
  const record = value as Record<string, unknown>;
  const nested =
    objectValue(record.error) ??
    objectValue(record.detail) ??
    objectValue(record.details) ??
    objectValue(record.cause);
  return {
    message:
      stringValue(record.message) ??
      stringValue(record.error) ??
      stringValue(record.detail) ??
      stringValue(record.reason) ??
      (nested ? detailsFromValue(nested).message : undefined),
    code:
      stringValue(record.code) ??
      stringValue(record.error_code) ??
      stringValue(record.type) ??
      (nested ? detailsFromValue(nested).code : undefined),
    provider:
      stringValue(record.provider) ??
      stringValue(record.provider_id) ??
      stringValue(record.providerID) ??
      (nested ? detailsFromValue(nested).provider : undefined),
  };
}

function objectValue(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function compactBody(body: string | undefined): string | undefined {
  const text = body?.replace(/\s+/g, " ").trim();
  if (!text) return undefined;
  return text.length > 240 ? `${text.slice(0, 237)}...` : text;
}
