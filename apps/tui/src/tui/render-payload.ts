import { t } from "../i18n.js";
import { truncate } from "./render-terminal.js";

const rawAnsiControlPattern =
  /\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x1b]*(?:\x07|\x1b\\)|[PX^_][\s\S]*?\x1b\\|.)/gu;

export function sanitizeRawTerminalText(value: string): string {
  return value.replace(/\r\n/g, "\n").replace(/\r/g, "\n").replace(rawAnsiControlPattern, "");
}

export function compactInlinePayloads(value: string): string {
  const compactKnown = value.replace(
    /\[([a-z_][a-z0-9_-]*):\s*([\s\S]*?)\]/gu,
    (match, tool, payload) => {
      const summary =
        compactPayloadField(String(payload)) ??
        compactCommandJson(normalizePayloadText(String(payload))) ??
        summarizePayloadText(String(payload));
      return summary ? `[${String(tool)}: ${summary}]` : String(match);
    },
  );
  const inline = compactKnown.replace(
    /\[([a-z_][a-z0-9_-]*):\s*(\{[\s\S]*?\})\]/giu,
    (match, tool, payload) => {
      const summary =
        compactPayloadField(String(payload)) ??
        compactCommandJson(normalizePayloadText(String(payload))) ??
        summarizePayloadText(String(payload));
      return summary ? `[${String(tool)}: ${summary}]` : String(match);
    },
  );
  return inline
    .split(/\r?\n/)
    .map((line) => {
      const match = line.trim().match(/^\[([a-z_][a-z0-9_-]*):\s*([\s\S]+)\]$/iu);
      if (!match) return line;
      const tool = match[1] ?? t("tool");
      const payload = match[2] ?? "";
      const summary =
        compactPayloadField(payload) ??
        compactCommandJson(normalizePayloadText(payload)) ??
        summarizePayloadText(payload);
      return summary ? `[${tool}: ${summary}]` : line;
    })
    .join("\n");
}

function summarizePayloadText(value: string): string | undefined {
  if (
    !/[{[]/.test(value) ||
    !/(command_run|apply_patch|image_url|command_type|tool_result|results)/i.test(value)
  )
    return undefined;
  const normalized = normalizePayloadText(value);
  const snippets: string[] = [];
  for (const match of normalized.matchAll(/"path"\s*:\s*"([^"]+)"/g))
    snippets.push(`[${t("read")}: ${match[1]}]`);
  for (const match of normalized.matchAll(/"command_line"\s*:\s*"([^"]+)"/g)) {
    const command = decodeJsonString(match[1] ?? "");
    if (looksLikeCommand(command)) snippets.push(`[${t("bash")}: ${firstCommandLine(command)}]`);
  }
  for (const match of normalized.matchAll(/"command"\s*:\s*"([^"]+)"/g)) {
    const command = decodeJsonString(match[1] ?? "");
    if (looksLikeCommand(command)) snippets.push(`[${t("bash")}: ${firstCommandLine(command)}]`);
  }
  for (const match of normalized.matchAll(/"command_type"\s*:\s*"([^"]+)"/g))
    snippets.push(`[${t("tool")}: ${match[1]}]`);
  for (const match of normalized.matchAll(/"label"\s*:\s*"([^"]+)"/g)) {
    if (/img|image/i.test(match[1])) snippets.push(`[${t("media")}: ${match[1]}]`);
  }
  const unique = Array.from(new Set(snippets)).slice(0, 8);
  if (unique.length) return unique.join("\n");
  return `[${t("toolResult")}]`;
}

export function extractCommandsFromText(value: string): string[] {
  const text = sanitizeRawTerminalText(value).trim();
  if (!text) return [];
  const commands: string[] = [];
  for (const pattern of [
    /"command_line"\s*:\s*"([^"]+)"/g,
    /"command"\s*:\s*"([^"]+)"/g,
    /`([^`\n]*(?:npm|pnpm|yarn|node|python|cargo|git|rg|powershell|pwsh|cmd)\s+[^`\n]*)`/gi,
  ]) {
    for (const match of text.matchAll(pattern)) {
      const command = decodeJsonString(match[1] ?? "");
      if (looksLikeCommand(command)) commands.push(command);
    }
  }
  const compact = compactCommandJson(text);
  if (compact) {
    for (const command of extractCommandsFromText(compact)) commands.push(command);
  }
  return commands;
}

export function extractCommandsFromUnknown(value: unknown): string[] {
  if (!value) return [];
  if (typeof value === "string") return extractCommandsFromText(value);
  if (Array.isArray(value)) return value.flatMap(extractCommandsFromUnknown);
  if (typeof value !== "object") return [];
  const object = value as Record<string, unknown>;
  const commands: string[] = [];
  for (const key of ["command_line", "command"]) {
    const command = object[key];
    if (typeof command === "string" && looksLikeCommand(command))
      commands.push(sanitizeRawTerminalText(command).trim());
  }
  for (const key of ["commands", "results", "steps", "input", "output"]) {
    commands.push(...extractCommandsFromUnknown(object[key]));
  }
  return commands;
}

export function firstCommandLine(value: string): string {
  return sanitizeRawTerminalText(value).trim().split(/\n/)[0]?.trim() ?? "";
}

export function looksLikeCommand(value: string): boolean {
  const command = firstCommandLine(value);
  return (
    Boolean(command) &&
    !/^https?:\/\//i.test(command) &&
    (/^(?:npm|pnpm|yarn|node|python|py|cargo|git|rg|powershell|pwsh|cmd|npx|tsx|tsc|ls|dir|cd|cat|type|echo|mkdir|copy|xcopy|robocopy|del|rm|cp|mv)\b/i.test(
      command,
    ) ||
      /^[A-Z][A-Za-z]+-[A-Z][A-Za-z]+\b/u.test(command))
  );
}

function decodeJsonString(value: string): string {
  try {
    return sanitizeRawTerminalText(JSON.parse(`"${value.replace(/"/g, '\\"')}"`) as string);
  } catch {
    return sanitizeRawTerminalText(value.replace(/\\"/g, '"').replace(/\\\\/g, "\\"));
  }
}

export function toolSummary(state: Record<string, unknown>): string {
  const output = state.output;
  if (typeof output === "string") {
    const clean = cleanToolText(output);
    return (
      compactPayloadField(clean) ??
      summarizePayloadText(clean) ??
      compactCommandJson(clean) ??
      clean
    );
  }
  if (output && typeof output === "object") {
    const object = output as Record<string, unknown>;
    for (const key of ["reply_message", "text", "summary", "stdout", "stderr"]) {
      const value = object[key];
      if (typeof value === "string" && value.trim()) {
        const clean = cleanToolText(value);
        return (
          compactPayloadField(clean) ??
          summarizePayloadText(clean) ??
          compactCommandJson(clean) ??
          clean
        );
      }
    }
  }
  const input = state.input;
  if (input && typeof input === "object") {
    const object = input as Record<string, unknown>;
    for (const key of ["step_summary", "task_group", "summary", "status", "label"]) {
      const value = object[key];
      if (typeof value === "string" && value.trim()) return sanitizeRawTerminalText(value).trim();
    }
    for (const key of ["command_line", "command"]) {
      const value = object[key];
      if (typeof value === "string" && looksLikeCommand(value)) return firstCommandLine(value);
    }
    const commands = object.commands;
    if (Array.isArray(commands)) {
      const first = commands.find((item) => item && typeof item === "object") as
        | Record<string, unknown>
        | undefined;
      const command = first?.command_line ?? first?.command ?? first?.command_type;
      if (typeof command === "string" && command.trim())
        return sanitizeRawTerminalText(command).trim();
    }
  }
  return "";
}

function cleanToolText(value: string): string {
  return sanitizeRawTerminalText(value)
    .replace(/<br\s*\/?>/g, "\n")
    .replace(/^\s*\[command_run:\s*[^\r\n\]]*\]\s*$/gimu, "")
    .replace(/\[command_run:\s*[^\r\n\]]*\]/giu, "")
    .replace(/data:image\/[a-z0-9.+-]+;base64,[A-Za-z0-9+/=]+/gi, `[${t("imageData")}]`)
    .replace(/[A-Za-z0-9+/]{180,}={0,2}/g, `[${t("encodedData")}]`)
    .trim();
}

export function compactPayloadField(value: string): string | undefined {
  const normalized = normalizePayloadText(value);
  for (const key of ["task_group", "summary", "status", "label"]) {
    const index = normalized.indexOf(key);
    if (index < 0) continue;
    const colon = normalized.indexOf(":", index + key.length);
    const open = colon >= 0 ? normalized.indexOf('"', colon + 1) : -1;
    const close = open >= 0 ? normalized.indexOf('"', open + 1) : -1;
    if (open >= 0 && close > open)
      return truncate(
        normalized
          .slice(open + 1, close)
          .trim()
          .replace(/\s+/g, " "),
        90,
      );
  }
  for (const key of ["task_group", "summary", "status", "label"]) {
    const direct = normalized.match(new RegExp(`${key}\\\\*"?\\s*:\\s*\\\\*"([^"\\\\]+)`, "u"));
    if (direct?.[1]?.trim()) return truncate(direct[1].trim().replace(/\s+/g, " "), 90);
  }
  const start = normalized.indexOf("{");
  const end = normalized.lastIndexOf("}");
  if (start >= 0 && end > start) {
    const compact = compactCommandJson(normalized.slice(start, end + 1));
    if (compact) return compact;
  }
  for (const key of ["task_group", "summary", "status", "label"]) {
    const match = normalized.match(new RegExp(`\\\\*"${key}\\\\*"\\s*:\\s*\\\\*"([^"\\\\]+)`, "u"));
    if (match?.[1]?.trim()) return truncate(match[1].trim().replace(/\s+/g, " "), 90);
  }
  const loose = normalized.replace(/[\\"]/g, "");
  for (const key of ["task_group", "summary", "status", "label"]) {
    const index = loose.indexOf(`${key}:`);
    if (index < 0) continue;
    const rest = loose.slice(index + key.length + 1);
    const stop = rest.search(/[}\],]/u);
    const field = (stop >= 0 ? rest.slice(0, stop) : rest).trim();
    if (field) return truncate(field.replace(/\s+/g, " "), 90);
  }
  return undefined;
}

function normalizePayloadText(value: string): string {
  let normalized = sanitizeRawTerminalText(value);
  for (let index = 0; index < 5; index += 1) {
    const next = normalized.replace(/\\\\/g, "\\").replace(/\\"/g, '"');
    if (next === normalized) return normalized;
    normalized = next;
  }
  return normalized;
}

function compactCommandJson(value: string): string | undefined {
  const trimmed = value.trim();
  if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) return undefined;
  try {
    const parsed = JSON.parse(trimmed) as unknown;
    return compactCommandValue(parsed);
  } catch {
    return undefined;
  }
}

function compactCommandValue(value: unknown): string | undefined {
  if (Array.isArray(value)) {
    const nested = value.map(compactCommandValue).filter(Boolean).slice(0, 4);
    return nested.length ? nested.join("  ") : undefined;
  }
  if (!value || typeof value !== "object") return undefined;
  const object = value as Record<string, unknown>;
  const command = object.command ?? object.command_line;
  if (typeof command === "string" && command.trim()) return `[${t("bash")}: ${command.trim()}]`;
  const commandType = object.command_type;
  if (typeof commandType === "string" && commandType.trim())
    return `[${t("tool")}: ${commandType.trim()}]`;
  for (const key of ["task_group", "summary", "status", "label"]) {
    const value = object[key];
    if (typeof value === "string" && value.trim())
      return truncate(value.trim().replace(/\s+/g, " "), 90);
  }
  const output = object.output;
  if (typeof output === "string" && output.trim())
    return truncate(output.trim().replace(/\s+/g, " "), 90);
  if (output && typeof output === "object") {
    const nested = compactCommandValue(output);
    if (nested) return nested;
  }
  for (const key of ["results", "commands", "changes"]) {
    const nested = compactCommandValue(object[key]);
    if (nested) return nested;
  }
  return undefined;
}
