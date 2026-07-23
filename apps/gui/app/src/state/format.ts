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

export function classNames(...values: Array<string | false | undefined>): string {
  return values.filter(Boolean).join(" ");
}
