import type { AppState } from "./reducer.js";
import type { MessagePart } from "../types/session.js";
import { messageText, sessionPlanSummary, sessionStartAt, sessionStartCondition, sessionPlanStatus, sessionTitle } from "../types/session.js";
import { t } from "../i18n.js";

const clear = "\x1b[2J\x1b[H";
const reset = "\x1b[0m";
const bold = "\x1b[1m";
const italic = "\x1b[3m";
const underline = "\x1b[4m";
const strike = "\x1b[9m";
const inverse = "\x1b[7m";
const dim = "\x1b[2m";
const cyan = "\x1b[36m";
const magenta = "\x1b[35m";
const green = "\x1b[32m";
const yellow = "\x1b[33m";
const red = "\x1b[31m";
const ansiControlPattern = /^(?:\x1b\[[0-9;]*m|\x1b\]8;;[^\x1b]*\x1b\\)/;
const ansiControlGlobalPattern = /(?:\x1b\[[0-9;]*m|\x1b\]8;;[^\x1b]*\x1b\\)/g;

export function render(state: AppState): string {
  const rows = process.stdout.rows || 30;
  const cols = process.stdout.columns || 100;
  const lines: string[] = [];
  lines.push(`${clear}${topBar(state, cols)}`);
  lines.push(sessionBar(state, cols));
  lines.push(commandBar(state, cols));
  lines.push(rule(cols));

  if (state.help) {
    lines.push(...helpLines());
  } else if (state.sessionsOpen) {
    lines.push(...sessionLines(state, cols, rows - 7));
  } else if (state.authOpen) {
    lines.push(...authLines(state, cols, rows - 7));
  } else if (state.settingsOpen) {
    lines.push(...settingsLines(state, cols, rows - 7));
  } else if (state.planOpen) {
    lines.push(...planLines(state, cols, rows - 7));
  } else if (state.modelsOpen) {
    lines.push(...modelLines(state, rows - 7));
  } else if (state.diffOpen) {
    lines.push(`${bold}${t("diff")}${reset}`);
    lines.push(...wrap(state.diffText || t("noDiff"), cols).slice(0, rows - 7));
  } else {
    lines.push(...transcriptLines(state, cols, rows - 9));
  }

  if (state.todos.length) {
    lines.push(rule(cols));
    lines.push(...state.todos.slice(0, 4).map((todo) => `${todoMark(todo.status)} ${truncate(todo.content ?? todo.title ?? todo.id ?? "", cols - 4)}`));
  }
  if (state.permissions.length) {
    lines.push(rule(cols));
    for (const permission of state.permissions.slice(0, 3)) {
      lines.push(`${yellow}${t("permissions")}${reset} ${permission.id} ${permission.permission} ${dim}/approve ${permission.id} /deny ${permission.id}${reset}`);
    }
  }
  if (state.questions.length) {
    lines.push(rule(cols));
    for (const question of state.questions.slice(0, 3)) {
      lines.push(`${yellow}${t("question")}${reset} ${question.id} ${truncate(question.question, Math.max(12, cols - 34))} ${dim}${t("answerHint", { id: question.id })}${reset}`);
    }
  }
  if (state.notice) lines.push(...noticeLines(state.notice, cols));
  lines.push(rule(cols));
  lines.push(...composerLines(state.composer, cols));
  lines.push(`${dim}${t("enterSend")}  ${t("newline")}  ${t("closePanel")}  /auth  /settings  /sessions  /models  /quit${reset}`);
  return fit(lines, rows, cols).join("\n");
}

function topBar(state: AppState, cols: number): string {
  const title = `${bold}${magenta}Tura${reset} ${statusDot(state.status)} ${statusLabel(state.status)}`;
  const activity = [
    state.permissions.length ? `${yellow}${state.permissions.length} ${t("permissions")}${reset}` : undefined,
    state.questions.length ? `${yellow}${state.questions.length} ${t("question")}${reset}` : undefined,
    state.todos.length ? `${cyan}${state.todos.length} ${t("todo")}${reset}` : undefined,
  ].filter(Boolean).join("  ");
  const right = truncate(activity || state.cwd, Math.max(10, cols - visibleTextWidth(title) - 3));
  return `${title} ${dim}${right}${reset}`;
}

function sessionBar(state: AppState, cols: number): string {
  if (!state.session) return `${yellow}${t("noSessionSelected")}${reset}`;
  const runtime = [
    state.session.agent ?? state.sessionConfig?.active_agent,
    state.session.model ?? state.sessionConfig?.model ?? state.sessionConfig?.active_model,
    state.session.model_variant ?? state.sessionConfig?.model_variant,
    state.session.model_acceleration_enabled ?? state.sessionConfig?.model_acceleration_enabled ? t("priority") : undefined,
  ].filter(Boolean);
  const text = `${bold}${sessionTitle(state.session)}${reset} ${dim}${truncate(state.session.id, 12)}${reset} ${runtime.join(" ")}`;
  return truncate(text, cols + 32);
}

function commandBar(state: AppState, cols: number): string {
  const panel = state.authOpen ? "auth" : state.settingsOpen ? "settings" : state.sessionsOpen ? "sessions" : state.modelsOpen ? "models" : state.planOpen ? "plan" : state.diffOpen ? "diff" : "chat";
  const authSummary = providerSummary(state);
  const text = `${dim}${t("panel")}:${reset} ${cyan}${panelLabel(panel)}${reset}  ${dim}${t("gateway")}:${reset} ${authSummary}  ${dim}${t("cwd")}:${reset} ${truncate(state.cwd, 40)}`;
  return truncate(text, cols + 32);
}

function panelLabel(panel: "auth" | "settings" | "sessions" | "models" | "plan" | "diff" | "chat"): string {
  if (panel === "auth") return t("auth");
  if (panel === "settings") return t("settings");
  if (panel === "sessions") return t("sessions");
  if (panel === "models") return t("models");
  if (panel === "plan") return t("plan");
  if (panel === "diff") return t("diff");
  return t("chat");
}

function transcriptLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines: string[] = [];
  for (const message of state.messages.slice(-24)) {
    const text = displayMessageText(message.role, messageText(message));
    const label = message.role === "assistant" ? `${green}${t("assistant")}:${reset}` : message.role === "user" ? `${cyan}${t("user")}:${reset}` : `${dim}${message.role}:${reset}`;
    const partLines = message.parts.flatMap(partTranscriptLines);
    if (!text && partLines.length === 0) continue;
    const textLines = text ? wrapAnsi(renderRichText(text), Math.max(20, cols - 14)) : [];
    if (textLines.length) {
      lines.push(`${label} ${textLines[0]}`);
      for (const line of textLines.slice(1)) lines.push(`            ${line}`);
    } else {
      lines.push(label);
    }
    for (const partLine of partLines) lines.push(...wrap(partLine, Math.max(20, cols - 6)).map((line) => `  ${dim}${line}${reset}`));
  }
  return lines.slice(Math.max(0, lines.length - maxLines));
}

function sessionLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = [`${bold}${t("sessions")}${reset}`, `${dim}${t("selectSessions")}  ${t("enterResume")}  ${t("createSession")}  /resume <id> ${t("directSwitch")}${reset}`];
  if (!state.sessions.length) {
    lines.push(t("noSessions"));
    return lines;
  }
  for (const [index, session] of state.sessions.entries()) {
    const selected = index === state.selectedSessionIndex ? `${cyan}>${reset}` : " ";
    const current = session.id === state.session?.id ? `${green}${t("active")}${reset}` : session.status ?? t("sessionIdle");
    const meta = `${sessionPlanStatus(session)} ${current} ${formatTime(sessionStartAt(session))}`;
    lines.push(`${selected} ${truncate(session.id, 8)} ${pad(meta, 22)} ${truncate(sessionTitle(session), Math.max(12, cols - 34))}`);
  }
  return lines.slice(0, maxLines);
}

function authLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = [`${bold}${t("providerLogin")}${reset}`, `${dim}${t("loginProvider")} ${t("startsAuth")}  ${t("logoutProvider")}${reset}`];
  const providers = state.providers?.all ?? [];
  if (!providers.length) {
    lines.push(t("noProviders"));
    return lines;
  }
  for (const provider of providers) {
    const status = state.authStatuses[provider.id];
    const methods = state.authMethods?.[provider.id] ?? [];
    const connected = status?.authenticated || state.providers?.connected.includes(provider.id);
    const marker = connected ? `${green}${t("connected")}${reset}` : `${yellow}${t("needsLogin")}${reset}`;
    const source = provider.source ? ` ${dim}${provider.source}${reset}` : "";
    lines.push(`${connected ? green : yellow}${provider.id}${reset} ${provider.name} ${marker}${source}`);
    const statusText = [
      status?.login ? `${t("loginState")}:${status.login}` : undefined,
      status?.auth_state ? `${t("authState")}:${status.auth_state}` : undefined,
      status?.runtime_state ? `${t("runtime")}:${status.runtime_state}` : undefined,
      status?.account_id ? `${t("account")}:${status.account_id}` : undefined,
      status?.token_env ? `${t("env")}:${status.token_env}` : provider.env?.[0] ? `${t("env")}:${provider.env[0]}` : undefined,
    ].filter(Boolean).join("  ");
    if (statusText) lines.push(`  ${dim}${truncate(statusText, cols - 4)}${reset}`);
    if (methods.length) {
      for (const [index, method] of methods.slice(0, 4).entries()) {
        lines.push(`  ${cyan}${index}${reset} ${pad(method.label || method.login, 24)} ${dim}${method.type}${method.kind ? `/${method.kind}` : ""}${reset}`);
      }
    }
  }
  return lines.slice(0, maxLines);
}

function settingsLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = [`${bold}${t("sessionSettings")}${reset}`, `${dim}${t("configGet")}  ${t("configSet")}  /model provider/model  /agent ${t("agent")}${reset}`];
  const config = state.sessionConfig;
  if (!config) {
    lines.push(t("noSessionConfig"));
    return lines;
  }
  const rows = [
    [t("model"), config.model ?? config.active_model],
    [t("provider"), config.active_provider],
    [t("agent"), config.active_agent],
    [t("variant"), config.model_variant],
    [t("priority"), config.model_acceleration_enabled],
    [t("session"), config.session_type],
    [t("context"), config.context_message_limit],
    [t("validator"), config.validator_enabled],
    [t("stallGuard"), config.command_run_stall_guard_profile],
  ];
  for (const [key, value] of rows) {
    if (value === undefined || value === null || value === "") continue;
    lines.push(`${pad(String(key), 14)} ${truncate(String(value), cols - 18)}`);
  }
  return lines.slice(0, maxLines);
}

function planLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = [`${bold}${t("plan")}${reset}`, `${dim}${t("taskPlanHint")}${reset}`];
  const visible = state.sessions.filter((session) => sessionPlanStatus(session) !== "archived");
  const lanes = [
    ["todo", t("todo")],
    ["doing", t("doing")],
    ["question", t("question")],
    ["done", t("done")],
  ] as const;
  for (const [status, label] of lanes) {
    const lane = visible.filter((session) => sessionPlanStatus(session) === status);
    lines.push(`${bold}${label}${reset} ${dim}${lane.length}${reset}`);
    for (const session of lane.slice(0, 6)) {
      const title = truncate(sessionPlanSummary(session), Math.max(10, cols - 34));
      lines.push(`  ${truncate(session.id, 8)} ${title} ${dim}${shortCondition(sessionStartCondition(session))} ${formatTime(sessionStartAt(session))}${reset}`);
    }
  }
  const archived = state.sessions.filter((session) => sessionPlanStatus(session) === "archived").length;
  if (archived > 0) lines.push(`${dim}${t("archived")} ${archived}${reset}`);
  return lines.slice(0, maxLines);
}

function modelLines(state: AppState, maxLines: number): string[] {
  const lines = [`${bold}${t("models")}${reset}`, `${dim}${t("selectModels")}${reset}`];
  const providers = state.providers?.all ?? [];
  let row = 0;
  for (const provider of providers) {
    const defaults = state.providers?.default[provider.id];
    const connected = state.providers?.connected.includes(provider.id) ? green : dim;
    lines.push(`${connected}${provider.id}${reset} ${provider.name}`);
    for (const model of Object.keys(provider.models ?? {}).slice(0, 12)) {
      const selected = row === state.selectedModelIndex ? `${cyan}>${reset}` : " ";
      lines.push(`${selected} ${provider.id}/${model}${model === defaults ? ` ${dim}(${t("defaultModel")})${reset}` : ""}`);
      row += 1;
    }
  }
  if (lines.length === 2) lines.push(t("noProviders"));
  return lines.slice(0, maxLines);
}

function helpLines(): string[] {
  return [
    `${bold}${t("help")}${reset}`,
    `/chat                      ${t("helpChat")}`,
    `/new                       ${t("helpNew")}`,
    `/resume <id>                ${t("helpResume")}`,
    `/auth                       ${t("providerLogin")}`,
    `${t("loginProvider")}  ${t("helpLogin")}`,
    `${t("logoutProvider")}          ${t("helpLogout")}`,
    `/settings                   ${t("helpSettings")}`,
    `/model <provider/model>     ${t("helpModel")}`,
    `/agent <name>               ${t("agent")}`,
    `/sessions                   ${t("helpSessions")}`,
    `/plan                       ${t("helpPlan")}`,
    `/task <state> [id]          ${t("helpTask")}`,
    `/ticket <summary>           ${t("createTicket")}`,
    `/models                     ${t("helpModels")}`,
    `/permissions                ${t("permissions")}`,
    `/approve <id> /deny <id>    ${t("helpApprove")}`,
    `/command <name> [args...]   ${t("helpCommand")}`,
    `/abort                      ${t("helpAbort")}`,
    `/diff                       ${t("helpDiff")}`,
    `/status                     ${t("helpStatus")}`,
    `${t("configGet")}            ${t("helpConfigGet")}`,
    `${t("configSet")}...     ${t("helpConfigSet")}`,
    `/quit                       ${t("helpQuit")}`,
  ];
}

function wrap(text: string, cols: number): string[] {
  const width = Math.max(20, cols - 2);
  const result: string[] = [];
  for (const inputLine of text.split(/\r?\n/)) {
    let line = inputLine;
    while (line.length > width) {
      result.push(line.slice(0, width));
      line = line.slice(width);
    }
    result.push(line);
  }
  return result;
}

function wrapAnsi(text: string, cols: number): string[] {
  const width = Math.max(20, cols - 2);
  const result: string[] = [];
  for (const inputLine of text.split(/\r?\n/)) {
    let line = "";
    let visible = 0;
    for (let index = 0; index < inputLine.length; index += 1) {
      const char = inputLine[index];
      if (char === "\x1b") {
        const match = inputLine.slice(index).match(ansiControlPattern);
        if (match) {
          line += match[0];
          index += match[0].length - 1;
          continue;
        }
      }
      if (visible >= width) {
        result.push(line + reset);
        line = "";
        visible = 0;
      }
      line += char;
      visible += 1;
    }
    result.push(line);
  }
  return result;
}

function noticeLines(value: string, cols: number): string[] {
  const text = compactNotice(value);
  return wrap(`${dim}${text}${reset}`, cols).slice(0, 3);
}

function compactNotice(value: string): string {
  const trimmed = value.trim();
  if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) return trimmed;
  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const object = parsed as Record<string, unknown>;
      const pieces: string[] = [];
      for (const key of ["active_agent", "model", "active_model", "model_variant", "service_tier", "session_type"]) {
        const item = object[key];
        if (item !== undefined && item !== null && item !== "") pieces.push(`${key}:${String(item)}`);
      }
      if ("model_acceleration_enabled" in object) pieces.push(`${t("priority")}:${String(object.model_acceleration_enabled)}`);
      if (pieces.length) return `${t("settings")} ${pieces.join("  ")}`;
      if ("mano" in object || "router" in object || "lsp" in object) {
        return `${t("status")} mano:${serviceState(object.mano)}  ${t("router")}:${serviceState(object.router)}  lsp:${Array.isArray(object.lsp) ? object.lsp.length : 0}`;
      }
    }
  } catch {
    // Fall back to a short single-line JSON preview.
  }
  return trimmed.replace(/\s+/g, " ");
}

function serviceState(value: unknown): string {
  if (value && typeof value === "object") {
    const object = value as Record<string, unknown>;
    return String(object.status ?? object.error ?? t("unknown"));
  }
  return t("unknown");
}

function composerLines(value: string, cols: number): string[] {
  const text = value || "";
  const lines = wrap(text, Math.max(20, cols - 3));
  if (lines.length === 0) return [`${cyan}>${reset} `];
  return lines.map((line, index) => `${index === 0 ? `${cyan}>${reset}` : " "} ${line}`);
}

function displayMessageText(role: string, value: string): string {
  let text = cleanMessageText(value);
  if (!text) return "";
  const payloadSummary = summarizePayloadText(text);
  if (payloadSummary) return payloadSummary;
  if (role === "user") {
    const first = text.split(/\r?\n/).find((line) => line.trim()) ?? text;
    return truncate(first.trim(), 140);
  }
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.trim())
    .slice(0, 14);
  return lines.join("\n");
}

function cleanMessageText(value: string): string {
  return value
    .replace(/<br\s*\/?>/g, "\n")
    .replace(/data:image\/[a-z0-9.+-]+;base64,[A-Za-z0-9+/=]+/gi, `[${t("imageData")}]`)
    .replace(/[A-Za-z0-9+/]{180,}={0,2}/g, `[${t("encodedData")}]`)
    .trim();
}

function renderRichText(source: string): string {
  if (!source) return "";
  const tokenized = source.replace(/\[(MEDIA):([\s\S]*?):MEDIA\]|\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu, (_match, media, path, mode, emoji) => {
    if (media) return renderMediaToken(String(path).trim());
    return mode === "react" ? `${dim}[${t("react")}: ${String(emoji).trim()}]${reset}` : String(emoji).trim();
  });
  return renderHtmlSubset(tokenized);
}

function renderHtmlSubset(source: string): string {
  let output = source;
  output = output.replace(/<pre(?:\s[^>]*)?>\s*<code(?:\s+class=['"]language-([^'"]+)['"])?>([\s\S]*?)<\/code>\s*<\/pre>/giu, (_match, language, body) => {
    const title = language ? `${dim}[${t("code")}: ${decodeHtml(language)}]${reset}` : `${dim}[${t("code")}]${reset}`;
    return `${title}\n${renderCodeBlock(decodeHtml(body))}`;
  });
  output = output.replace(/<blockquote>([\s\S]*?)<\/blockquote>/giu, (_match, body) =>
    decodeHtml(stripHtml(body))
      .split(/\r?\n/)
      .map((line) => `${dim}│ ${line}${reset}`)
      .join("\n"),
  );
  const replacements: Array<[RegExp, (body: string, attr?: string) => string]> = [
    [/<(?:b|strong)>([\s\S]*?)<\/(?:b|strong)>/giu, (body) => `${bold}${renderHtmlSubset(body)}${reset}`],
    [/<(?:i|em)>([\s\S]*?)<\/(?:i|em)>/giu, (body) => `${italic}${renderHtmlSubset(body)}${reset}`],
    [/<u>([\s\S]*?)<\/u>/giu, (body) => `${underline}${renderHtmlSubset(body)}${reset}`],
    [/<(?:s|del)>([\s\S]*?)<\/(?:s|del)>/giu, (body) => `${strike}${renderHtmlSubset(body)}${reset}`],
    [/<code>([\s\S]*?)<\/code>/giu, (body) => `${cyan}${decodeHtml(stripHtml(body))}${reset}`],
    [/<span\s+class=['"]tg-spoiler['"]>([\s\S]*?)<\/span>/giu, (body) => `${inverse}${decodeHtml(stripHtml(body))}${reset}`],
    [/<a\s+href=['"](https?:\/\/[^'"]+)['"][^>]*>([\s\S]*?)<\/a>/giu, (body, href) => `${terminalLink(href ?? "", `${underline}${renderHtmlSubset(body)}${reset}`)} ${dim}(${href})${reset}`],
  ];
  let changed = true;
  while (changed) {
    changed = false;
    for (const [pattern, format] of replacements) {
      output = output.replace(pattern, (match, first, second) => {
        changed = true;
        if (pattern.source.startsWith("<a")) return format(second, decodeHtml(first));
        return format(first);
      });
    }
  }
  return decodeHtml(stripUnsupportedHtml(output));
}

function renderCodeBlock(value: string): string {
  return value
    .replace(/\r\n/g, "\n")
    .split("\n")
    .map((line) => `${dim}  ${line}${reset}`)
    .join("\n");
}

function renderMediaToken(path: string): string {
  const label = `[${t("media")}: ${path}]`;
  return isSafeUrl(path) ? terminalLink(path, `${dim}${label}${reset}`) : `${dim}${label}${reset}`;
}

function terminalLink(url: string, label: string): string {
  if (!isSafeUrl(url)) return label;
  return `\x1b]8;;${url}\x1b\\${label}\x1b]8;;\x1b\\`;
}

function stripHtml(value: string): string {
  return stripUnsupportedHtml(value).replace(/<[^>]+>/gu, "");
}

function stripUnsupportedHtml(value: string): string {
  return value.replace(/<br\s*\/?>/giu, "\n").replace(/<\/?(?:p|div)>/giu, "\n").replace(/<\/?[^>]+>/gu, "");
}

function isSafeUrl(value: string): boolean {
  return /^https?:\/\//iu.test(value);
}

function decodeHtml(value: string): string {
  return value
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&")
    .replace(/&quot;/g, "\"")
    .replace(/&#39;/g, "'");
}

function summarizePayloadText(value: string): string | undefined {
  if (!/[{[]/.test(value) || !/(command_run|apply_patch|image_url|command_type|tool_result|results)/i.test(value)) return undefined;
  const snippets: string[] = [];
  for (const match of value.matchAll(/"path"\s*:\s*"([^"]+)"/g)) snippets.push(`[${t("read")}: ${match[1]}]`);
  for (const match of value.matchAll(/"command_line"\s*:\s*"([^"]+)"/g)) snippets.push(`[${t("bash")}: ${match[1]}]`);
  for (const match of value.matchAll(/"command"\s*:\s*"([^"]+)"/g)) snippets.push(`[${t("bash")}: ${match[1]}]`);
  for (const match of value.matchAll(/"command_type"\s*:\s*"([^"]+)"/g)) snippets.push(`[${t("tool")}: ${match[1]}]`);
  for (const match of value.matchAll(/"label"\s*:\s*"([^"]+)"/g)) {
    if (/img|image/i.test(match[1])) snippets.push(`[${t("media")}: ${match[1]}]`);
  }
  const unique = Array.from(new Set(snippets)).slice(0, 8);
  if (unique.length) return unique.join("\n");
  return `[${t("toolResult")}]`;
}

function partTranscriptLines(part: MessagePart): string[] {
  if (part.type !== "tool") return [];
  const state = part.state && typeof part.state === "object" ? (part.state as Record<string, unknown>) : {};
  const status = typeof state.status === "string" ? state.status : t("updated");
  const tool = part.tool ?? t("tool");
  const summary = truncateAnsi(renderRichText(toolSummary(state)), 88);
  return [`[${tool}: ${summary || status}]`];
}

function toolSummary(state: Record<string, unknown>): string {
  const output = state.output;
  if (typeof output === "string") {
    const clean = cleanMessageText(output);
    return summarizePayloadText(clean) ?? compactCommandJson(clean) ?? clean;
  }
  if (output && typeof output === "object") {
    const object = output as Record<string, unknown>;
    for (const key of ["reply_message", "text", "summary", "stdout", "stderr"]) {
      const value = object[key];
      if (typeof value === "string" && value.trim()) {
        const clean = cleanMessageText(value);
        return summarizePayloadText(clean) ?? compactCommandJson(clean) ?? clean;
      }
    }
  }
  const input = state.input;
  if (input && typeof input === "object") {
    const object = input as Record<string, unknown>;
    for (const key of ["step_summary", "command_line", "command"]) {
      const value = object[key];
      if (typeof value === "string" && value.trim()) return value.trim();
    }
    const commands = object.commands;
    if (Array.isArray(commands)) {
      const first = commands.find((item) => item && typeof item === "object") as Record<string, unknown> | undefined;
      const command = first?.command_line ?? first?.command ?? first?.command_type;
      if (typeof command === "string" && command.trim()) return command.trim();
    }
  }
  return "";
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
  if (typeof commandType === "string" && commandType.trim()) return `[${t("tool")}: ${commandType.trim()}]`;
  const output = object.output;
  if (typeof output === "string" && output.trim()) return truncate(output.trim().replace(/\s+/g, " "), 90);
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

function fit(lines: string[], rows: number, cols: number): string[] {
  return lines.slice(0, rows).map((line) => truncateAnsi(line, cols));
}

function truncate(text: string, width: number): string {
  return text.length > width ? `${text.slice(0, Math.max(0, width - 1))}…` : text;
}

function truncateAnsi(text: string, width: number): string {
  let visible = 0;
  let output = "";
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (char === "\x1b") {
      const match = text.slice(index).match(ansiControlPattern);
      if (match) {
        output += match[0];
        index += match[0].length - 1;
        continue;
      }
    }
    if (visible >= Math.max(0, width - 1)) return `${output}…${reset}`;
    output += char;
    visible += 1;
  }
  return output;
}

function rule(cols: number): string {
  return "─".repeat(cols);
}

function pad(text: string, width: number): string {
  return text.length >= width ? truncate(text, width) : `${text}${" ".repeat(width - text.length)}`;
}

function visibleTextWidth(text: string): number {
  return text.replace(ansiControlGlobalPattern, "").length;
}

function providerSummary(state: AppState): string {
  const providers = state.providers?.all ?? [];
  if (!providers.length) return `${dim}${t("unknown")}${reset}`;
  const connected = providers.filter((provider) => state.authStatuses[provider.id]?.authenticated || state.providers?.connected.includes(provider.id));
  return connected.length ? `${green}${connected.length}/${providers.length} ${t("connected")}${reset}` : `${yellow}${t("loginNeeded")}${reset}`;
}

function statusDot(status: string): string {
  if (status === "busy") return `${yellow}●${reset}`;
  if (status === "error") return `${red}●${reset}`;
  return `${green}●${reset}`;
}

function statusLabel(status: string): string {
  if (status === "busy") return t("busy");
  if (status === "error") return t("error");
  if (status === "idle") return t("sessionIdle");
  return status;
}

function todoMark(status?: string): string {
  if (status === "completed") return `${green}✓${reset}`;
  if (status === "in_progress") return `${cyan}→${reset}`;
  if (status === "cancelled") return `${red}×${reset}`;
  return `${dim}•${reset}`;
}

function shortCondition(value: string): string {
  if (value === "scheduled_task") return t("scheduledTask");
  if (value === "polling_task") return t("pollingTask");
  if (value === "session_idle") return t("sessionIdle");
  return t("userAction");
}

function formatTime(value: string | number | undefined): string {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "-";
  return `${date.getMonth() + 1}/${date.getDate()} ${date.getHours()}:${String(date.getMinutes()).padStart(2, "0")}`;
}
