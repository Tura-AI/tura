import { emitKeypressEvents } from "node:readline";
import { setTimeout as delay } from "node:timers/promises";
import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import type { Session } from "../types/session.js";
import type { ProviderAuthStatus } from "../types/provider.js";
import { isPlanStatus, sessionUpdatedAt } from "../types/session.js";
import { promptPayload } from "../commands/run.js";
import { sessionConfigPatchFromAssignments } from "../commands/config-values.js";
import { initialState, reducer, type AppState } from "./reducer.js";
import { render } from "./render.js";

export async function runTui(context: CliContext, initialPrompt?: string): Promise<void> {
  if (!process.stdin.isTTY || !process.stdout.isTTY) {
    throw new CliUsageError("interactive TUI requires a TTY; use `tura run` for non-interactive prompts");
  }
  const client = new GatewayClient({ baseUrl: context.gatewayUrl, directory: context.cwd, verbose: context.verbose });
  await client.health();
  await client.syncWorkspace();
  let session = await pickInitialSession(client);
  let state = await hydrate(initialState(context.cwd), client, session);
  const dispatch = (action: Parameters<typeof reducer>[1]) => {
    state = reducer(state, action);
    draw(state);
  };

  draw(state);
  const controller = new AbortController();
  void eventLoop(client, controller.signal, dispatch);
  void pollingLoop(client, () => state, dispatch, controller.signal);

  if (initialPrompt?.trim()) {
    await submitPrompt(client, () => state, dispatch, initialPrompt);
  }

  await inputLoop(client, () => state, dispatch);
  controller.abort();
  if (process.stdin.isTTY) process.stdin.setRawMode(false);
  process.stdout.write("\x1b[?25h\x1b[0m\n");
}

async function pickInitialSession(client: GatewayClient): Promise<Session> {
  const sessions = await client.listSessions({ limit: 20 });
  sessions.sort((left, right) => sessionUpdatedAt(right) - sessionUpdatedAt(left));
  return sessions[0] ?? client.createSession();
}

async function hydrate(state: AppState, client: GatewayClient, session: Session): Promise<AppState> {
  const [messages, todos, permissions, questions, providers, sessionConfig] = await Promise.all([
    client.listMessages(session.id).catch(() => []),
    client.todos(session.id).catch(() => []),
    client.listPermissions().catch(() => []),
    client.listQuestions().catch(() => []),
    client.listProviders().catch(() => undefined),
    client.getSessionConfig().catch(() => undefined),
  ]);
  const auth = providers ? await fetchAuthSurface(client, providers.all.map((provider) => provider.id)) : {};
  const sessions = await client.listSessions({ includeChildren: true, limit: 50 }).catch(() => []);
  return reducer(
    reducer(state, {
      type: "hydrate",
      session,
      messages,
      todos,
      permissions,
      providers,
      sessions,
      authMethods: auth.methods,
      authStatuses: auth.statuses,
      sessionConfig,
    }),
    {
      type: "questions",
      value: questions,
    },
  );
}

async function eventLoop(client: GatewayClient, signal: AbortSignal, dispatch: (action: Parameters<typeof reducer>[1]) => void): Promise<void> {
  while (!signal.aborted) {
    try {
      for await (const event of client.streamEvents(signal)) {
        dispatch({ type: "event", event });
      }
    } catch (error) {
      if (signal.aborted) return;
      dispatch({ type: "notice", value: `event stream reconnecting: ${error instanceof Error ? error.message : String(error)}` });
      await delay(1000);
    }
  }
}

async function pollingLoop(
  client: GatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  signal: AbortSignal,
): Promise<void> {
  while (!signal.aborted) {
    const sessionID = getState().session?.id;
    if (sessionID) {
      const [status, todos, permissions, questions] = await Promise.all([
        client.sessionStatus(sessionID).catch(() => undefined),
        client.todos(sessionID).catch(() => undefined),
        client.listPermissions().catch(() => undefined),
        client.listQuestions().catch(() => undefined),
      ]);
      if (status) dispatch({ type: "status", value: status });
      if (todos) dispatch({ type: "todos", value: todos });
      if (permissions) dispatch({ type: "permissions", value: permissions });
      if (questions) dispatch({ type: "questions", value: questions });
    }
    await delay(1500);
  }
}

async function inputLoop(
  client: GatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
): Promise<void> {
  emitKeypressEvents(process.stdin);
  if (process.stdin.isTTY) process.stdin.setRawMode(true);
  process.stdout.write("\x1b[?25l");
  return new Promise((resolve) => {
    const onResize = () => dispatch({ type: "notice", value: getState().notice });
    const onKeypress = async (text: string, key: { name?: string; ctrl?: boolean; meta?: boolean } | undefined) => {
      const state = getState();
      if (key?.ctrl && key.name === "c") {
        process.stdin.off("keypress", onKeypress);
        process.stdout.off("resize", onResize);
        resolve();
        return;
      }
      if (key?.name === "escape") {
        dispatch({ type: "diff", open: false });
        if (state.help) dispatch({ type: "toggle-help" });
        if (state.sessionsOpen) dispatch({ type: "toggle-sessions" });
        if (state.modelsOpen) dispatch({ type: "toggle-models" });
        if (state.authOpen) dispatch({ type: "toggle-auth" });
        if (state.settingsOpen) dispatch({ type: "toggle-settings" });
        if (state.planOpen) dispatch({ type: "toggle-plan" });
        return;
      }
      if (key?.name === "up" || key?.name === "down") {
        const delta = key.name === "up" ? -1 : 1;
        if (state.sessionsOpen) dispatch({ type: "select-session", delta });
        else if (state.modelsOpen) dispatch({ type: "select-model", delta });
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
              todos: next.todos,
              permissions: next.permissions,
              providers: next.providers,
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
              todos: state.todos,
              permissions: state.permissions,
              providers: state.providers,
              sessions: state.sessions,
            });
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
      if (key?.name === "backspace") {
        dispatch({ type: "composer", value: state.composer.slice(0, -1) });
        return;
      }
      if (key?.name === "tab") {
        dispatch({ type: "composer", value: completeSlash(state.composer) });
        return;
      }
      if (text && !key?.ctrl && !key?.meta) {
        dispatch({ type: "composer", value: state.composer + text });
      }
    };
    process.stdin.on("keypress", onKeypress);
    process.stdout.on("resize", onResize);
  });
}

async function slashCommand(
  client: GatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  input: string,
): Promise<boolean> {
  const [name, ...args] = input.slice(1).trim().split(/\s+/).filter(Boolean);
  if (!name || name === "help") dispatch({ type: "toggle-help" });
  else if (name === "chat") dispatch({ type: "close-panels" });
  else if (name === "quit" || name === "exit") return true;
  else if (name === "new") {
    const session = await client.createSession();
    const next = await hydrate(getState(), client, session);
    dispatch({ type: "hydrate", session: next.session!, messages: next.messages, todos: next.todos, permissions: next.permissions, providers: next.providers, sessions: next.sessions });
    dispatch({ type: "questions", value: next.questions });
  } else if (name === "resume") {
    const id = args[0];
    if (!id) dispatch({ type: "notice", value: "usage: /resume <session-id>" });
    else {
      const session = await client.getSession(id);
      const next = await hydrate(getState(), client, session);
      dispatch({ type: "hydrate", session: next.session!, messages: next.messages, todos: next.todos, permissions: next.permissions, providers: next.providers, sessions: next.sessions });
      dispatch({ type: "questions", value: next.questions });
    }
  } else if (name === "sessions") {
    dispatch({ type: "sessions", value: await client.listSessions({ includeChildren: true, limit: 50 }), open: true });
  }
  else if (name === "plan") {
    dispatch({ type: "sessions", value: await client.listSessions({ includeChildren: true, limit: 100 }), open: false });
    dispatch({ type: "toggle-plan" });
  }
  else if (name === "task") {
    const status = args[0];
    const sessionID = args[1] ?? getState().session?.id;
    if (!status || !sessionID || !isPlanStatus(status)) dispatch({ type: "notice", value: "usage: /task todo|doing|question|done|archived [session-id]" });
    else {
      const session = await client.updateSession(sessionID, { task_management: { status: status } });
      const sessions = getState().sessions.map((item) => (item.id === session.id ? session : item));
      dispatch({ type: "sessions", value: sessions });
      if (session.id === getState().session?.id) {
        dispatch({ type: "hydrate", session, messages: getState().messages, todos: getState().todos, permissions: getState().permissions, providers: getState().providers, sessions });
      }
    }
  }
  else if (name === "ticket") {
    const summary = args.join(" ").trim();
    if (!summary) dispatch({ type: "notice", value: "usage: /ticket <summary>" });
    else {
      const current = getState().session ?? (await client.createSession());
      const session = await client.updateSession(current.id, {
        task_management: {
          plan_summary: summary,
          task_summary: `执行任务：${summary}`,
        },
      });
      const next = await hydrate(getState(), client, session);
      dispatch({ type: "hydrate", session: next.session!, messages: next.messages, todos: next.todos, permissions: next.permissions, providers: next.providers, sessions: next.sessions });
      dispatch({ type: "questions", value: next.questions });
    }
  }
  else if (name === "models") dispatch({ type: "toggle-models" });
  else if (name === "auth" || name === "login") {
    const providerID = args[0];
    if (!providerID || name === "auth") {
      const providers = await client.listProviders().catch(() => getState().providers);
      const ids = providers?.all.map((provider) => provider.id) ?? [];
      const auth = await fetchAuthSurface(client, ids);
      dispatch({ type: "auth", methods: auth.methods, statuses: auth.statuses, open: true });
      if (name === "login" && !providerID) dispatch({ type: "notice", value: "usage: /login <provider> [method-index]" });
    } else {
      const method = Number(args[1] ?? "0");
      const auth = await client.providerOauthAuthorize(providerID, Number.isFinite(method) ? method : 0);
      const status = await client.providerAuthStatus(providerID).catch(() => undefined);
      dispatch({
        type: "auth",
        statuses: status ? { ...getState().authStatuses, [providerID]: status } : getState().authStatuses,
        open: true,
      });
      dispatch({
        type: "notice",
        value: [auth.instructions, auth.url ? `open: ${auth.url}` : undefined].filter(Boolean).join(" "),
      });
    }
  }
  else if (name === "logout") {
    const providerID = args[0];
    if (!providerID) dispatch({ type: "notice", value: "usage: /logout <provider>" });
    else {
      await client.providerLogout(providerID);
      const status = await client.providerAuthStatus(providerID).catch(() => undefined);
      dispatch({
        type: "auth",
        statuses: status ? { ...getState().authStatuses, [providerID]: status } : getState().authStatuses,
        open: true,
      });
    }
  }
  else if (name === "model") {
    const model = args[0];
    const sessionID = getState().session?.id;
    if (model && sessionID) {
      const session = await client.updateSession(sessionID, { model });
      dispatch({ type: "hydrate", session, messages: getState().messages, todos: getState().todos, permissions: getState().permissions, providers: getState().providers, sessions: getState().sessions });
    }
  } else if (name === "agent") {
    const agent = args[0];
    const sessionID = getState().session?.id;
    if (agent && sessionID) {
      const session = await client.updateSession(sessionID, { agent });
      dispatch({ type: "hydrate", session, messages: getState().messages, todos: getState().todos, permissions: getState().permissions, providers: getState().providers, sessions: getState().sessions });
    }
  } else if (name === "permissions") {
    dispatch({ type: "permissions", value: await client.listPermissions() });
  } else if (name === "approve" || name === "deny") {
    const id = args[0];
    if (!id) dispatch({ type: "notice", value: `usage: /${name} <request-id>` });
    else {
      await client.replyPermission(id, name === "approve");
      dispatch({ type: "permissions", value: await client.listPermissions() });
    }
  } else if (name === "answer") {
    const id = args.shift();
    const response = args.join(" ");
    if (!id || !response) dispatch({ type: "notice", value: "usage: /answer <request-id> <response>" });
    else {
      await client.replyQuestion(id, response);
      dispatch({ type: "questions", value: await client.listQuestions() });
    }
  } else if (name === "reject") {
    const id = args[0];
    if (!id) dispatch({ type: "notice", value: "usage: /reject <request-id>" });
    else {
      await client.rejectQuestion(id);
      dispatch({ type: "questions", value: await client.listQuestions() });
    }
  } else if (name === "abort") {
    const sessionID = getState().session?.id;
    if (sessionID) await client.abort(sessionID);
  } else if (name === "diff") {
    const diff = await client.diff();
    const text = diff.files.flatMap((file) => [`diff ${file.old_file_name} -> ${file.new_file_name}`, ...file.hunks.flatMap((hunk) => hunk.lines)]).join("\n");
    dispatch({ type: "diff", open: true, text });
  } else if (name === "status") {
    dispatch({ type: "notice", value: JSON.stringify(await client.serviceStatus()) });
  } else if (name === "settings") {
    dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
  } else if (name === "config") {
    const subcommand = args.shift() ?? "get";
    if (subcommand === "set") {
      if (args.length === 0) dispatch({ type: "notice", value: "usage: /config set KEY=VALUE..." });
      else {
        const config = await client.patchSessionConfig(sessionConfigPatchFromAssignments(args));
        dispatch({ type: "session-config", value: config, open: true });
        dispatch({ type: "notice", value: "settings updated" });
      }
    } else if (subcommand === "get") {
      const config = await client.getSessionConfig();
      const key = args[0];
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: JSON.stringify(key ? config[key] : config) });
    } else {
      dispatch({ type: "notice", value: "usage: /config get [KEY] or /config set KEY=VALUE..." });
    }
  } else if (name === "command") {
    const command = args.shift();
    if (!command) dispatch({ type: "notice", value: "usage: /command <name> [args...]" });
    else dispatch({ type: "notice", value: (await client.executeCommand(command, args)).output });
  } else {
    const result = await client.executeCommand(name, args);
    dispatch({ type: "notice", value: result.output });
  }
  return false;
}

async function submitPrompt(
  client: GatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  prompt: string,
): Promise<void> {
  const session = getState().session ?? (await client.createSession());
  await client.sendPromptAsync(
    session.id,
    promptPayload(prompt, {
      source: "tui",
      model: session.model ?? undefined,
      agent: session.agent ?? undefined,
      modelVariant: session.model_variant ?? undefined,
      modelAccelerationEnabled: session.model_acceleration_enabled,
    }),
  );
  dispatch({ type: "close-panels" });
  dispatch({ type: "status", value: "busy" });
}

function completeSlash(value: string): string {
  const commands = ["/help", "/chat", "/new", "/resume", "/sessions", "/plan", "/task", "/ticket", "/auth", "/login", "/logout", "/models", "/model", "/agent", "/settings", "/permissions", "/approve", "/deny", "/answer", "/reject", "/abort", "/diff", "/status", "/config", "/command", "/quit"];
  if (!value.startsWith("/")) return value;
  const matches = commands.filter((command) => command.startsWith(value));
  return matches.length === 1 ? `${matches[0]} ` : value;
}

async function fetchAuthSurface(client: GatewayClient, providerIDs: string[]): Promise<{
  methods?: Awaited<ReturnType<GatewayClient["listProviderAuthMethods"]>>;
  statuses?: Record<string, ProviderAuthStatus>;
}> {
  const [methods, statuses] = await Promise.all([
    client.listProviderAuthMethods().catch(() => undefined),
    Promise.all(
      providerIDs.map(async (providerID) => [providerID, await client.providerAuthStatus(providerID).catch(() => undefined)] as const),
    ).then((items) =>
      Object.fromEntries(items.filter((item): item is readonly [string, ProviderAuthStatus] => Boolean(item[1]))),
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

function draw(state: AppState): void {
  if (!process.stdout.isTTY) return;
  process.stdout.write(render(state));
}
