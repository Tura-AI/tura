import { emitKeypressEvents } from "node:readline";
import { setTimeout as delay } from "node:timers/promises";
import { existsSync, statSync } from "node:fs";
import { basename } from "node:path";
import { GatewayClient } from "../gateway/client.js";
import { ensureGatewayAvailable, killOwnedGateway } from "../gateway/autostart.js";
import { userFacingError } from "../gateway/errors.js";
import { MockGatewayClient } from "../gateway/mock-client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import type { Session } from "../types/session.js";
import type { ProviderAuthStatus } from "../types/provider.js";
import { messageText, sessionUpdatedAt } from "../types/session.js";
import { promptPayload } from "../commands/run.js";
import { sessionConfigPatchFromAssignments } from "../commands/config-values.js";
import { initialState, reducer, type AppState, type SettingDetail } from "./reducer.js";
import { renderFrame, settingOptions, settingsEntries } from "./render.js";
import { clear as terminalClear } from "./render-terminal.js";
import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";
import { TUI_ANIMATION_TICKS, TUI_DRAW_DEBOUNCE_MS, TUI_TICK_INTERVAL_MS } from "./frame-rate.js";
import { t } from "../i18n.js";

type TuiGatewayClient = GatewayClient | MockGatewayClient;

export async function runTui(context: CliContext, initialPrompt?: string): Promise<void> {
  if (!process.stdin.isTTY || !process.stdout.isTTY) {
    throw new CliUsageError(t("tuiRequiresTty"));
  }
  // Kill the owned gateway on any exit (crash, SIGTERM, unhandled rejection, etc.).
  // killOwnedGateway() is idempotent so calling it from both here and the normal
  // exit path is safe.
  process.on("exit", killOwnedGateway);

  const capabilities = detectTerminalCapabilities(context.display);
  let client: TuiGatewayClient;
  let devLogPath: string | undefined;
  if (context.mock) {
    client = new MockGatewayClient({ directory: context.cwd });
  } else {
    const gatewayUrl = await ensureGatewayAvailable(
      context.gatewayUrl,
      capabilities,
      context.dev,
      context.gatewayUrlExplicit,
    );
    client = new GatewayClient({
      baseUrl: gatewayUrl,
      directory: context.cwd,
      verbose: context.verbose,
    });
    const healthInfo = (await client.health()) as {
      healthy: boolean;
      version: string;
      dev_log_path?: string;
    };
    devLogPath = healthInfo.dev_log_path;
    await client.syncWorkspace();
  }
  let state = initialState(context.cwd);
  if (devLogPath) {
    state = reducer(state, { type: "notice", value: t("devModeActive", { path: devLogPath }) });
  }
  let lastFrame = "";
  if (capabilities.cursorControl) process.stdout.write(terminalClear);
  let pendingDraw: ReturnType<typeof setTimeout> | undefined;
  let pendingImmediateDraw: ReturnType<typeof setImmediate> | undefined;
  const flushDraw = () => {
    if (pendingImmediateDraw) {
      clearImmediate(pendingImmediateDraw);
      pendingImmediateDraw = undefined;
    }
    if (pendingDraw) {
      clearTimeout(pendingDraw);
      pendingDraw = undefined;
    }
    lastFrame = draw(state, capabilities, lastFrame);
  };
  const scheduleDraw = () => {
    if (pendingDraw || pendingImmediateDraw) return;
    pendingDraw = setTimeout(() => {
      pendingDraw = undefined;
      lastFrame = draw(state, capabilities, lastFrame);
    }, TUI_DRAW_DEBOUNCE_MS);
  };
  const dispatch = (action: Parameters<typeof reducer>[1]) => {
    state = reducer(state, action);
    if (action.type === "event" && action.event.payload?.type === "message.part.delta") {
      scheduleDraw();
      return;
    }
    if (action.type === "tick") {
      scheduleDraw();
      return;
    }
    // Composer: use setImmediate so single keystrokes draw instantly while
    // paste (all chars dispatched synchronously) coalesces into one draw.
    if (action.type === "composer") {
      flushDraw();
      return;
    }
    flushDraw();
  };

  // Paint immediately so the title stays pinned at the top and the screen is
  // never blank while the initial session list + transcript hydrate over the
  // network — which can be slow when the gateway is busy serving other clients.
  flushDraw();

  const controller = new AbortController();
  if (!context.mock) {
    void eventLoop(client, controller.signal, dispatch);
    void pollingLoop(client, () => state, dispatch, controller.signal);
  }
  // Timer fires at 20fps for UI responsiveness. thinkingFrame (spinner) only
  // advances every TUI_ANIMATION_TICKS firings to keep animation at 2fps.
  let tickCount = 0;
  const thinkingTimer = setInterval(() => {
    const current = state.status || state.session?.status;
    if (current !== "busy" && !state.questions.length && !state.permissions.length) return;
    tickCount += 1;
    if (tickCount % TUI_ANIMATION_TICKS === 0) {
      dispatch({ type: "tick" }); // advances thinkingFrame (spinner)
    } else {
      scheduleDraw(); // heartbeat redraw without advancing animation
    }
  }, TUI_TICK_INTERVAL_MS);

  // Load the initial session + transcript in the background. Keeping it off the
  // startup path means a slow or wedged gateway can never freeze the UI or block
  // keyboard input — the title is already on screen and input is wired up below.
  // Any failure surfaces as a notice instead of hanging or crashing the TUI.
  void (async () => {
    try {
      const session = await pickInitialSession(client);
      const next = await hydrate(initialState(context.cwd), client, session);
      if (!shouldApplyInitialHydrate(state, session.id)) return;
      dispatch({
        type: "hydrate",
        session: next.session!,
        messages: next.messages,
        permissions: next.permissions,
        providers: next.providers,
        agents: next.agents,
        personas: next.personas,
        sessions: next.sessions,
        authMethods: next.authMethods,
        authStatuses: next.authStatuses,
        sessionConfig: next.sessionConfig,
      });
      dispatch({ type: "questions", value: next.questions });
      if (initialPrompt?.trim()) {
        await submitPrompt(client, () => state, dispatch, initialPrompt);
      }
    } catch (error) {
      dispatch({ type: "notice", value: userFacingError(error) });
    }
  })();

  await inputLoop(client, () => state, dispatch, capabilities);
  // Normal exit path: kill gateway immediately so it doesn't outlive the TUI.
  killOwnedGateway();
  clearInterval(thinkingTimer);
  controller.abort();
  flushDraw();
  if (process.stdin.isTTY) process.stdin.setRawMode(false);
  if (capabilities.cursorControl) process.stdout.write("\x1b[?25h\x1b[0m\n");
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
  const sessions = await client.listSessions({ includeChildren: true }).catch(() => []);
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
          error: userFacingError(error),
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
  if (capabilities.cursorControl) process.stdout.write("\x1b[?25h");
  return new Promise((resolve) => {
    const onResize = () => dispatch({ type: "notice", value: getState().notice });
    const onKeypress = async (
      text: string,
      key: { name?: string; ctrl?: boolean; meta?: boolean } | undefined,
    ) => {
      try {
        const state = getState();
        const sequence = keySequence(key) ?? text ?? "";
        if (key?.ctrl && key.name === "c") {
          process.stdin.off("keypress", onKeypress);
          process.stdout.off("resize", onResize);
          resolve();
          return;
        }
        if (key?.name === "tab" || sequence === "\t") {
          await openSessionPicker(client, getState, dispatch);
          return;
        }
        if (key?.name === "escape") {
          if (state.help) dispatch({ type: "toggle-help" });
          if (state.sessionsOpen) dispatch({ type: "toggle-sessions" });
          if (state.modelsOpen) dispatch({ type: "toggle-models" });
          if (state.authOpen) dispatch({ type: "toggle-auth" });
          if (state.settingInput) {
            dispatch({ type: "setting-input", value: undefined });
            dispatch({ type: "composer", value: "" });
            return;
          }
          if (state.settingsOpen) {
            if (state.settingDetail === "providerAuth")
              dispatch({ type: "open-setting-detail", detail: "provider" });
            else if (state.settingDetail) dispatch({ type: "close-setting-detail" });
            else dispatch({ type: "toggle-settings" });
          }
          if (state.personasOpen) dispatch({ type: "toggle-personas" });
          return;
        }
        if (
          key?.name === "up" ||
          key?.name === "down" ||
          sequence === "\x1b[A" ||
          sequence === "\x1b[B"
        ) {
          if (state.settingInput) return;
          const delta = key?.name === "up" || sequence === "\x1b[A" ? -1 : 1;
          if (state.sessionsOpen) dispatch({ type: "select-session", delta });
          else if (state.modelsOpen) dispatch({ type: "select-model", delta });
          else if (state.personasOpen) dispatch({ type: "select-persona", delta });
          else if (state.settingsOpen && state.settingDetail)
            dispatch({ type: "select-setting-option", delta });
          else if (state.settingsOpen) dispatch({ type: "select-settings", delta });
          else {
            // Scroll transcript: up arrow = older content (+offset), down = newer (-offset)
            dispatch({ type: "scroll", delta: delta === -1 ? 1 : -1 });
          }
          return;
        }
        // Page Up / Page Down for larger scroll jumps
        if (key?.name === "pageup" || sequence === "\x1b[5~") {
          if (!isAnyPanelOpen(state)) dispatch({ type: "scroll", delta: 10 });
          return;
        }
        if (key?.name === "pagedown" || sequence === "\x1b[6~") {
          if (!isAnyPanelOpen(state)) dispatch({ type: "scroll", delta: -10 });
          return;
        }
        if (key?.name === "return") {
          if (state.settingInput) {
            await submitSettingInput(client, getState, dispatch);
            return;
          }
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
              dispatch({ type: "toggle-sessions" });
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
                  value: userFacingError(error),
                });
              }
            }
            return;
          }
          if (state.settingsOpen && !state.composer.trim()) {
            if (state.settingDetail) {
              await applySelectedSetting(client, getState, dispatch);
            } else {
              const detail = selectedSettingDetail(state);
              if (detail) dispatch({ type: "open-setting-detail", detail });
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
            // Always scroll to bottom when the user sends a new message
            dispatch({ type: "scroll", delta: -Number.MAX_SAFE_INTEGER });
            await submitPrompt(client, getState, dispatch, value);
          }
          return;
        }
        if (state.settingsOpen && !state.settingInput) return;
        if (key?.ctrl && key.name === "j") {
          dispatch({ type: "composer", value: `${state.composer}\n` });
          return;
        }
        if (key?.ctrl && key.name === "l") {
          dispatch({ type: "notice", value: state.notice });
          return;
        }
        if (key?.name === "backspace") {
          dispatch({ type: "composer", value: state.composer.slice(0, -1) });
          return;
        }
        const printable = text ?? printableSequence(keySequence(key));
        if (printable && !key?.ctrl && !key?.meta) {
          dispatch({ type: "composer", value: state.composer + printable });
        }
      } catch (error) {
        dispatch({ type: "notice", value: userFacingError(error) });
      }
    };
    process.stdin.on("keypress", onKeypress);
    process.stdout.on("resize", onResize);
  });
}

function isAnyPanelOpen(state: AppState): boolean {
  return (
    state.help ||
    state.sessionsOpen ||
    state.modelsOpen ||
    state.authOpen ||
    state.settingsOpen ||
    state.personasOpen
  );
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
  else if (name === "commands") {
    if (args[0]) {
      const config = await client.patchSessionConfig(
        sessionConfigPatchFromAssignments([
          `show_command_instructions=${args[0]}`,
          ...args.slice(1),
        ]),
      );
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: t("settingsUpdated") });
    } else {
      dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
      dispatch({ type: "open-setting-detail", detail: "commands" });
    }
  } else if (name === "quit" || name === "exit") return true;
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
    await openSessionPicker(client, getState, dispatch);
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
    if (!model) {
      dispatch({ type: "toggle-models" });
    } else if (sessionID) {
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
    if (!agent) {
      dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
      dispatch({ type: "open-setting-detail", detail: "agent" });
    } else if (sessionID) {
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
        dispatch({ type: "notice", value: userFacingError(error) });
      }
    }
  } else if (name === "abort" || name === "stop") {
    const sessionID = getState().session?.id;
    if (sessionID) {
      await client.abort(sessionID);
      dispatch({ type: "notice", value: t("abortRequested") });
    }
  } else if (name === "settings" || name === "setting") {
    dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
  } else if (name === "provider") {
    if (args[0] === "set-auth") {
      const providerID = args[1];
      const keyIndex = args.indexOf("--key");
      const key = keyIndex >= 0 ? args[keyIndex + 1] : undefined;
      if (!providerID || !key) {
        dispatch({ type: "notice", value: t("providerKeyHint", { provider: providerID ?? "" }) });
      } else {
        await client.setProviderAuth(providerID, { type: "api_key", key });
        const status = await client.providerAuthStatus(providerID).catch(() => undefined);
        dispatch({
          type: "auth",
          statuses: status
            ? { ...getState().authStatuses, [providerID]: status }
            : getState().authStatuses,
          open: false,
        });
        dispatch({ type: "notice", value: t("settingsUpdated") });
      }
    } else if (!args[0]) {
      dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
      dispatch({ type: "open-setting-detail", detail: "provider" });
    } else {
      const providerID = args[0];
      dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
      const auth = await fetchAuthSurface(client, [providerID]);
      dispatch({ type: "auth", methods: auth.methods, statuses: auth.statuses, open: false });
      dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID });
    }
  } else if (name === "variant") {
    if (!args[0]) {
      dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
      dispatch({ type: "open-setting-detail", detail: "variant" });
    } else {
      const config = await client.patchSessionConfig({ model_variant: args[0] });
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: t("settingsUpdated") });
    }
  } else if (name === "priority") {
    if (!args[0]) {
      dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
      dispatch({ type: "open-setting-detail", detail: "priority" });
    } else {
      const config = await client.patchSessionConfig({
        model_acceleration_enabled: /^(1|true|yes|on|priority)$/iu.test(args[0]),
      });
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: t("settingsUpdated") });
    }
  } else if (name === "session") {
    if (!args[0]) {
      dispatch({ type: "notice", value: t("usageConfigSet") });
    } else {
      const config = await client.patchSessionConfig({ session_type: args[0] });
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: t("settingsUpdated") });
    }
  } else if (name === "validator") {
    if (!args[0]) {
      dispatch({ type: "notice", value: t("usageConfigSet") });
    } else {
      const config = await client.patchSessionConfig({
        validator_enabled: /^(1|true|yes|on|enabled)$/iu.test(args[0]),
      });
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: t("settingsUpdated") });
    }
  } else if (name === "stall-guard") {
    if (!args[0]) {
      dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
      dispatch({ type: "open-setting-detail", detail: "stallGuard" });
    } else {
      const config = await client.patchSessionConfig({ command_run_stall_guard_profile: args[0] });
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: t("settingsUpdated") });
    }
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
  let session = getState().session;
  if (!session) {
    session = await client.createSession();
    const current = getState();
    dispatch({
      type: "hydrate",
      session,
      messages: current.messages,
      permissions: current.permissions,
      providers: current.providers,
      agents: current.agents,
      personas: current.personas,
      sessions: upsertSessionLocal(current.sessions, session),
      authMethods: current.authMethods,
      authStatuses: current.authStatuses,
      sessionConfig: current.sessionConfig,
    });
  }
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

function shouldApplyInitialHydrate(state: AppState, sessionID: string): boolean {
  if (state.status === "busy") return state.session?.id === sessionID;
  return !state.session || state.session.id === sessionID;
}

function upsertSessionLocal(sessions: Session[], session: Session): Session[] {
  return [...sessions.filter((item) => item.id !== session.id), session];
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

async function openSessionPicker(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
): Promise<void> {
  const state = getState();
  if (state.sessionsOpen) {
    dispatch({ type: "toggle-sessions" });
    return;
  }
  const sessions = await client.listSessions({ includeChildren: true }).catch(() => state.sessions);
  sessions.sort((left, right) => sessionUpdatedAt(right) - sessionUpdatedAt(left));
  dispatch({ type: "sessions", value: sessions, open: true });
  const previews = await sessionPreviews(client, sessions);
  dispatch({ type: "session-previews", value: previews });
}

async function sessionPreviews(
  client: TuiGatewayClient,
  sessions: Session[],
): Promise<Record<string, string>> {
  const entries = await Promise.all(
    sessions.slice(0, 30).map(async (session) => {
      const messages = await client.listMessages(session.id).catch(() => []);
      const preview = lastMessagePreview(messages);
      return preview ? ([session.id, preview] as const) : undefined;
    }),
  );
  return Object.fromEntries(
    entries.filter((entry): entry is readonly [string, string] => Boolean(entry)),
  );
}

function lastMessagePreview(
  messages: Awaited<ReturnType<TuiGatewayClient["listMessages"]>>,
): string {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const text = messageText(messages[index]).replace(/\s+/g, " ").trim();
    if (text) return text;
  }
  return "";
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

function selectedSettingDetail(state: AppState): SettingDetail | undefined {
  return settingsEntries(state)[state.selectedSettingsIndex]?.detail;
}

async function applySelectedSetting(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
): Promise<void> {
  const state = getState();
  const detail = state.settingDetail;
  if (!detail) return;
  const selected = settingOptions(state)[state.selectedSettingOptionIndex];
  if (!selected) return;
  const value = selected[2];
  if (detail === "model") {
    if (typeof value !== "string" || !state.session?.id) return;
    const session = await client.updateSession(state.session.id, { model: value });
    dispatchHydrateFromState(
      dispatch,
      state,
      session,
      await client.getSessionConfig().catch(() => state.sessionConfig),
    );
    dispatch({ type: "notice", value: t("settingsUpdated") });
    return;
  }
  if (detail === "agent") {
    if (typeof value !== "string" || !state.session?.id) return;
    const session = await client.updateSession(state.session.id, { agent: value });
    dispatchHydrateFromState(
      dispatch,
      state,
      session,
      await client.getSessionConfig().catch(() => state.sessionConfig),
    );
    dispatch({ type: "notice", value: t("settingsUpdated") });
    return;
  }
  if (detail === "persona" && typeof value === "string") {
    await applyPersonaToActiveAgent(client, getState, dispatch, value);
    dispatch({ type: "close-setting-detail" });
    dispatch({ type: "notice", value: t("settingsUpdated") });
    return;
  }
  if (detail === "provider" && typeof value === "string") {
    const auth = await fetchAuthSurface(client, [value]);
    dispatch({ type: "auth", methods: auth.methods, statuses: auth.statuses, open: false });
    dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID: value });
    return;
  }
  if (detail === "providerAuth") {
    await applyProviderAuthAction(client, getState, dispatch, value);
    return;
  }
  const patch = settingPatch(detail, value);
  if (!patch) return;
  const config = await client.patchSessionConfig(patch);
  dispatch({ type: "session-config", value: config, open: true });
  dispatch({ type: "open-setting-detail", detail });
  dispatch({ type: "notice", value: t("settingsUpdated") });
}

function dispatchHydrateFromState(
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  state: AppState,
  session: Session,
  sessionConfig: AppState["sessionConfig"],
): void {
  dispatch({
    type: "hydrate",
    session,
    messages: state.messages,
    permissions: state.permissions,
    providers: state.providers,
    agents: state.agents,
    personas: state.personas,
    sessions: state.sessions,
    sessionConfig,
  });
}

function settingPatch(detail: SettingDetail, value: unknown): Record<string, unknown> | undefined {
  if (detail === "variant") return { model_variant: value };
  if (detail === "priority") return { model_acceleration_enabled: value };
  if (detail === "commands") return { show_command_instructions: value };
  if (detail === "stallGuard") return { command_run_stall_guard_profile: value };
  return undefined;
}

async function applyProviderAuthAction(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  value: unknown,
): Promise<void> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return;
  const action = value as { action?: string; providerID?: string; method?: number };
  const providerID = action.providerID;
  if (!providerID) return;
  if (action.action === "oauth") {
    const auth = await client.providerOauthAuthorize(providerID, action.method ?? 0);
    const status = await client.providerAuthStatus(providerID).catch(() => undefined);
    dispatch({
      type: "auth",
      statuses: status
        ? { ...getState().authStatuses, [providerID]: status }
        : getState().authStatuses,
      open: false,
    });
    dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID });
    dispatch({
      type: "setting-input",
      value: {
        kind: "oauth-callback",
        providerID,
        prompt: t("oauthCallbackInputHint"),
      },
    });
    dispatch({
      type: "notice",
      value: [auth.instructions, auth.url ? t("openUrl", { url: auth.url }) : undefined]
        .filter(Boolean)
        .join(" "),
    });
    return;
  }
  if (action.action === "logout") {
    await client.providerLogout(providerID);
    const status = await client.providerAuthStatus(providerID).catch(() => undefined);
    dispatch({
      type: "auth",
      statuses: status
        ? { ...getState().authStatuses, [providerID]: status }
        : getState().authStatuses,
      open: false,
    });
    dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID });
    dispatch({ type: "notice", value: t("settingsUpdated") });
    return;
  }
  if (action.action === "api-key") {
    const method = (getState().authMethods?.[providerID] ?? []).find((item) =>
      /key|token|api/i.test(
        [item.type, item.kind, item.login, item.label].filter(Boolean).join(" "),
      ),
    );
    dispatch({
      type: "setting-input",
      value: {
        kind: "api-key",
        providerID,
        prompt: t("apiKeyInputHint", { provider: providerID }),
      },
    });
    dispatch({
      type: "notice",
      value: [
        method?.api_key_url ? t("openUrl", { url: method.api_key_url }) : undefined,
        method?.docs_url,
      ]
        .filter(Boolean)
        .join(" "),
    });
  }
}

async function submitSettingInput(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
): Promise<void> {
  const state = getState();
  const input = state.settingInput;
  const value = state.composer.trim();
  if (!input || !value) return;
  if (input.kind === "api-key") {
    await client.setProviderAuth(input.providerID, { type: "api_key", key: value });
  } else {
    await client.setProviderAuth(input.providerID, {
      type: "oauth",
      access: value,
      metadata: { callback_url_or_token: value },
    });
  }
  const status = await client.providerAuthStatus(input.providerID).catch(() => undefined);
  dispatch({
    type: "auth",
    statuses: status
      ? { ...getState().authStatuses, [input.providerID]: status }
      : getState().authStatuses,
    open: false,
  });
  dispatch({ type: "setting-input", value: undefined });
  dispatch({ type: "composer", value: "" });
  dispatch({ type: "open-setting-detail", detail: "providerAuth", providerID: input.providerID });
  dispatch({ type: "notice", value: t("settingsUpdated") });
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
    open: state.personasOpen,
  });
}

function storedAgentID(agent: AppState["agents"][number]): string | undefined {
  return agent.summary?.id ?? (agent as unknown as { name?: string }).name;
}

let lastDrawDimensions = "";

function draw(state: AppState, capabilities: TerminalCapabilities, previousFrame = ""): string {
  if (!process.stdout.isTTY) return previousFrame;
  const rendered = renderFrame(state, capabilities);
  const frame = rendered.frame;

  if (!capabilities.cursorControl) {
    // Non-interactive / plain terminal: simple full repaint, skip if identical
    if (frame === previousFrame) return previousFrame;
    process.stdout.write(`${terminalClear}${frame}`);
    return frame;
  }

  // Force a full clear when the terminal is resized so stale rows from the old
  // geometry can't linger. The clear sequence carries `\x1b[3J`, which the web
  // terminal uses as its "reset + scroll to top" marker.
  const dimensions = `${process.stdout.rows ?? 30}x${process.stdout.columns ?? 100}`;
  const resized = dimensions !== lastDrawDimensions;
  lastDrawDimensions = dimensions;

  if (frame === previousFrame && !resized) {
    // Nothing changed — only make sure the cursor sits in the composer.
    if (rendered.cursor) {
      process.stdout.write(`\x1b[${rendered.cursor.row};${rendered.cursor.column}H`);
    }
    return frame;
  }

  // Stable repaint: position every line at an absolute row and erase it before
  // writing its content. We never emit a newline, so the terminal can never
  // scroll and content can never be pushed into scrollback — the root cause of
  // the duplicated-while-scrolling artifacts. The trailing `\x1b[J` clears any
  // rows left over when the frame got shorter (e.g. composer shrank).
  const lines = frame.split("\n");
  let output = "\x1b[?25l"; // hide cursor while painting to avoid flicker
  if (!previousFrame || resized) output += terminalClear; // baseline / post-resize clear
  output += "\x1b[H";
  for (let row = 0; row < lines.length; row += 1) {
    output += `\x1b[${row + 1};1H\x1b[2K${lines[row]}`;
  }
  output += `\x1b[${lines.length + 1};1H\x1b[J`;
  if (rendered.cursor) {
    output += `\x1b[${rendered.cursor.row};${rendered.cursor.column}H`;
  }
  output += "\x1b[?25h"; // restore cursor
  process.stdout.write(output);
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
