import { emitKeypressEvents } from "node:readline";
import { setTimeout as delay } from "node:timers/promises";
import { existsSync, statSync } from "node:fs";
import { basename } from "node:path";
import { GatewayClient } from "../gateway/client.js";
import { ensureGatewayAvailable, killOwnedGateway } from "../gateway/autostart.js";
import { userFacingError } from "../gateway/errors.js";
import { MockGatewayClient } from "../gateway/mock-client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import type { CreateSessionRequest, Message, PromptPayload, Session } from "../types/session.js";
import type { ProviderAuthStatus } from "../types/provider.js";
import { isDraftSession, sessionUpdatedAt } from "../types/session.js";
import { promptPayload } from "../commands/run.js";
import { sessionConfigPatchFromAssignments } from "../commands/config-values.js";
import { initialState, reducer, type AppState } from "./reducer.js";
import { renderChatFrameParts, renderFrame, settingOptions } from "./render.js";
import { clear as terminalClear } from "./render-terminal.js";
import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";
import {
  TUI_ANIMATION_INTERVAL_MS,
  TUI_MIN_DRAW_INTERVAL_MS,
  TUI_TICK_INTERVAL_MS,
} from "./frame-rate.js";
import { t } from "../i18n.js";
import { keySequence, printableSequence } from "./interactions/keyboard.js";
import {
  selectedModel,
  selectedPersonaID,
  selectedSettingDetail,
  promptRuntimeSelection,
  settingPatch,
} from "./logic/selection.js";
import { lastMessagePreview } from "./services/message-preview.js";

type TuiGatewayClient = GatewayClient | MockGatewayClient;
const SESSION_PREVIEW_FETCH_LIMIT = 8;
let draftSessionCounter = 0;

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
  resetDrawState();
  if (devLogPath) {
    state = reducer(state, { type: "notice", value: t("devModeActive", { path: devLogPath }) });
  }
  let lastFrame = "";
  clearTerminalForSurfaceTransition();
  let pendingDraw: ReturnType<typeof setTimeout> | undefined;
  let pendingDrawAt = 0;
  let lastDrawAt = 0;
  const flushDraw = () => {
    if (pendingDraw) {
      clearTimeout(pendingDraw);
      pendingDraw = undefined;
      pendingDrawAt = 0;
    }
    lastFrame = draw(state, capabilities, lastFrame);
    lastDrawAt = Date.now();
  };
  const scheduleDraw = () => {
    const now = Date.now();
    const nextDrawAt = Math.max(now, lastDrawAt + TUI_MIN_DRAW_INTERVAL_MS);
    if (pendingDraw && pendingDrawAt <= nextDrawAt) return;
    if (pendingDraw) clearTimeout(pendingDraw);
    pendingDrawAt = nextDrawAt;
    pendingDraw = setTimeout(
      () => {
        pendingDraw = undefined;
        pendingDrawAt = 0;
        lastFrame = draw(state, capabilities, lastFrame);
        lastDrawAt = Date.now();
      },
      Math.max(0, nextDrawAt - now),
    );
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
    // Composer input uses the same Codex-style frame limiter as streaming so
    // pasted text coalesces into one paint instead of hundreds of synchronous
    // terminal rewrites.
    if (action.type === "composer") {
      scheduleDraw();
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
  const heartbeatTimer = setInterval(() => {
    const current = state.status || state.session?.status;
    if (current !== "busy" && !state.questions.length && !state.permissions.length) return;
    scheduleDraw();
  }, TUI_TICK_INTERVAL_MS);
  const animationTimer = setInterval(() => {
    const current = state.status || state.session?.status;
    if (current !== "busy" && !state.questions.length && !state.permissions.length) return;
    if (hasActiveAnimation(state)) dispatch({ type: "tick" });
  }, TUI_ANIMATION_INTERVAL_MS);

  // Load the initial session + transcript in the background. Keeping it off the
  // startup path means a slow or wedged gateway can never freeze the UI or block
  // keyboard input — the title is already on screen and input is wired up below.
  // Any failure surfaces as a notice instead of hanging or crashing the TUI.
  void (async () => {
    try {
      const session = await pickInitialSession(client, context.cwd);
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
      const mockInitialComposer = context.mock ? process.env.TURA_TUI_MOCK_INITIAL_COMPOSER : "";
      if (mockInitialComposer) dispatch({ type: "composer", value: mockInitialComposer });
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
  clearInterval(heartbeatTimer);
  clearInterval(animationTimer);
  controller.abort();
  flushDraw();
  if (process.stdin.isTTY) process.stdin.setRawMode(false);
  if (process.stdout.isTTY) process.stdout.write("\x1b[?25h\x1b[0m\n");
  else process.stdout.write("\n");
}

async function pickInitialSession(client: TuiGatewayClient, cwd: string): Promise<Session> {
  const sessions = await client.listSessions({ limit: 20 });
  sessions.sort((left, right) => sessionUpdatedAt(right) - sessionUpdatedAt(left));
  if (sessions[0]) return sessions[0];
  return client.createSession().catch(() => createDraftSession(cwd));
}

async function hydrate(
  state: AppState,
  client: TuiGatewayClient,
  session: Session,
): Promise<AppState> {
  const draft = isDraftSession(session);
  const [messages, providers, sessionConfig, agents, personas] = await Promise.all([
    draft ? Promise.resolve([]) : client.listMessages(session.id).catch(() => []),
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
    if (sessionID && !isDraftSession(getState().session)) {
      await refreshActiveMessages(client, getState, dispatch, sessionID);
    }
    await delay(1500);
  }
}

async function refreshActiveMessages(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  sessionID: string,
): Promise<void> {
  const state = getState();
  const session = state.session;
  if (!session || session.id !== sessionID) return;
  const cursor = state.refreshState[sessionID];
  const messages = await client
    .listMessages(sessionID, cursor?.lastFinalMessageID ? { after: cursor.lastFinalMessageID } : {})
    .catch(() => undefined);
  if (!messages) return;
  const current = getState();
  const active = current.session;
  if (!active || active.id !== sessionID) return;
  if (!messages.length) return;
  dispatch({ type: "messages-incremental", sessionID, messages, session: active });
}

async function inputLoop(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  capabilities: TerminalCapabilities,
): Promise<void> {
  emitKeypressEvents(process.stdin);
  if (process.stdin.isTTY && capabilities.interactive) process.stdin.setRawMode(true);
  if (process.stdout.isTTY) process.stdout.write("\x1b[?25h");
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
          if (state.sessionsOpen) {
            clearTerminalForSurfaceTransition();
            dispatch({ type: "toggle-sessions" });
          }
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
          else return;
          return;
        }
        if (key?.name === "pageup" || sequence === "\x1b[5~") {
          return;
        }
        if (key?.name === "pagedown" || sequence === "\x1b[6~") {
          return;
        }
        if (key?.name === "return") {
          if (state.settingInput) {
            await submitSettingInput(client, getState, dispatch);
            return;
          }
          if (state.sessionsOpen && !state.composer.trim()) {
            if (state.selectedSessionIndex === 0) {
              clearTerminalForSurfaceTransition();
              await createAndSelectSession(client, getState, dispatch, true);
              return;
            }
            const target = state.sessions[state.selectedSessionIndex - 1];
            if (target) {
              clearTerminalForSurfaceTransition();
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
                closePanels: true,
              });
              dispatch({ type: "questions", value: next.questions });
            }
            return;
          }
          if (state.modelsOpen && !state.composer.trim()) {
            const model = selectedModel(state);
            const sessionID = state.session?.id;
            if (model && sessionID) {
              await updateActiveSession(client, getState, dispatch, { model });
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
          } else await submitPrompt(client, getState, dispatch, value);
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
  else if (name === "new") await createAndSelectSession(client, getState, dispatch);
  else if (name === "resume") {
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
      await updateActiveSession(client, getState, dispatch, { model });
    }
  } else if (name === "agent") {
    const agent = args[0];
    const sessionID = getState().session?.id;
    if (!agent) {
      dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
      dispatch({ type: "open-setting-detail", detail: "agent" });
    } else if (sessionID) {
      await updateActiveSession(client, getState, dispatch, { agent });
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
    if (sessionID && !isDraftSession(getState().session)) {
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

export async function createAndSelectSession(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  closePanels = false,
): Promise<void> {
  const current = getState();
  const runtimeSelection = promptRuntimeSelection(current);
  const session = await client
    .createSession(createSessionRequest(runtimeSelection))
    .catch(() => createDraftSession(current.cwd));
  dispatch({
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: current.providers,
    agents: current.agents,
    personas: current.personas,
    sessions: isDraftSession(session)
      ? current.sessions
      : upsertSessionLocal(current.sessions, session),
    authMethods: current.authMethods,
    authStatuses: current.authStatuses,
    sessionConfig: current.sessionConfig,
    closePanels,
  });
  dispatch({ type: "questions", value: [] });
}

function hasActiveAnimation(state: AppState): boolean {
  if (state.questions.length || state.permissions.length) return true;
  const sessionID = state.session?.id;
  if (
    Object.values(state.liveStreams).some(
      (stream) => !sessionID || !stream.sessionID || stream.sessionID === sessionID,
    )
  )
    return true;
  if (state.messages.at(-1)?.role === "user") return true;
  return state.messages.some((message) =>
    (message.parts ?? []).some((part) => {
      if (part.tool !== "command_run" && part.type !== "tool") return false;
      const status = commandStatus(part.state);
      return /run|progress|pending|busy|question|in[_ -]?progress|execut|start/i.test(status);
    }),
  );
}

function commandStatus(value: unknown): string {
  if (!value || typeof value !== "object") return "";
  const status = (value as { status?: unknown }).status;
  return typeof status === "string" ? status : "";
}

export async function submitPrompt(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  prompt: string,
): Promise<void> {
  let session = getState().session;
  const runtimeSelection = promptRuntimeSelection(getState());
  if (!session || isDraftSession(session)) {
    session = await client.createSession(createSessionRequest(runtimeSelection));
    const current = getState();
    dispatch({
      type: "hydrate",
      session,
      messages: isDraftSession(current.session) ? [] : current.messages,
      permissions: isDraftSession(current.session) ? [] : current.permissions,
      providers: current.providers,
      agents: current.agents,
      personas: current.personas,
      sessions: upsertSessionLocal(current.sessions, session),
      authMethods: current.authMethods,
      authStatuses: current.authStatuses,
      sessionConfig: current.sessionConfig,
    });
  }
  const payload = promptPayload(richPromptFromInput(prompt), {
    source: "tui",
    ...runtimeSelection,
  });
  dispatch({ type: "close-panels" });
  dispatch({
    type: "messages-incremental",
    sessionID: session.id,
    messages: [localUserMessage(session.id, payload)],
    session: { ...session, status: "busy" },
  });
  await client.sendPromptAsync(session.id, payload);
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

function localUserMessage(sessionID: string, payload: PromptPayload): Message {
  const now = Date.now();
  return {
    id: payload.messageID,
    sessionID,
    session_id: sessionID,
    role: "user",
    created_at: now,
    updated_at: now,
    parts: payload.parts.map((part) => ({ ...part, sessionID, session_id: sessionID })),
  };
}

function createDraftSession(cwd: string): Session {
  draftSessionCounter += 1;
  return {
    id: `draft-session-${Date.now()}-${draftSessionCounter}`,
    draft: true,
    name: t("newSession"),
    directory: cwd,
    status: "idle",
    updated_at: Date.now(),
    message_count: 0,
  };
}

function createSessionRequest(
  runtimeSelection: ReturnType<typeof promptRuntimeSelection>,
): CreateSessionRequest {
  return {
    ...(runtimeSelection.model ? { model: runtimeSelection.model } : {}),
    ...(runtimeSelection.agent ? { agent: runtimeSelection.agent } : {}),
    ...(runtimeSelection.modelVariant ? { model_variant: runtimeSelection.modelVariant } : {}),
    ...(runtimeSelection.modelAccelerationEnabled !== undefined
      ? { model_acceleration_enabled: runtimeSelection.modelAccelerationEnabled }
      : {}),
  };
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

export async function openSessionPicker(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
): Promise<void> {
  const state = getState();
  if (state.sessionsOpen) {
    clearTerminalForSurfaceTransition();
    dispatch({ type: "toggle-sessions" });
    return;
  }
  clearTerminalForSurfaceTransition();
  dispatch({
    type: "sessions",
    value: sortedSessions(state.sessions.length ? state.sessions : activeSessionList(state)),
    open: true,
  });
  void refreshOpenSessionPicker(client, getState, dispatch);
}

async function sessionPreviews(
  client: TuiGatewayClient,
  sessions: Session[],
  cachedPreviews: Record<string, string> = {},
): Promise<Record<string, string>> {
  const entries = await Promise.all(
    sessions.slice(0, SESSION_PREVIEW_FETCH_LIMIT).map(async (session) => {
      const cached = cachedPreviews[session.id];
      if (cached) return [session.id, cached] as const;
      const messages = await client.listMessages(session.id, { limit: 1 }).catch(() => []);
      const preview = lastMessagePreview(messages);
      return preview ? ([session.id, preview] as const) : undefined;
    }),
  );
  return Object.fromEntries(
    entries.filter((entry): entry is readonly [string, string] => Boolean(entry)),
  );
}

async function refreshOpenSessionPicker(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
): Promise<void> {
  const sessions = sortedSessions(
    await client.listSessions({ includeChildren: true }).catch(() => getState().sessions),
  );
  if (!getState().sessionsOpen) return;
  dispatch({ type: "sessions", value: sessions, open: true });
  const previews = await sessionPreviews(client, sessions, getState().sessionPreviews);
  if (!getState().sessionsOpen) return;
  dispatch({ type: "session-previews", value: previews });
}

function activeSessionList(state: AppState): Session[] {
  return state.session && !isDraftSession(state.session) ? [state.session] : [];
}

function sortedSessions(sessions: Session[]): Session[] {
  return [...sessions].sort((left, right) => sessionUpdatedAt(right) - sessionUpdatedAt(left));
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
    await updateActiveSession(client, getState, dispatch, { model: value });
    dispatch({ type: "notice", value: t("settingsUpdated") });
    return;
  }
  if (detail === "agent") {
    if (typeof value !== "string" || !state.session?.id) return;
    await updateActiveSession(client, getState, dispatch, { agent: value });
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

async function updateActiveSession(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  patch: Partial<Session>,
): Promise<Session | undefined> {
  const state = getState();
  const active = state.session;
  if (!active) return undefined;
  const session = isDraftSession(active)
    ? { ...active, ...patch, updated_at: Date.now() }
    : await client.updateSession(active.id, patch);
  dispatchHydrateFromState(
    dispatch,
    state,
    session,
    await client.getSessionConfig().catch(() => state.sessionConfig),
  );
  return session;
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

let lastDrawSurface = "";
let lastDrawSessionID = "";
let lastChatCacheFrame = "";
let lastChatLiveFrame = "";
let lastChatChromeFrame = "";
let lastChatChromeCursor: { row: number; column: number } | undefined;

export function resetDrawState(): void {
  lastDrawSurface = "";
  lastDrawSessionID = "";
  lastChatCacheFrame = "";
  lastChatLiveFrame = "";
  lastChatChromeFrame = "";
  lastChatChromeCursor = undefined;
}

export function clearTerminalForSurfaceTransition(): void {
  if (!process.stdout.isTTY) return;
  resetDrawState();
  process.stdout.write(terminalSurfaceClear());
}

export function draw(
  state: AppState,
  capabilities: TerminalCapabilities,
  previousFrame = "",
): string {
  if (!process.stdout.isTTY) return previousFrame;
  const surface = drawSurface(state);
  const rendered =
    surface === "chat"
      ? renderChatFrameParts(state, capabilities)
      : renderFrame(state, capabilities);
  const frame = rendered.frame;
  const sessionID = state.session?.id ?? "";
  const previousSurface = lastDrawSurface;
  const previousSessionID = lastDrawSessionID;
  const sessionSurfaceBoundary =
    (surface === "sessions" && previousSurface !== surface) ||
    (previousSurface === "sessions" && previousSurface !== surface);
  const shouldClearForSessionSurface = previousSessionID !== sessionID || sessionSurfaceBoundary;
  lastDrawSurface = surface;
  lastDrawSessionID = sessionID;

  if (surface === "chat") {
    return drawChatFrame(
      rendered as ReturnType<typeof renderChatFrameParts>,
      previousFrame,
      shouldClearForSessionSurface || previousSurface !== "chat",
    );
  }

  let output = "\x1b[?25l";
  output += terminalSurfaceClear();
  output += terminalAppendFrame(frame);
  output += cursorOutputFromFrameEnd(frame, rendered.cursor);
  process.stdout.write(output);
  lastChatCacheFrame = "";
  lastChatLiveFrame = "";
  lastChatChromeFrame = "";
  lastChatChromeCursor = undefined;
  return frame;
}

function drawChatFrame(
  rendered: ReturnType<typeof renderChatFrameParts>,
  previousFrame: string,
  forceReset: boolean,
): string {
  const frame = rendered.frame;
  if (frame === previousFrame && !forceReset) {
    return frame;
  }

  let output = "\x1b[?25l";
  if (forceReset || !lastChatCacheFrame || rendered.cacheFrame !== lastChatCacheFrame) {
    output += terminalSurfaceClear();
    output += terminalAppendFrame(frame);
    output += cursorOutputFromFrameEnd(frame, rendered.cursor);
  } else {
    output += terminalRewriteChatLiveAndChrome(rendered);
  }
  process.stdout.write(output);
  lastChatCacheFrame = rendered.cacheFrame;
  lastChatLiveFrame = rendered.liveFrame;
  lastChatChromeFrame = rendered.chromeFrame;
  lastChatChromeCursor = rendered.chromeCursor;
  return frame;
}

function drawSurface(state: AppState): string {
  if (state.help) return "help";
  if (state.sessionsOpen) return "sessions";
  if (state.authOpen) return "auth";
  if (state.settingsOpen) return "settings";
  if (state.personasOpen) return "personas";
  if (state.modelsOpen) return "models";
  return "chat";
}

function terminalAppendFrame(frame: string): string {
  if (!frame) return "";
  return frame.replace(/\n/g, "\r\n");
}

function terminalRewriteChatLiveAndChrome(
  rendered: ReturnType<typeof renderChatFrameParts>,
): string {
  const previousLiveLines = frameLines(lastChatLiveFrame);
  const nextLiveLines = frameLines(rendered.liveFrame);
  const diffIndex = firstDifferentLine(previousLiveLines, nextLiveLines);
  const currentChromeCursorRow =
    lastChatChromeCursor?.row ?? Math.max(1, frameLineCount(lastChatChromeFrame));
  const rowsUp = Math.max(0, currentChromeCursorRow - 1 + previousLiveLines.length - diffIndex);
  const repaintLines = [...nextLiveLines.slice(diffIndex), ...frameLines(rendered.chromeFrame)];
  const repaintFrame = repaintLines.join("\n");
  const repaintCursor = rendered.chromeCursor
    ? {
        row: nextLiveLines.length - diffIndex + rendered.chromeCursor.row,
        column: rendered.chromeCursor.column,
      }
    : undefined;
  return [
    "\x1b[1G",
    rowsUp > 0 ? `\x1b[${rowsUp}A` : "",
    "\x1b[J",
    terminalAppendFrame(repaintFrame),
    cursorOutputFromFrameEnd(repaintFrame, repaintCursor),
  ].join("");
}

function frameLines(frame: string): string[] {
  return frame ? frame.split("\n") : [];
}

function firstDifferentLine(left: string[], right: string[]): number {
  const count = Math.min(left.length, right.length);
  for (let index = 0; index < count; index += 1) {
    if (left[index] !== right[index]) return index;
  }
  return count;
}

function cursorOutputFromFrameEnd(frame: string, cursor?: { row: number; column: number }): string {
  if (!cursor) return "";
  return `${cursorPositionFromFrameEnd(frame, cursor)}\x1b[?25h`;
}

function cursorPositionFromFrameEnd(
  frame: string,
  cursor: { row: number; column: number },
): string {
  const frameRows = frameLineCount(frame);
  const cursorRow = Math.max(1, Math.min(frameRows, cursor.row));
  const rowsBelowCursor = frameRows - cursorRow;
  const column = Math.max(1, cursor.column);
  return `${rowsBelowCursor > 0 ? `\x1b[${rowsBelowCursor}A` : ""}\x1b[${column}G`;
}

function frameLineCount(frame: string): number {
  return frame ? frame.split("\n").length : 1;
}

function terminalSurfaceClear(): string {
  return `\x1b[0m${terminalClear}`;
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
