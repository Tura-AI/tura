import type { AppState } from "./reducer.js";
import type { MessagePart } from "../types/session.js";
import { messageText, sessionTitle } from "../types/session.js";
import { t } from "../i18n.js";
import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";

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
const osc8FullPattern = /\x1b\]8;;[^\x1b]*\x1b\\[\s\S]*?\x1b\]8;;\x1b\\/g;
let activeCapabilities: TerminalCapabilities = detectTerminalCapabilities();

export function render(state: AppState, capabilities: TerminalCapabilities = detectTerminalCapabilities()): string {
  activeCapabilities = capabilities;
  if (capabilities.level === "plain") return renderPlain(state);
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
  } else if (state.personasOpen) {
    lines.push(...personaLines(state, cols, rows - 7));
  } else if (state.modelsOpen) {
    lines.push(...modelLines(state, rows - 7));
  } else {
    lines.push(...transcriptLines(state, cols, rows - 9));
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
  lines.push(`${dim}${t("enterSend")}  ${t("newline")}  ${t("closePanel")}  /auth  /settings  /personas  /sessions  /models  /quit${reset}`);
  return fit(lines, rows, cols).join("\n");
}

function renderPlain(state: AppState): string {
  const cols = process.stdout.columns || 100;
  const lines: string[] = [];
  lines.push(`Tura ${statusLabel(state.status)} ${truncate(stripAnsi(state.cwd), cols - 12)}`);
  lines.push(state.session ? `${sessionTitle(state.session)} ${truncate(state.session.id, 12)}` : t("noSessionSelected"));
  lines.push(`${t("panel")}: ${plainPanel(state)}  ${t("cwd")}: ${truncate(state.cwd, Math.max(20, cols - 14))}`);
  lines.push(rule(cols));
  if (state.help) lines.push(...helpLines());
  else if (state.sessionsOpen) lines.push(...sessionLines(state, cols, 20));
  else if (state.authOpen) lines.push(...authLines(state, cols, 20));
  else if (state.settingsOpen) lines.push(...settingsLines(state, cols, 20));
  else if (state.personasOpen) lines.push(...personaLines(state, cols, 20));
  else if (state.modelsOpen) lines.push(...modelLines(state, 20));
  else lines.push(...transcriptLines(state, cols, 20));
  if (state.notice) lines.push(...noticeLines(state.notice, cols));
  lines.push(rule(cols));
  lines.push(...composerLines(state.composer, cols));
  return stripAnsi(lines.map((line) => truncateAnsi(line, cols)).join("\n"));
}

function plainPanel(state: AppState): string {
  if (state.authOpen) return t("auth");
  if (state.settingsOpen) return t("settings");
  if (state.personasOpen) return t("personas");
  if (state.sessionsOpen) return t("sessions");
  if (state.modelsOpen) return t("models");
  return t("chat");
}

function topBar(state: AppState, cols: number): string {
  const title = `${bold}${magenta}Tura${reset} ${statusDot(state.status)} ${statusLabel(state.status)}`;
  const activity = [
    state.permissions.length ? `${yellow}${state.permissions.length} ${t("permissions")}${reset}` : undefined,
    state.questions.length ? `${yellow}${state.questions.length} ${t("question")}${reset}` : undefined,
  ].filter(Boolean).join("  ");
  const right = truncate(activity || state.cwd, Math.max(10, cols - visibleTextWidth(title) - 3));
  return `${title} ${dim}${right}${reset}`;
}

function sessionBar(state: AppState, cols: number): string {
  if (!state.session) return `${yellow}${t("noSessionSelected")}${reset}`;
  const runtime = [
    state.session.agent ?? state.sessionConfig?.active_agent,
    activePersonaID(state),
    state.session.model ?? state.sessionConfig?.model ?? state.sessionConfig?.active_model,
    state.session.model_variant ?? state.sessionConfig?.model_variant,
    state.session.model_acceleration_enabled ?? state.sessionConfig?.model_acceleration_enabled ? t("priority") : undefined,
  ].filter(Boolean);
  const text = `${bold}${sessionTitle(state.session)}${reset} ${dim}${truncate(state.session.id, 12)}${reset} ${runtime.join(" ")}`;
  return truncate(text, cols + 32);
}

function commandBar(state: AppState, cols: number): string {
  const panel = state.authOpen ? "auth" : state.settingsOpen ? "settings" : state.personasOpen ? "personas" : state.sessionsOpen ? "sessions" : state.modelsOpen ? "models" : "chat";
  const text = `${dim}${t("panel")}:${reset} ${cyan}${panelLabel(panel)}${reset}  ${agentPersonaSummary(state)}  ${dim}${t("cwd")}:${reset} ${truncate(state.cwd, 40)}`;
  return truncate(text, cols + 32);
}

function panelLabel(panel: "auth" | "settings" | "personas" | "sessions" | "models" | "chat"): string {
  if (panel === "auth") return t("auth");
  if (panel === "settings") return t("settings");
  if (panel === "personas") return t("personas");
  if (panel === "sessions") return t("sessions");
  if (panel === "models") return t("models");
  return t("chat");
}

function transcriptLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines: string[] = [];
  for (const message of state.messages.slice(-24)) {
    const text = displayMessageText(message.role, messageText(message));
    const label = roleLabel(message.role);
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

function roleLabel(role: string): string {
  if (role === "assistant") return `${green}${t("assistant")}:${reset}`;
  if (role === "user") return `${cyan}${t("user")}:${reset}`;
  return `${dim}${t("system")}:${reset}`;
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
    const meta = `${current} ${formatTime(session.updated_at ?? session.created_at)}`;
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
        const availability = method.available === false ? ` ${yellow}unavailable${reset}` : "";
        lines.push(`  ${cyan}${index}${reset} ${pad(method.label || method.login, 24)} ${dim}${method.type}${method.kind ? `/${method.kind}` : ""}${reset}${availability}`);
      }
    }
  }
  return lines.slice(0, maxLines);
}

function settingsLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = [`${bold}${t("sessionSettings")}${reset}`, `${dim}${t("configGet")}  ${t("configSet")}  /model provider/model  /agent ${t("agent")}  /persona ${t("persona")}${reset}`];
  const config = state.sessionConfig;
  if (!config) {
    lines.push(t("noSessionConfig"));
    return lines;
  }
  const rows: Array<[string, unknown]> = [
    [t("model"), config.model ?? config.active_model],
    [t("provider"), config.active_provider],
    [t("agent"), config.active_agent],
    [t("persona"), activePersonaID(state)],
    [t("variant"), config.model_variant],
    [t("priority"), config.model_acceleration_enabled],
    [t("session"), config.session_type],
    [t("context"), config.context_message_limit],
    [t("validator"), config.validator_enabled],
    [t("stallGuard"), config.command_run_stall_guard_profile],
  ];
  lines.push(...alignedRows(rows, cols));
  return lines.slice(0, maxLines);
}

function personaLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = [`${bold}${t("personas")}${reset}`, `${dim}${t("selectPersonas")}${reset}`];
  if (!state.personas.length) {
    lines.push(t("noPersonas"));
    return lines;
  }
  const active = activePersonaID(state);
  for (const [index, persona] of state.personas.entries()) {
    const id = personaID(persona) ?? t("unknown");
    const selected = index === state.selectedPersonaIndex ? `${cyan}>${reset}` : " ";
    const marker = id === active ? `${green}${t("active")}${reset}` : persona.summary?.source ?? "";
    const description = persona.summary?.description ?? stringField(persona.config, "description") ?? "";
    lines.push(`${selected} ${pad(id, 18)} ${pad(marker, 12)} ${truncate(description, Math.max(12, cols - 36))}`);
    const style = typeof persona.communication_style === "string" ? persona.communication_style.trim() : "";
    if (style) lines.push(`  ${dim}${truncate(style.replace(/\s+/g, " "), Math.max(20, cols - 4))}${reset}`);
  }
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
    `/personas                   ${t("personas")}`,
    `/persona <name>             ${t("applyPersona")}`,
    `/sessions                   ${t("helpSessions")}`,
    `/models                     ${t("helpModels")}`,
    `/abort                      ${t("helpAbort")}`,
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
  if (payloadSummary) return "";
  if (/completed without a user-facing message/i.test(text)) return "";
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
  if (activeCapabilities.richText === "none") return plainRichText(source);
  if (activeCapabilities.richText === "basicMarkdown") return basicRichText(source);
  const tokenized = source.replace(/\[(MEDIA):([\s\S]*?):MEDIA\]|\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu, (_match, media, path, mode, emoji) => {
    if (media) return renderMediaToken(String(path).trim());
    return mode === "react" ? `${dim}[EMOJI:react:${String(emoji).trim()}:EMOJI]${reset}` : String(emoji).trim();
  });
  return renderInlineMarkdown(renderMarkdownTables(renderHtmlSubset(tokenized)));
}

function plainRichText(source: string): string {
  return renderMarkdownTables(decodeHtml(
    stripUnsupportedHtml(
      source
        .replace(/<a\s+href=['"]((?:https?:\/\/|file:\/\/)[^'"]+)['"][^>]*>([\s\S]*?)<\/a>/giu, (_match, href, body) => `${stripHtml(String(body))} (${href})`)
        .replace(/\[([^\]\n]+)\]\(([^)\s]+)\)/gu, "$1 ($2)")
        .replace(/\[MEDIA:([\s\S]*?):MEDIA\]/gu, "[MEDIA:$1:MEDIA]")
        .replace(/\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu, (_match, mode, emoji) => `[EMOJI:${mode}:${emojiFallbackName(String(emoji).trim())}:EMOJI]`),
    ),
  ));
}

function basicRichText(source: string): string {
  const tokenized = source.replace(/\[(MEDIA):([\s\S]*?):MEDIA\]|\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu, (_match, media, path, _mode, emoji) => {
    if (media) return `[MEDIA:${String(path).trim()}:MEDIA]`;
    return String(emoji).trim();
  });
  return renderInlineMarkdown(renderMarkdownTables(renderHtmlSubset(tokenized)));
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
      .map((line) => `${dim}${activeCapabilities.unicode ? "│" : ">"} ${line}${reset}`)
      .join("\n"),
  );
  const replacements: Array<[RegExp, (body: string, attr?: string) => string]> = [
    [/<(?:b|strong)>([\s\S]*?)<\/(?:b|strong)>/giu, (body) => `${bold}${renderHtmlSubset(body)}${reset}`],
    [/<(?:i|em)>([\s\S]*?)<\/(?:i|em)>/giu, (body) => `${italic}${renderHtmlSubset(body)}${reset}`],
    [/<u>([\s\S]*?)<\/u>/giu, (body) => `${underline}${renderHtmlSubset(body)}${reset}`],
    [/<(?:s|del)>([\s\S]*?)<\/(?:s|del)>/giu, (body) => `${strike}${renderHtmlSubset(body)}${reset}`],
    [/<code>([\s\S]*?)<\/code>/giu, (body) => `${cyan}${decodeHtml(stripHtml(body))}${reset}`],
    [/<span\s+class=['"]tg-spoiler['"]>([\s\S]*?)<\/span>/giu, (body) => `${inverse}${decodeHtml(stripHtml(body))}${reset}`],
    [/<a\s+href=['"]((?:https?:\/\/|file:\/\/)[^'"]+)['"][^>]*>([\s\S]*?)<\/a>/giu, (body, href) => renderLinkTarget(href ?? "", renderHtmlSubset(body))],
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

function renderMarkdownTables(source: string): string {
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const output: string[] = [];
  for (let index = 0; index < lines.length;) {
    if (isMarkdownTableStart(lines, index)) {
      const table: string[][] = [tableCells(lines[index])];
      index += 2;
      while (index < lines.length && /^\s*\|.*\|\s*$/u.test(lines[index])) {
        table.push(tableCells(lines[index]));
        index += 1;
      }
      output.push(...formatMarkdownTable(table));
      continue;
    }
    output.push(lines[index]);
    index += 1;
  }
  return output.join("\n");
}

function isMarkdownTableStart(lines: string[], index: number): boolean {
  return index + 1 < lines.length &&
    /^\s*\|.*\|\s*$/u.test(lines[index]) &&
    /^\s*\|?\s*:?-{3,}:?\s*(?:\|\s*:?-{3,}:?\s*)+\|?\s*$/u.test(lines[index + 1]);
}

function tableCells(line: string): string[] {
  return line.trim().replace(/^\|/u, "").replace(/\|$/u, "").split("|").map((cell) => cell.trim());
}

function formatMarkdownTable(rows: string[][]): string[] {
  const width = Math.max(...rows.map((row) => row.length));
  const normalized = rows.map((row) => Array.from({ length: width }, (_item, index) => row[index] ?? ""));
  const widths = Array.from({ length: width }, (_item, column) =>
    Math.min(48, Math.max(3, ...normalized.map((row) => visibleTextWidth(row[column])))),
  );
  return normalized.map((row, index) => {
    const cells = row.map((cell, column) => pad(truncate(cell, widths[column]), widths[column]));
    const text = ` ${cells.join("  ")} `;
    return index === 0 ? `${bold}${text}${reset}` : text;
  });
}

function renderInlineMarkdown(source: string): string {
  const linked = source.replace(/\[([^\]\n]+)\]\(([^)\s]+)\)/gu, (_match, label, href) =>
    renderLinkTarget(String(href), String(label)),
  );
  return linkLocalPathsPreservingOsc(linked);
}

function renderMediaToken(path: string): string {
  const label = `[MEDIA:${path}:MEDIA]`;
  return isLinkTarget(path) ? terminalLink(linkTargetUrl(path), `${dim}${label}${reset}`) : `${dim}${label}${reset}`;
}

function emojiFallbackName(value: string): string {
  const known: Record<string, string> = {
    "👍": "thumbs_up",
    "👎": "thumbs_down",
    "😂": "face_with_tears_of_joy",
    "😀": "grinning_face",
    "🙂": "slightly_smiling_face",
    "😊": "smiling_face",
    "❤️": "red_heart",
    "✅": "check_mark",
    "❌": "cross_mark",
    "🔥": "fire",
    "🚀": "rocket",
  };
  if (known[value]) return known[value];
  const fallback = Array.from(value)
    .map((char) => `u+${char.codePointAt(0)?.toString(16) ?? "unknown"}`)
    .join("_");
  return fallback || "unknown";
}

function renderLinkTarget(target: string, label: string): string {
  if (!isLinkTarget(target)) return `${label} (${target})`;
  const visible = `${underline}${label}${reset}`;
  return `${terminalLink(linkTargetUrl(target), visible)} ${dim}(${target})${reset}`;
}

const LOCAL_PATH_PATTERN =
  /(?:[A-Za-z]:[\\/][^\s<>"'`]+|\\\\[^\\/\s<>"'`]+\\[^\\/\s<>"'`]+(?:\\[^\s<>"'`]+)*|\/[A-Za-z0-9_.-]+(?:\/[A-Za-z0-9_.-]+)+|\.{1,2}[\\/][^\s<>"'`]+)/gu;
const TRAILING_PATH_PUNCTUATION = /[),.;:!?]+$/u;

function linkLocalPaths(source: string): string {
  return source.replace(LOCAL_PATH_PATTERN, (raw, offset: number) => {
    if (/^[A-Za-z]:[\\/]/u.test(raw) && offset > 0 && /[A-Za-z0-9]/u.test(source[offset - 1])) return raw;
    const path = raw.replace(TRAILING_PATH_PUNCTUATION, "");
    const trailing = raw.slice(path.length);
    if (!isLocalPath(path)) return raw;
    return `${terminalLink(linkTargetUrl(path), `${underline}${path}${reset}`)}${trailing}`;
  });
}

function linkLocalPathsPreservingOsc(source: string): string {
  let cursor = 0;
  let output = "";
  for (const match of source.matchAll(osc8FullPattern)) {
    const index = match.index ?? 0;
    output += linkLocalPaths(source.slice(cursor, index));
    output += match[0];
    cursor = index + match[0].length;
  }
  output += linkLocalPaths(source.slice(cursor));
  return output;
}

function isLocalPath(value: string): boolean {
  return /^(?:[A-Za-z]:[\\/]|\\\\|\/|\.{1,2}[\\/])/u.test(value);
}

function terminalLink(url: string, label: string): string {
  if (!isLinkTarget(url) || !activeCapabilities.osc8) return label;
  return `\x1b]8;;${url}\x1b\\${label}\x1b]8;;\x1b\\`;
}

function stripHtml(value: string): string {
  return stripUnsupportedHtml(value).replace(/<[^>]+>/gu, "");
}

function stripUnsupportedHtml(value: string): string {
  return value.replace(/<br\s*\/?>/giu, "\n").replace(/<\/?(?:p|div)>/giu, "\n").replace(/<\/?[^>]+>/gu, "");
}

function isLinkTarget(value: string): boolean {
  return /^(?:https?:\/\/|file:\/\/)/iu.test(value) || isLocalPath(value);
}

function linkTargetUrl(value: string): string {
  if (/^(?:https?:\/\/|file:\/\/)/iu.test(value)) return value;
  return localPathUrl(value);
}

function localPathUrl(value: string): string {
  const normalized = value.replace(/\\/g, "/");
  const withSlash = /^[A-Za-z]:\//u.test(normalized) ? `/${normalized}` : normalized;
  return `file://${encodeURI(withSlash)}`;
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
  const ellipsis = activeCapabilities.unicode ? "…" : "...";
  return text.length > width ? `${text.slice(0, Math.max(0, width - ellipsis.length))}${ellipsis}` : text;
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
    if (visible >= Math.max(0, width - 1)) return `${output}${activeCapabilities.unicode ? "…" : "..."}${reset}`;
    output += char;
    visible += 1;
  }
  return output;
}

function rule(cols: number): string {
  return (activeCapabilities.unicode ? "─" : "-").repeat(cols);
}

function pad(text: string, width: number): string {
  return text.length >= width ? truncate(text, width) : `${text}${" ".repeat(width - text.length)}`;
}

function alignedRows(rows: Array<[unknown, unknown]>, cols: number): string[] {
  const visibleRows = rows
    .filter(([, value]) => value !== undefined && value !== null && value !== "")
    .map(([key, value]) => [String(key), String(value)] as const);
  const keyWidth = Math.min(22, Math.max(8, ...visibleRows.map(([key]) => visibleTextWidth(key))));
  return visibleRows.map(([key, value]) => `${pad(key, keyWidth)}  ${truncate(value, Math.max(12, cols - keyWidth - 3))}`);
}

function personaID(persona: AppState["personas"][number]): string | undefined {
  const configName = persona.config?.persona_name;
  return persona.summary?.id ?? (typeof configName === "string" ? configName : undefined);
}

function activePersonaID(state: AppState): string | undefined {
  const agentID = state.session?.agent ?? state.sessionConfig?.active_agent;
  const agent = state.agents.find((item) => storedAgentID(item) === agentID);
  const first = Array.isArray(agent?.config?.agent_persona) ? agent?.config?.agent_persona[0] : undefined;
  if (first && typeof first === "object" && !Array.isArray(first)) {
    const name = (first as Record<string, unknown>).persona_name;
    if (typeof name === "string" && name.trim()) return name.trim();
  }
  const runtimePersonas = (agent as unknown as { options?: { personas?: AppState["personas"] } } | undefined)?.options?.personas;
  return runtimePersonas?.[0] ? personaID(runtimePersonas[0]) : undefined;
}

function storedAgentID(agent: AppState["agents"][number]): string | undefined {
  return agent.summary?.id ?? (agent as unknown as { name?: string }).name;
}

function stringField(value: Record<string, unknown> | undefined, key: string): string | undefined {
  const item = value?.[key];
  return typeof item === "string" ? item : undefined;
}

function visibleTextWidth(text: string): number {
  return text.replace(ansiControlGlobalPattern, "").length;
}

function stripAnsi(text: string): string {
  return text.replace(ansiControlGlobalPattern, "");
}

function agentPersonaSummary(state: AppState): string {
  const agent = state.session?.agent ?? state.sessionConfig?.active_agent;
  const persona = activePersonaID(state);
  const parts = [
    `${dim}${t("agent")}:${reset} ${agent ? `${green}${agent}${reset}` : `${yellow}${t("unknown")}${reset}`}`,
    `${dim}${t("persona")}:${reset} ${persona ? `${green}${persona}${reset}` : `${dim}-${reset}`}`,
  ];
  return parts.join("  ");
}

function statusDot(status: string): string {
  const dot = activeCapabilities.unicode ? "●" : "*";
  if (status === "busy") return `${yellow}${dot}${reset}`;
  if (status === "error") return `${red}${activeCapabilities.unicode ? dot : "!"}${reset}`;
  return `${green}${dot}${reset}`;
}

function statusLabel(status: string): string {
  if (status === "busy") return t("busy");
  if (status === "error") return t("error");
  if (status === "idle") return t("sessionIdle");
  return status;
}

function formatTime(value: string | number | undefined): string {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "-";
  return `${date.getMonth() + 1}/${date.getDate()} ${date.getHours()}:${String(date.getMinutes()).padStart(2, "0")}`;
}
