import { emitKeypressEvents } from "node:readline";
import { setTimeout as delay } from "node:timers/promises";
import { existsSync, statSync } from "node:fs";
import { basename } from "node:path";
import { GatewayClient } from "../gateway/client.js";
import { ensureGatewayAvailable } from "../gateway/autostart.js";
import { MockGatewayClient } from "../gateway/mock-client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import type { Session } from "../types/session.js";
import type { ProviderAuthStatus } from "../types/provider.js";
import { sessionUpdatedAt } from "../types/session.js";
import { promptPayload } from "../commands/run.js";
import { sessionConfigPatchFromAssignments } from "../commands/config-values.js";
import { initialState, reducer, type AppState } from "./reducer.js";
import { render } from "./render.js";
import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";
import { t } from "../i18n.js";

type TuiGatewayClient = GatewayClient | MockGatewayClient;

export async function runTui(context: CliContext, initialPrompt?: string): Promise<void> {
  if (!process.stdin.isTTY || !process.stdout.isTTY) {
    throw new CliUsageError(t("tuiRequiresTty"));
  }
  const capabilities = detectTerminalCapabilities(context.display);
  const client = context.mock
    ? new MockGatewayClient({ directory: context.cwd })
    : new GatewayClient({
        baseUrl: context.gatewayUrl,
        directory: context.cwd,
        verbose: context.verbose,
      });
  if (!context.mock) {
    await ensureGatewayAvailable(context.gatewayUrl, capabilities);
    await client.health();
    await client.syncWorkspace();
  }
  const session = await pickInitialSession(client);
  let state = await hydrate(initialState(context.cwd), client, session);
  let lastFrame = "";
  const dispatch = (action: Parameters<typeof reducer>[1]) => {
    state = reducer(state, action);
    lastFrame = draw(state, capabilities, lastFrame);
  };

  lastFrame = draw(state, capabilities, lastFrame);
  const controller = new AbortController();
  if (!context.mock) {
    void eventLoop(client, controller.signal, dispatch);
    void pollingLoop(client, () => state, dispatch, controller.signal);
  }
  const thinkingTimer = setInterval(() => {
    if (state.status === "busy" || state.session?.status === "busy") dispatch({ type: "tick" });
  }, 350);

  if (initialPrompt?.trim()) {
    await submitPrompt(client, () => state, dispatch, initialPrompt);
  }

  await inputLoop(client, () => state, dispatch, capabilities);
  clearInterval(thinkingTimer);
  controller.abort();
  if (process.stdin.isTTY) process.stdin.setRawMode(false);
  if (capabilities.cursorControl)
    process.stdout.write(
      `${capabilities.level === "rich" ? "\x1b[?1000l\x1b[?1006l" : ""}\x1b[?25h\x1b[0m\n`,
    );
  else process.stdout.write("\n");
}

async function pickInitialSession(client: TuiGatewayClient): Promise<Session> {
  const sessions = await client.listSessions({ limit: 20 });
  sessions.sort((left, right) => sessionUpdatedAt(right) - sessionUpdatedAt(left));
  return sessions[0] ?? client.createSession();
}

async function hydrate(
  state: AppState,
  client: TuiGatewayClient,
  session: Session,
): Promise<AppState> {
  const [messages, providers, sessionConfig, agents, personas] = await Promise.all([
    client.listMessages(session.id).catch(() => []),
    client.listProviders().catch(() => undefined),
    client.getSessionConfig().catch(() => undefined),
    client.listAgents().catch(() => []),
    client.listPersonas().catch(() => []),
  ]);
  const auth = providers
    ? await fetchAuthSurface(
        client,
        providers.all.map((provider) => provider.id),
      )
    : {};
  const sessions = await client.listSessions({ includeChildren: true, limit: 50 }).catch(() => []);
  return reducer(
    reducer(state, {
      type: "hydrate",
      session,
      messages,
      permissions: [],
      providers,
      agents,
      personas,
      sessions,
      authMethods: auth.methods,
      authStatuses: auth.statuses,
      sessionConfig,
    }),
    {
      type: "questions",
      value: [],
    },
  );
}

async function eventLoop(
  client: TuiGatewayClient,
  signal: AbortSignal,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
): Promise<void> {
  while (!signal.aborted) {
    try {
      for await (const event of client.streamEvents(signal)) {
        dispatch({ type: "event", event });
      }
    } catch (error) {
      if (signal.aborted) return;
      dispatch({
        type: "notice",
        value: t("eventStreamReconnecting", {
          error: error instanceof Error ? error.message : String(error),
        }),
      });
      await delay(1000);
    }
  }
}

async function pollingLoop(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  signal: AbortSignal,
): Promise<void> {
  while (!signal.aborted) {
    const sessionID = getState().session?.id;
    if (sessionID) {
      const messages = await client.listMessages(sessionID).catch(() => undefined);
      const session = getState().session;
      if (messages && session) {
        dispatch({
          type: "hydrate",
          session,
          messages,
          permissions: getState().permissions,
          providers: getState().providers,
          agents: getState().agents,
          personas: getState().personas,
          sessions: getState().sessions,
        });
      }
    }
    await delay(1500);
  }
}

async function inputLoop(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  capabilities: TerminalCapabilities,
): Promise<void> {
  emitKeypressEvents(process.stdin);
  if (process.stdin.isTTY && capabilities.interactive) process.stdin.setRawMode(true);
  if (capabilities.cursorControl)
    process.stdout.write(
      `${capabilities.level === "rich" ? "\x1b[?1000h\x1b[?1006h" : ""}\x1b[?25l`,
    );
  return new Promise((resolve) => {
    const onResize = () => dispatch({ type: "notice", value: getState().notice });
    const onKeypress = async (
      text: string,
      key: { name?: string; ctrl?: boolean; meta?: boolean } | undefined,
    ) => {
      const state = getState();
      const sequence = keySequence(key) ?? text ?? "";
      if (capabilities.level === "rich" && isMouseClickSequence(sequence)) {
        if (
          !state.help &&
          !state.sessionsOpen &&
          !state.modelsOpen &&
          !state.authOpen &&
          !state.settingsOpen &&
          !state.personasOpen
        ) {
          dispatch({ type: "toggle-command-details" });
        }
        return;
      }
      if (key?.ctrl && key.name === "c") {
        process.stdin.off("keypress", onKeypress);
        process.stdout.off("resize", onResize);
        resolve();
        return;
      }
      if (key?.name === "escape") {
        if (state.help) dispatch({ type: "toggle-help" });
        if (state.sessionsOpen) dispatch({ type: "toggle-sessions" });
        if (state.modelsOpen) dispatch({ type: "toggle-models" });
        if (state.authOpen) dispatch({ type: "toggle-auth" });
        if (state.settingsOpen) dispatch({ type: "toggle-settings" });
        if (state.personasOpen) dispatch({ type: "toggle-personas" });
        return;
      }
      if (key?.name === "up" || key?.name === "down") {
        const delta = key.name === "up" ? -1 : 1;
        if (state.sessionsOpen) dispatch({ type: "select-session", delta });
        else if (state.modelsOpen) dispatch({ type: "select-model", delta });
        else if (state.personasOpen) dispatch({ type: "select-persona", delta });
        return;
      }
      if (key?.name === "return") {
        if (state.sessionsOpen && !state.composer.trim()) {
          const target = state.sessions[state.selectedSessionIndex];
          if (target) {
            const next = await hydrate(getState(), client, target);
            dispatch({
              type: "hydrate",
              session: next.session!,
              messages: next.messages,
              permissions: next.permissions,
              providers: next.providers,
              agents: next.agents,
              personas: next.personas,
              sessions: next.sessions,
            });
            dispatch({ type: "questions", value: next.questions });
          }
          return;
        }
        if (state.modelsOpen && !state.composer.trim()) {
          const model = selectedModel(state);
          const sessionID = state.session?.id;
          if (model && sessionID) {
            const session = await client.updateSession(sessionID, { model });
            dispatch({
              type: "hydrate",
              session,
              messages: state.messages,
              permissions: state.permissions,
              providers: state.providers,
              agents: state.agents,
              personas: state.personas,
              sessions: state.sessions,
            });
          }
          return;
        }
        if (state.personasOpen && !state.composer.trim()) {
          const persona = selectedPersonaID(state);
          if (persona) {
            try {
              await applyPersonaToActiveAgent(client, getState, dispatch, persona);
            } catch (error) {
              dispatch({
                type: "notice",
                value: error instanceof Error ? error.message : String(error),
              });
            }
          }
          return;
        }
        const value = state.composer.trim();
        dispatch({ type: "composer", value: "" });
        if (!value) return;
        if (value.startsWith("/")) {
          const shouldExit = await slashCommand(client, getState, dispatch, value);
          if (shouldExit) {
            process.stdin.off("keypress", onKeypress);
            process.stdout.off("resize", onResize);
            resolve();
          }
        } else {
          await submitPrompt(client, getState, dispatch, value);
        }
        return;
      }
      if (key?.ctrl && key.name === "j") {
        dispatch({ type: "composer", value: `${state.composer}\n` });
        return;
      }
      if (key?.ctrl && key.name === "o") {
        dispatch({ type: "toggle-command-details" });
        return;
      }
      if (key?.name === "backspace") {
        dispatch({ type: "composer", value: state.composer.slice(0, -1) });
        return;
      }
      if (key?.name === "tab") {
        dispatch({ type: "composer", value: completeSlash(state.composer) });
        return;
      }
      const printable = text ?? printableSequence(keySequence(key));
      if (printable && !key?.ctrl && !key?.meta) {
        dispatch({ type: "composer", value: state.composer + printable });
      }
    };
    process.stdin.on("keypress", onKeypress);
    process.stdout.on("resize", onResize);
  });
}

function isMouseClickSequence(sequence: string): boolean {
  return /\x1b\[<\d+;\d+;\d+M/u.test(sequence);
}

function printableSequence(sequence: string | undefined): string | undefined {
  if (!sequence || sequence.length !== 1) return undefined;
  const code = sequence.charCodeAt(0);
  return code >= 0x20 && code !== 0x7f ? sequence : undefined;
}

function keySequence(
  key: { name?: string; ctrl?: boolean; meta?: boolean } | undefined,
): string | undefined {
  return typeof (key as { sequence?: unknown } | undefined)?.sequence === "string"
    ? (key as { sequence: string }).sequence
    : undefined;
}

async function slashCommand(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  input: string,
): Promise<boolean> {
  const [name, ...args] = input.slice(1).trim().split(/\s+/).filter(Boolean);
  if (!name || name === "help") dispatch({ type: "toggle-help" });
  else if (name === "chat") dispatch({ type: "close-panels" });
  else if (name === "commands") dispatch({ type: "toggle-command-details" });
  else if (name === "quit" || name === "exit") return true;
  else if (name === "new") {
    const session = await client.createSession();
    const next = await hydrate(getState(), client, session);
    dispatch({
      type: "hydrate",
      session: next.session!,
      messages: next.messages,
      permissions: next.permissions,
      providers: next.providers,
      agents: next.agents,
      personas: next.personas,
      sessions: next.sessions,
    });
    dispatch({ type: "questions", value: next.questions });
  } else if (name === "resume") {
    const id = args[0];
    if (!id) dispatch({ type: "notice", value: t("usageResume") });
    else {
      const session = await client.getSession(id);
      const next = await hydrate(getState(), client, session);
      dispatch({
        type: "hydrate",
        session: next.session!,
        messages: next.messages,
        permissions: next.permissions,
        providers: next.providers,
        agents: next.agents,
        personas: next.personas,
        sessions: next.sessions,
      });
      dispatch({ type: "questions", value: next.questions });
    }
  } else if (name === "sessions") {
    dispatch({
      type: "sessions",
      value: await client.listSessions({ includeChildren: true, limit: 50 }),
      open: true,
    });
  } else if (name === "models") dispatch({ type: "toggle-models" });
  else if (name === "personas") {
    dispatch({
      type: "personas",
      value: await client.listPersonas().catch(() => getState().personas),
      open: true,
    });
  } else if (name === "auth" || name === "login") {
    const providerID = args[0];
    if (!providerID || name === "auth") {
      const providers = await client.listProviders().catch(() => getState().providers);
      const ids = providers?.all.map((provider) => provider.id) ?? [];
      const auth = await fetchAuthSurface(client, ids);
      dispatch({ type: "auth", methods: auth.methods, statuses: auth.statuses, open: true });
      if (name === "login" && !providerID) dispatch({ type: "notice", value: t("usageLogin") });
    } else {
      const method = Number(args[1] ?? "0");
      const auth = await client.providerOauthAuthorize(
        providerID,
        Number.isFinite(method) ? method : 0,
      );
      const status = await client.providerAuthStatus(providerID).catch(() => undefined);
      dispatch({
        type: "auth",
        statuses: status
          ? { ...getState().authStatuses, [providerID]: status }
          : getState().authStatuses,
        open: true,
      });
      dispatch({
        type: "notice",
        value: [auth.instructions, auth.url ? t("openUrl", { url: auth.url }) : undefined]
          .filter(Boolean)
          .join(" "),
      });
    }
  } else if (name === "logout") {
    const providerID = args[0];
    if (!providerID) dispatch({ type: "notice", value: t("usageLogout") });
    else {
      await client.providerLogout(providerID);
      const status = await client.providerAuthStatus(providerID).catch(() => undefined);
      dispatch({
        type: "auth",
        statuses: status
          ? { ...getState().authStatuses, [providerID]: status }
          : getState().authStatuses,
        open: true,
      });
    }
  } else if (name === "model") {
    const model = args[0];
    const sessionID = getState().session?.id;
    if (model && sessionID) {
      const session = await client.updateSession(sessionID, { model });
      dispatch({
        type: "hydrate",
        session,
        messages: getState().messages,
        permissions: getState().permissions,
        providers: getState().providers,
        agents: getState().agents,
        personas: getState().personas,
        sessions: getState().sessions,
      });
    }
  } else if (name === "agent") {
    const agent = args[0];
    const sessionID = getState().session?.id;
    if (agent && sessionID) {
      const session = await client.updateSession(sessionID, { agent });
      dispatch({
        type: "hydrate",
        session,
        messages: getState().messages,
        permissions: getState().permissions,
        providers: getState().providers,
        agents: getState().agents,
        personas: getState().personas,
        sessions: getState().sessions,
      });
    }
  } else if (name === "persona") {
    const persona = args[0];
    if (!persona) {
      dispatch({
        type: "personas",
        value: await client.listPersonas().catch(() => getState().personas),
        open: true,
      });
    } else {
      try {
        await applyPersonaToActiveAgent(client, getState, dispatch, persona);
      } catch (error) {
        dispatch({ type: "notice", value: error instanceof Error ? error.message : String(error) });
      }
    }
  } else if (name === "abort") {
    const sessionID = getState().session?.id;
    if (sessionID) await client.abort(sessionID);
  } else if (name === "settings") {
    dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
  } else if (name === "config") {
    const subcommand = args.shift() ?? "get";
    if (subcommand === "set") {
      if (args.length === 0) dispatch({ type: "notice", value: t("usageConfigSet") });
      else {
        const config = await client.patchSessionConfig(sessionConfigPatchFromAssignments(args));
        dispatch({ type: "session-config", value: config, open: true });
        dispatch({ type: "notice", value: t("settingsUpdated") });
      }
    } else if (subcommand === "get") {
      const config = await client.getSessionConfig();
      const key = args[0];
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: JSON.stringify(key ? config[key] : config) });
    } else {
      dispatch({ type: "notice", value: t("usageConfig") });
    }
  } else {
    dispatch({ type: "notice", value: t("unknownCommand", { command: `/${name}` }) });
  }
  return false;
}

async function submitPrompt(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  prompt: string,
): Promise<void> {
  const session = getState().session ?? (await client.createSession());
  await client.sendPromptAsync(
    session.id,
    promptPayload(richPromptFromInput(prompt), {
      source: "tui",
      model: session.model ?? undefined,
      agent: session.agent ?? undefined,
      modelVariant: session.model_variant ?? undefined,
      modelAccelerationEnabled: session.model_acceleration_enabled,
    }),
  );
  dispatch({ type: "close-panels" });
  if (client instanceof MockGatewayClient) {
    const next = await hydrate(getState(), client, session);
    dispatch({
      type: "hydrate",
      session: next.session!,
      messages: next.messages,
      permissions: next.permissions,
      providers: next.providers,
      agents: next.agents,
      personas: next.personas,
      sessions: next.sessions,
    });
    dispatch({ type: "questions", value: next.questions });
    dispatch({ type: "status", value: "idle" });
    return;
  }
  dispatch({ type: "status", value: "busy" });
}

function completeSlash(value: string): string {
  const commands = [
    "/help",
    "/chat",
    "/commands",
    "/new",
    "/resume",
    "/sessions",
    "/auth",
    "/login",
    "/logout",
    "/models",
    "/model",
    "/personas",
    "/persona",
    "/agent",
    "/settings",
    "/abort",
    "/config",
    "/quit",
  ];
  if (!value.startsWith("/")) return value;
  const matches = commands.filter((command) => command.startsWith(value));
  return matches.length === 1 ? `${matches[0]} ` : value;
}

async function fetchAuthSurface(
  client: TuiGatewayClient,
  providerIDs: string[],
): Promise<{
  methods?: Awaited<ReturnType<TuiGatewayClient["listProviderAuthMethods"]>>;
  statuses?: Record<string, ProviderAuthStatus>;
}> {
  const [methods, statuses] = await Promise.all([
    client.listProviderAuthMethods().catch(() => undefined),
    Promise.all(
      providerIDs.map(
        async (providerID) =>
          [providerID, await client.providerAuthStatus(providerID).catch(() => undefined)] as const,
      ),
    ).then((items) =>
      Object.fromEntries(
        items.filter((item): item is readonly [string, ProviderAuthStatus] => Boolean(item[1])),
      ),
    ),
  ]);
  return { methods, statuses };
}

function selectedModel(state: AppState): string | undefined {
  let row = 0;
  for (const provider of state.providers?.all ?? []) {
    for (const model of Object.keys(provider.models ?? {})) {
      if (row === state.selectedModelIndex) return `${provider.id}/${model}`;
      row += 1;
    }
  }
  return undefined;
}

function selectedPersonaID(state: AppState): string | undefined {
  const persona = state.personas[state.selectedPersonaIndex];
  return personaID(persona);
}

function personaID(persona: AppState["personas"][number] | undefined): string | undefined {
  const configName = persona?.config?.persona_name;
  return persona?.summary?.id ?? (typeof configName === "string" ? configName : undefined);
}

async function applyPersonaToActiveAgent(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  targetPersonaID: string,
): Promise<void> {
  const state = getState();
  const agentID = state.session?.agent ?? state.sessionConfig?.active_agent;
  if (!agentID) throw new Error("No active agent selected.");
  const persona =
    state.personas.find((item) => personaID(item) === targetPersonaID) ??
    (await client.getPersona(targetPersonaID));
  const stored = await client.getAgent(agentID);
  const config = {
    ...stored.config,
    agent_persona: [
      {
        persona_name: targetPersonaID,
        persona_directory:
          persona.config?.persona_directory ??
          persona.summary?.path ??
          `personas/src/${targetPersonaID}`,
      },
    ],
  };
  const updated = await client.updateAgent(agentID, { config, prompt: stored.prompt ?? undefined });
  const agents = state.agents.map((agent) => (storedAgentID(agent) === agentID ? updated : agent));
  dispatch({
    type: "agents",
    value: agents.length === state.agents.length ? agents : [updated, ...state.agents],
  });
  dispatch({
    type: "personas",
    value: await client.listPersonas().catch(() => state.personas),
    open: true,
  });
}

function storedAgentID(agent: AppState["agents"][number]): string | undefined {
  return agent.summary?.id ?? (agent as unknown as { name?: string }).name;
}

function draw(state: AppState, capabilities: TerminalCapabilities, previousFrame = ""): string {
  if (!process.stdout.isTTY) return previousFrame;
  const frame = render(state, capabilities);
  if (capabilities.level === "plain" && frame === previousFrame) return previousFrame;
  process.stdout.write(frame);
  return frame;
}

function richPromptFromInput(value: string): string {
  const trimmed = value.trim();
  if (!trimmed || /\[(?:MEDIA|EMOJI):/u.test(trimmed) || /\[[^\]]+\]\([^)]+\)/u.test(trimmed))
    return value;
  const paths = draggedPaths(trimmed);
  if (!paths.length) return value;
  return paths.map(richTokenForPath).join("\n");
}

function draggedPaths(value: string): string[] {
  const matches = Array.from(value.matchAll(/"([^"]+)"|'([^']+)'|(\S+)/gu))
    .map((match) => match[1] ?? match[2] ?? match[3])
    .filter(Boolean);
  if (!matches.length) return [];
  return matches.every((item) => isExistingLocalPath(item)) ? matches : [];
}

function richTokenForPath(path: string): string {
  if (isMediaPath(path)) return `[MEDIA:${path}:MEDIA]`;
  const label = basename(path.replace(/[\\/]+$/u, "")) || path;
  return `[${label}](${fileUrl(path)})`;
}

function isExistingLocalPath(path: string): boolean {
  try {
    return existsSync(path);
  } catch {
    return false;
  }
}

function isMediaPath(path: string): boolean {
  try {
    const stat = statSync(path);
    if (stat.isDirectory()) return false;
  } catch {
    return false;
  }
  return /\.(?:png|jpe?g|gif|webp|svg|bmp|mp4|mov|webm|mp3|wav|ogg)$/iu.test(path);
}

function fileUrl(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  const withSlash = /^[A-Za-z]:\//u.test(normalized) ? `/${normalized}` : normalized;
  return `file://${encodeURI(withSlash)}`;
}
