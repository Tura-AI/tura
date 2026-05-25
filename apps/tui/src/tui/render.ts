import type { AppState } from "./reducer.js";
import type { MessagePart } from "../types/session.js";
import { messageText, sessionTitle } from "../types/session.js";

const clear = "\x1b[2J\x1b[H";
const reset = "\x1b[0m";
const bold = "\x1b[1m";
const dim = "\x1b[2m";
const cyan = "\x1b[36m";
const magenta = "\x1b[35m";
const green = "\x1b[32m";
const yellow = "\x1b[33m";
const red = "\x1b[31m";

export function render(state: AppState): string {
  const rows = process.stdout.rows || 30;
  const cols = process.stdout.columns || 100;
  const lines: string[] = [];
  lines.push(`${clear}${bold}${magenta}Tura${reset} ${statusDot(state.status)} ${state.status} ${dim}${truncate(state.cwd, Math.max(12, cols - 28))}${reset}`);
  if (state.session) {
    const runtime = [
      state.session.agent,
      state.session.model,
      state.session.model_variant ? `variant:${state.session.model_variant}` : undefined,
      state.session.model_acceleration_enabled ? "priority" : undefined,
    ].filter(Boolean);
    lines.push(`${bold}${sessionTitle(state.session)}${reset} ${dim}${state.session.id}${reset} ${runtime.join(" ")}`.trim());
  } else {
    lines.push(`${yellow}No session selected${reset}`);
  }
  lines.push("─".repeat(cols));

  if (state.help) {
    lines.push(...helpLines());
  } else if (state.sessionsOpen) {
    lines.push(...sessionLines(state, cols, rows - 7));
  } else if (state.modelsOpen) {
    lines.push(...modelLines(state, rows - 7));
  } else if (state.diffOpen) {
    lines.push(`${bold}Diff${reset}`);
    lines.push(...wrap(state.diffText || "No diff.", cols).slice(0, rows - 7));
  } else {
    lines.push(...transcriptLines(state, cols, rows - 9));
  }

  if (state.todos.length) {
    lines.push("─".repeat(cols));
    lines.push(...state.todos.slice(0, 4).map((todo) => `${todoMark(todo.status)} ${truncate(todo.content ?? todo.title ?? todo.id ?? "", cols - 4)}`));
  }
  if (state.permissions.length) {
    lines.push("─".repeat(cols));
    for (const permission of state.permissions.slice(0, 3)) {
      lines.push(`${yellow}permission${reset} ${permission.id} ${permission.permission} ${dim}/approve ${permission.id} or /deny ${permission.id}${reset}`);
    }
  }
  if (state.questions.length) {
    lines.push("─".repeat(cols));
    for (const question of state.questions.slice(0, 3)) {
      lines.push(`${yellow}question${reset} ${question.id} ${truncate(question.question, Math.max(12, cols - 34))} ${dim}/answer ${question.id} <text>${reset}`);
    }
  }
  if (state.notice) lines.push(...wrap(`${dim}${state.notice}${reset}`, cols).slice(0, 3));
  lines.push("─".repeat(cols));
  lines.push(...composerLines(state.composer, cols));
  lines.push(`${dim}Enter send  Ctrl+J newline  /help  /sessions  /models  /diff  /quit${reset}`);
  return fit(lines, rows, cols).join("\n");
}

function transcriptLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines: string[] = [];
  for (const message of state.messages.slice(-24)) {
    const text = messageText(message).trim();
    const label = message.role === "assistant" ? `${magenta}assistant${reset}` : message.role === "user" ? `${cyan}user${reset}` : `${dim}${message.role}${reset}`;
    const partLines = message.parts.flatMap(partTranscriptLines);
    if (!text && partLines.length === 0) continue;
    lines.push(label);
    if (text) lines.push(...wrap(text, cols).map((line) => `  ${line}`));
    for (const partLine of partLines) lines.push(...wrap(partLine, cols).map((line) => `  ${line}`));
  }
  return lines.slice(Math.max(0, lines.length - maxLines));
}

function sessionLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = [`${bold}Sessions${reset}`, `${dim}Up/Down select  Enter resume  /new create  Esc close${reset}`];
  if (!state.sessions.length) {
    lines.push("No sessions found.");
    return lines;
  }
  for (const [index, session] of state.sessions.entries()) {
    const selected = index === state.selectedSessionIndex ? `${cyan}>${reset}` : " ";
    const current = session.id === state.session?.id ? `${green}current${reset}` : session.status ?? "idle";
    lines.push(`${selected} ${truncate(session.id, 14)}  ${current}  ${truncate(sessionTitle(session), Math.max(12, cols - 32))}`);
  }
  return lines.slice(0, maxLines);
}

function modelLines(state: AppState, maxLines: number): string[] {
  const lines = [`${bold}Models${reset}`, `${dim}Up/Down select  Enter set  /model provider/model also works.${reset}`];
  const providers = state.providers?.all ?? [];
  let row = 0;
  for (const provider of providers) {
    const defaults = state.providers?.default[provider.id];
    const connected = state.providers?.connected.includes(provider.id) ? green : dim;
    lines.push(`${connected}${provider.id}${reset} ${provider.name}`);
    for (const model of Object.keys(provider.models ?? {}).slice(0, 12)) {
      const selected = row === state.selectedModelIndex ? `${cyan}>${reset}` : " ";
      lines.push(`${selected} ${provider.id}/${model}${model === defaults ? ` ${dim}(default)${reset}` : ""}`);
      row += 1;
    }
  }
  if (lines.length === 2) lines.push("No providers returned by gateway.");
  return lines.slice(0, maxLines);
}

function helpLines(): string[] {
  return [
    `${bold}Help${reset}`,
    "/new                       create a session",
    "/resume <id>                switch session",
    "/model <provider/model>     set current session model",
    "/agent <name>               set current session agent",
    "/sessions                   show session picker hint",
    "/models                     show provider/model catalog",
    "/permissions                refresh permission requests",
    "/approve <id> /deny <id>    answer permission",
    "/command <name> [args...]   render gateway slash command",
    "/abort                      abort current turn",
    "/diff                       show gateway VCS diff",
    "/status                     refresh status",
    "/config get [key]            show workspace config",
    "/config set KEY=VALUE...     update workspace config",
    "/quit                       exit",
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

function composerLines(value: string, cols: number): string[] {
  const text = value || "";
  const lines = wrap(text, Math.max(20, cols - 3));
  if (lines.length === 0) return [`${cyan}>${reset} `];
  return lines.map((line, index) => `${index === 0 ? `${cyan}>${reset}` : " "} ${line}`);
}

function partTranscriptLines(part: MessagePart): string[] {
  if (part.type !== "tool") return [];
  const state = part.state && typeof part.state === "object" ? (part.state as Record<string, unknown>) : {};
  const status = typeof state.status === "string" ? state.status : "updated";
  const tool = part.tool ?? "tool";
  const summary = toolSummary(state);
  return [`${dim}${tool} ${status}${summary ? `: ${summary}` : ""}${reset}`];
}

function toolSummary(state: Record<string, unknown>): string {
  const output = state.output;
  if (typeof output === "string") return output.trim();
  if (output && typeof output === "object") {
    const object = output as Record<string, unknown>;
    for (const key of ["reply_message", "text", "summary", "stdout", "stderr"]) {
      const value = object[key];
      if (typeof value === "string" && value.trim()) return value.trim();
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

function fit(lines: string[], rows: number, cols: number): string[] {
  return lines.slice(0, rows).map((line) => truncate(line, cols + 32));
}

function truncate(text: string, width: number): string {
  return text.length > width ? `${text.slice(0, Math.max(0, width - 1))}…` : text;
}

function statusDot(status: string): string {
  if (status === "busy") return `${yellow}●${reset}`;
  if (status === "error") return `${red}●${reset}`;
  return `${green}●${reset}`;
}

function todoMark(status?: string): string {
  if (status === "completed") return `${green}✓${reset}`;
  if (status === "in_progress") return `${cyan}→${reset}`;
  if (status === "cancelled") return `${red}×${reset}`;
  return `${dim}•${reset}`;
}
