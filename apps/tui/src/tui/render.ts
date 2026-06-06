import type { AppState } from "./reducer.js";
import type { Message, MessagePart } from "../types/session.js";
import { messageText, sessionTitle } from "../types/session.js";
import { t } from "../i18n.js";
import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";
import {
  activeCapabilities,
  bold,
  clear,
  cyan,
  dim,
  fit,
  gray,
  green,
  magenta,
  pad,
  red,
  reset,
  rule,
  setActiveCapabilities,
  stripAnsi,
  truncate,
  truncateAnsi,
  underline,
  visibleTextWidth,
  wrap,
  wrapAnsi,
  yellow,
} from "./render-terminal.js";
import {
  compactInlinePayloads,
  compactPayloadField,
  displayMessageText,
  extractCommandsFromText,
  extractCommandsFromUnknown,
  firstCommandLine,
  renderRichText,
  toolSummary,
} from "./render-rich-text.js";

type CommandInfo = {
  command: string;
  tool?: string;
  status?: string;
};

export function render(
  state: AppState,
  capabilities: TerminalCapabilities = detectTerminalCapabilities(),
): string {
  setActiveCapabilities(capabilities);
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
      lines.push(
        `${yellow}${t("permissions")}${reset} ${permission.id} ${permission.permission} ${dim}/approve ${permission.id} /deny ${permission.id}${reset}`,
      );
    }
  }
  if (state.questions.length) {
    lines.push(rule(cols));
    for (const question of state.questions.slice(0, 3)) {
      lines.push(
        `${yellow}${t("question")}${reset} ${question.id} ${truncate(question.question, Math.max(12, cols - 34))} ${dim}${t("answerHint", { id: question.id })}${reset}`,
      );
    }
  }
  if (state.notice) lines.push(...noticeLines(state.notice, cols));
  lines.push(rule(cols));
  lines.push(...composerLines(state.composer, cols));
  lines.push(
    `${dim}${t("enterSend")}  ${t("newline")}  ${t("closePanel")}  /commands  /auth  /settings  /personas  /sessions  /models  /quit${reset}`,
  );
  return fit(lines, rows, cols).join("\n");
}

function renderPlain(state: AppState): string {
  const cols = process.stdout.columns || 100;
  const lines: string[] = [];
  lines.push(`Tura ${statusLabel(state.status)} ${truncate(stripAnsi(state.cwd), cols - 12)}`);
  lines.push(
    state.session
      ? `${sessionTitle(state.session)} ${truncate(state.session.id, 12)}`
      : t("noSessionSelected"),
  );
  lines.push(
    `${t("panel")}: ${plainPanel(state)}  ${t("cwd")}: ${truncate(state.cwd, Math.max(20, cols - 14))}`,
  );
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
    state.permissions.length
      ? `${yellow}${state.permissions.length} ${t("permissions")}${reset}`
      : undefined,
    state.questions.length
      ? `${yellow}${state.questions.length} ${t("question")}${reset}`
      : undefined,
  ]
    .filter(Boolean)
    .join("  ");
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
    (state.session.model_acceleration_enabled ?? state.sessionConfig?.model_acceleration_enabled)
      ? t("priority")
      : undefined,
  ].filter(Boolean);
  const text = `${bold}${sessionTitle(state.session)}${reset} ${dim}${truncate(state.session.id, 12)}${reset} ${runtime.join(" ")}`;
  return truncate(text, cols + 32);
}

function commandBar(state: AppState, cols: number): string {
  const panel = state.authOpen
    ? "auth"
    : state.settingsOpen
      ? "settings"
      : state.personasOpen
        ? "personas"
        : state.sessionsOpen
          ? "sessions"
          : state.modelsOpen
            ? "models"
            : "chat";
  const text = `${dim}${t("panel")}:${reset} ${cyan}${panelLabel(panel)}${reset}  ${agentPersonaSummary(state)}  ${dim}${t("cwd")}:${reset} ${truncate(state.cwd, 40)}`;
  return truncate(text, cols + 32);
}

function panelLabel(
  panel: "auth" | "settings" | "personas" | "sessions" | "models" | "chat",
): string {
  if (panel === "auth") return t("auth");
  if (panel === "settings") return t("settings");
  if (panel === "personas") return t("personas");
  if (panel === "sessions") return t("sessions");
  if (panel === "models") return t("models");
  return t("chat");
}

function transcriptLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines: string[] = [];
  const showCommands = Boolean(state.sessionConfig?.show_command_instructions);
  for (const message of state.messages.slice(-24)) {
    const text = displayMessageText(message.role, messageText(message));
    const label = roleLabel(message.role);
    const partLines = message.parts.flatMap(partTranscriptLines);
    const commands = commandsForMessage(message);
    if (!text && partLines.length === 0 && commands.length === 0) continue;
    addTranscriptGap(lines);
    const primary = message.role === "assistant";
    const richText = text ? renderRichText(text) : "";
    const displayText = primary ? richText : secondaryText(stripAnsi(richText));
    const textLines = displayText ? wrapAnsi(displayText, Math.max(20, cols - 14)) : [];
    if (textLines.length) {
      lines.push(`${label} ${textLines[0]}`);
      for (const line of textLines.slice(1)) lines.push(`            ${line}`);
    } else {
      lines.push(label);
    }
    for (const partLine of partLines)
      lines.push(
        ...wrapAnsi(secondaryText(partLine), Math.max(20, cols - 6)).map((line) => `  ${line}`),
      );
    if (message.role === "assistant" && commands.length) {
      addTranscriptGap(lines);
      lines.push(commandSummaryLine(commands, state.commandDetailsOpen, cols));
      if (state.commandDetailsOpen || (showCommands && activeCapabilities.level !== "rich")) {
        for (const line of commandDetailLines(commands, cols)) lines.push(line);
      }
    }
  }
  if (isThinking(state)) {
    addTranscriptGap(lines);
    lines.push(thinkingLine(state, cols));
  }
  return lines.slice(Math.max(0, lines.length - maxLines));
}

function addTranscriptGap(lines: string[]): void {
  if (lines.length && lines.at(-1) !== "") lines.push("");
}

function roleLabel(role: string): string {
  if (role === "assistant") return `${green}${t("assistant")}:${reset}`;
  if (role === "user") return secondaryText(`${t("user")}:`);
  return `${dim}${t("system")}:${reset}`;
}

function secondaryText(value: string): string {
  if (!value) return value;
  return `${gray}${value.replaceAll(reset, `${reset}${gray}`)}${reset}`;
}

function commandsForMessage(message: Message): CommandInfo[] {
  const commands = [
    ...extractCommandsFromText(messageText(message)).map((command) => ({
      command,
      tool: t("assistant"),
    })),
    ...message.parts.flatMap(commandsForPart),
  ];
  const seen = new Set<string>();
  const unique: CommandInfo[] = [];
  for (const item of commands) {
    const command = firstCommandLine(item.command);
    if (!command || seen.has(command)) continue;
    seen.add(command);
    unique.push({ ...item, command });
  }
  return unique.slice(0, 24);
}

function commandsForPart(part: MessagePart): CommandInfo[] {
  const state =
    part.state && typeof part.state === "object" ? (part.state as Record<string, unknown>) : {};
  const status = typeof state.status === "string" ? state.status : undefined;
  const tool = part.tool ?? t("tool");
  return [
    ...extractCommandsFromUnknown(state.input).map((command) => ({ command, tool, status })),
    ...extractCommandsFromUnknown(state.output).map((command) => ({ command, tool, status })),
    ...extractCommandsFromUnknown(part.metadata).map((command) => ({ command, tool, status })),
  ];
}

function commandSummaryLine(commands: CommandInfo[], expanded: boolean, cols: number): string {
  const count = `${t("commands")}: ${commands.length}`;
  const running = commands.some((command) =>
    /run|progress|pending|busy|question/i.test(command.status ?? ""),
  );
  const icon = activeCapabilities.unicode
    ? running
      ? "■"
      : expanded
        ? "┬"
        : "◇"
    : running
      ? "#"
      : expanded
        ? "+"
        : "*";
  const label = `${icon} ${count}`;
  const visible = commandToggleLabel(label);
  return secondaryText(truncateAnsi(visible, Math.max(12, cols - 2)));
}

function commandToggleLabel(label: string): string {
  if (activeCapabilities.level === "plain") return label;
  const visible = `${underline}${label}${reset}`;
  if (activeCapabilities.level !== "rich" || !activeCapabilities.osc8) return visible;
  return `\x1b]8;;tura://commands/toggle\x1b\\${visible}\x1b]8;;\x1b\\`;
}

function commandDetailLines(commands: CommandInfo[], cols: number): string[] {
  const lines: string[] = [];
  for (const [index, command] of commands.entries()) {
    const isLast = index === commands.length - 1;
    const branch = activeCapabilities.unicode ? (isLast ? "└─" : "├─") : "|-";
    const stem = activeCapabilities.unicode ? (isLast ? "   " : "│  ") : "|  ";
    const symbol = statusSymbol(command.status);
    const meta = [command.tool ?? t("tool"), command.status].filter(Boolean).join(" ");
    lines.push(
      secondaryText(`${branch} ${stripAnsi(symbol)} #${index + 1}${meta ? ` ${meta}` : ""}`),
    );
    const prefix = "$ ";
    for (const line of wrapAnsi(
      secondaryText(`${prefix}${command.command}`),
      Math.max(20, cols - 2),
    )) {
      lines.push(`${secondaryText(stem)}${line}`);
    }
  }
  return lines;
}

function statusSymbol(status: string | undefined): string {
  const normalized = (status ?? "").toLowerCase();
  if (/fail|error|reject|denied/.test(normalized))
    return `${red}${activeCapabilities.unicode ? "✕" : "x"}${reset}`;
  if (/run|progress|pending|busy|question/.test(normalized))
    return activeCapabilities.unicode ? "■" : "#";
  if (/done|complete|success|ok/.test(normalized))
    return `${green}${activeCapabilities.unicode ? "✓" : "+"}${reset}`;
  return `${dim}${activeCapabilities.unicode ? "•" : "-"}${reset}`;
}

function isThinking(state: AppState): boolean {
  return state.status === "busy" || state.session?.status === "busy";
}

function thinkingLine(state: AppState, cols: number): string {
  const frames = activeCapabilities.unicode
    ? ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
    : ["|", "/", "-", "\\"];
  const frame = frames[state.thinkingFrame % frames.length] ?? ".";
  const commands = state.messages.slice(-24).flatMap(commandsForMessage);
  const suffix = commands.length ? `  ${t("commands")}: ${commands.length}` : "";
  return secondaryText(
    `${frame} thinking${suffix ? `  ${truncate(suffix.trim(), Math.max(12, cols - 14))}` : ""}`,
  );
}

function sessionLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = [
    `${bold}${t("sessions")}${reset}`,
    `${dim}${t("selectSessions")}  ${t("enterResume")}  ${t("createSession")}  /resume <id> ${t("directSwitch")}${reset}`,
  ];
  if (!state.sessions.length) {
    lines.push(t("noSessions"));
    return lines;
  }
  for (const [index, session] of state.sessions.entries()) {
    const selected = index === state.selectedSessionIndex ? `${cyan}>${reset}` : " ";
    const current =
      session.id === state.session?.id
        ? `${green}${t("active")}${reset}`
        : (session.status ?? t("sessionIdle"));
    const meta = `${current} ${formatTime(session.updated_at ?? session.created_at)}`;
    lines.push(
      `${selected} ${truncate(session.id, 8)} ${pad(meta, 22)} ${truncate(sessionTitle(session), Math.max(12, cols - 34))}`,
    );
  }
  return lines.slice(0, maxLines);
}

function authLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = [
    `${bold}${t("providerLogin")}${reset}`,
    `${dim}${t("loginProvider")} ${t("startsAuth")}  ${t("logoutProvider")}${reset}`,
  ];
  const providers = state.providers?.all ?? [];
  if (!providers.length) {
    lines.push(t("noProviders"));
    return lines;
  }
  for (const provider of providers) {
    const status = state.authStatuses[provider.id];
    const methods = state.authMethods?.[provider.id] ?? [];
    const connected = status?.authenticated || state.providers?.connected.includes(provider.id);
    const marker = connected
      ? `${green}${t("connected")}${reset}`
      : `${yellow}${t("needsLogin")}${reset}`;
    const source = provider.source ? ` ${dim}${provider.source}${reset}` : "";
    lines.push(
      `${connected ? green : yellow}${provider.id}${reset} ${provider.name} ${marker}${source}`,
    );
    const statusText = [
      status?.login ? `${t("loginState")}:${status.login}` : undefined,
      status?.auth_state ? `${t("authState")}:${status.auth_state}` : undefined,
      status?.runtime_state ? `${t("runtime")}:${status.runtime_state}` : undefined,
      status?.account_id ? `${t("account")}:${status.account_id}` : undefined,
      status?.token_env
        ? `${t("env")}:${status.token_env}`
        : provider.env?.[0]
          ? `${t("env")}:${provider.env[0]}`
          : undefined,
    ]
      .filter(Boolean)
      .join("  ");
    if (statusText) lines.push(`  ${dim}${truncate(statusText, cols - 4)}${reset}`);
    if (methods.length) {
      for (const [index, method] of methods.slice(0, 4).entries()) {
        const availability = method.available === false ? ` ${yellow}unavailable${reset}` : "";
        lines.push(
          `  ${cyan}${index}${reset} ${pad(method.label || method.login, 24)} ${dim}${method.type}${method.kind ? `/${method.kind}` : ""}${reset}${availability}`,
        );
      }
    }
  }
  return lines.slice(0, maxLines);
}

function settingsLines(state: AppState, cols: number, maxLines: number): string[] {
  const lines = [
    `${bold}${t("sessionSettings")}${reset}`,
    `${dim}${t("configGet")}  ${t("configSet")}  /model provider/model  /agent ${t("agent")}  /persona ${t("persona")}${reset}`,
  ];
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
    ["show_command_instructions", config.show_command_instructions ?? false],
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
    const marker =
      id === active ? `${green}${t("active")}${reset}` : (persona.summary?.source ?? "");
    const description =
      persona.summary?.description ?? stringField(persona.config, "description") ?? "";
    lines.push(
      `${selected} ${pad(id, 18)} ${pad(marker, 12)} ${truncate(description, Math.max(12, cols - 36))}`,
    );
    const style =
      typeof persona.communication_style === "string" ? persona.communication_style.trim() : "";
    if (style)
      lines.push(`  ${dim}${truncate(style.replace(/\s+/g, " "), Math.max(20, cols - 4))}${reset}`);
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
      lines.push(
        `${selected} ${provider.id}/${model}${model === defaults ? ` ${dim}(${t("defaultModel")})${reset}` : ""}`,
      );
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
    `/commands                  ${t("helpCommands")}`,
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
      for (const key of [
        "active_agent",
        "model",
        "active_model",
        "model_variant",
        "service_tier",
        "session_type",
      ]) {
        const item = object[key];
        if (item !== undefined && item !== null && item !== "")
          pieces.push(`${key}:${String(item)}`);
      }
      if ("model_acceleration_enabled" in object)
        pieces.push(`${t("priority")}:${String(object.model_acceleration_enabled)}`);
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

function partTranscriptLines(part: MessagePart): string[] {
  if (part.type !== "tool") return [];
  if (commandsForPart(part).length) return [];
  const state =
    part.state && typeof part.state === "object" ? (part.state as Record<string, unknown>) : {};
  const status = typeof state.status === "string" ? state.status : t("updated");
  const tool = part.tool ?? t("tool");
  const rawSummary = toolSummary(state);
  const compactSummary = compactPayloadField(rawSummary) ?? compactInlinePayloads(rawSummary);
  const summary = truncateAnsi(renderRichText(compactSummary), 88);
  return [`[${tool}: ${summary || status}]`];
}

function alignedRows(rows: Array<[unknown, unknown]>, cols: number): string[] {
  const visibleRows = rows
    .filter(([, value]) => value !== undefined && value !== null && value !== "")
    .map(([key, value]) => [String(key), String(value)] as const);
  const keyWidth = Math.min(22, Math.max(8, ...visibleRows.map(([key]) => visibleTextWidth(key))));
  return visibleRows.map(
    ([key, value]) =>
      `${pad(key, keyWidth)}  ${truncate(value, Math.max(12, cols - keyWidth - 3))}`,
  );
}

function personaID(persona: AppState["personas"][number]): string | undefined {
  const configName = persona.config?.persona_name;
  return persona.summary?.id ?? (typeof configName === "string" ? configName : undefined);
}

function activePersonaID(state: AppState): string | undefined {
  const agentID = state.session?.agent ?? state.sessionConfig?.active_agent;
  const agent = state.agents.find((item) => storedAgentID(item) === agentID);
  const first = Array.isArray(agent?.config?.agent_persona)
    ? agent?.config?.agent_persona[0]
    : undefined;
  if (first && typeof first === "object" && !Array.isArray(first)) {
    const name = (first as Record<string, unknown>).persona_name;
    if (typeof name === "string" && name.trim()) return name.trim();
  }
  const runtimePersonas = (
    agent as unknown as { options?: { personas?: AppState["personas"] } } | undefined
  )?.options?.personas;
  return runtimePersonas?.[0] ? personaID(runtimePersonas[0]) : undefined;
}

function storedAgentID(agent: AppState["agents"][number]): string | undefined {
  return agent.summary?.id ?? (agent as unknown as { name?: string }).name;
}

function stringField(value: Record<string, unknown> | undefined, key: string): string | undefined {
  const item = value?.[key];
  return typeof item === "string" ? item : undefined;
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
