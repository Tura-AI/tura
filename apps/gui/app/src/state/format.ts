export function formatTime(value?: number): string {
  if (!value) {
    return "--";
  }
  const date = new Date(value > 10_000_000_000 ? value : value * 1000);
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
    day: "numeric",
  }).format(date);
}

export function compactPath(value?: string): string {
  if (!value) {
    return "No workspace";
  }
  const normalized = value.replaceAll("\\", "/");
  const parts = normalized.split("/").filter(Boolean);
  if (parts.length <= 3) {
    return value;
  }
  return `.../${parts.slice(-3).join("/")}`;
}

export function truncate(value: string, max = 120): string {
  return value.length > max ? `${value.slice(0, max - 1)}...` : value;
}

export function jsonPreview(value: unknown): string {
  if (value === undefined || value === null) {
    return "";
  }
  if (typeof value === "string") {
    return value;
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function classNames(
  ...values: Array<string | false | undefined>
): string {
  return values.filter(Boolean).join(" ");
}
