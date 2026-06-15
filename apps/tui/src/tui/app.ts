import { emitKeypressEvents } from "node:readline";
import { GatewayClient } from "../gateway/client.js";
import { ensureGatewayAvailable, killOwnedGateway } from "../gateway/autostart.js";
import { userFacingError } from "../gateway/errors.js";
import { MockGatewayClient } from "../gateway/mock-client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { isDraftSession } from "../types/session.js";
import { sessionConfigPatchFromAssignments } from "../commands/config-values.js";
import { initialState, reducer, type AppAction, type AppState } from "./reducer.js";
import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";
import {
  TUI_ANIMATION_INTERVAL_MS,
  TUI_MIN_DRAW_INTERVAL_MS,
  TUI_TICK_INTERVAL_MS,
} from "./frame-rate.js";
import { parseLanguage, setLanguage, t } from "../i18n.js";
import { keySequence, printableSequence } from "./interactions/keyboard.js";
import { selectedModel, selectedPersonaID, selectedSettingDetail } from "./logic/selection.js";
import {
  clearTerminalForSurfaceTransition,
  draw,
  drawChatChromeOverlay,
  resetDrawState,
} from "./draw.js";
import {
  eventLoop,
  fetchAuthSurface,
  hydrate,
  pickInitialSession,
  type TuiGatewayClient,
} from "./runtime.js";
import { shouldApplyInitialHydrate } from "./session-state.js";
import {
  deleteSelectedSession,
  forkSelectedSession,
  openSessionPicker,
  refreshOpenSessionPicker,
  SESSION_PICKER_REFRESH_MS,
} from "./session-picker.js";
import {
  applyPersonaToActiveAgent,
  applySelectedSetting,
  submitSettingInput,
  updateActiveSession,
} from "./settings-actions.js";
import { hasActiveAnimation, isBusyState } from "./busy-state.js";
import { createAndSelectSession, submitPrompt } from "./session-actions.js";
import { createResizeDrawGate, createTerminalResizeHandler } from "./resize.js";

export { clearTerminalForSurfaceTransition, draw, resetDrawState } from "./draw.js";
export { createResizeDrawGate, createTerminalResizeHandler } from "./resize.js";
export {
  deleteSelectedSession,
  forkSelectedSession,
  openSessionPicker,
  refreshOpenSessionPicker,
} from "./session-picker.js";
export { createAndSelectSession, submitPrompt } from "./session-actions.js";

function isActiveSessionIdleEvent(action: AppAction, state: AppState): boolean {
  if (action.type !== "event") return false;
  const payload = action.event.payload;
  if (!payload) return false;
  if (payload.type === "session.status") {
    const properties = payload.properties as Record<string, unknown> | undefined;
    const sessionID = stringField(properties, "sessionID") ?? stringField(properties, "session_id");
    return (!sessionID || sessionID === state.session?.id) && properties?.status === "idle";
  }
  if (payload.type === "session.updated") {
    const session = (payload.properties as { info?: { id?: string; status?: unknown } } | undefined)
      ?.info;
    return Boolean(session && session.id === state.session?.id && session.status === "idle");
  }
  return false;
}

function stringField(record: Record<string, unknown> | undefined, key: string): string | undefined {
  const value = record?.[key];
  return typeof value === "string" ? value : undefined;
}

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
  const clearPendingDraw = () => {
    if (pendingDraw) {
      clearTimeout(pendingDraw);
      pendingDraw = undefined;
      pendingDrawAt = 0;
    }
  };
  const performDraw = (forceReset = false) => {
    clearPendingDraw();
    lastFrame = draw(state, capabilities, lastFrame, { forceReset });
    lastDrawAt = Date.now();
  };
  const resizeDrawGate = createResizeDrawGate({
    drawNow: () => performDraw(true),
    clearPendingDraw,
  });
  const flushDraw = () => {
    if (resizeDrawGate.isFrozen()) return;
    performDraw();
  };
  const scheduleDraw = () => {
    if (resizeDrawGate.isFrozen()) {
      clearPendingDraw();
      return;
    }
    const now = Date.now();
    const nextDrawAt = Math.max(now, lastDrawAt + TUI_MIN_DRAW_INTERVAL_MS);
    if (pendingDraw && pendingDrawAt <= nextDrawAt) return;
    if (pendingDraw) clearTimeout(pendingDraw);
    pendingDrawAt = nextDrawAt;
    pendingDraw = setTimeout(
      () => {
        pendingDraw = undefined;
        pendingDrawAt = 0;
        if (resizeDrawGate.isFrozen()) return;
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
    if (isActiveSessionIdleEvent(action, state)) {
      lastFrame = drawChatChromeOverlay(state, capabilities, lastFrame);
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
    void eventLoop(client, () => state, controller.signal, dispatch);
  }
  const heartbeatTimer = setInterval(() => {
    if (!isBusyState(state) && !state.questions.length && !state.permissions.length) return;
    scheduleDraw();
  }, TUI_TICK_INTERVAL_MS);
  const animationTimer = setInterval(() => {
    if (!isBusyState(state) && !state.questions.length && !state.permissions.length) return;
    if (hasActiveAnimation(state)) dispatch({ type: "tick" });
  }, TUI_ANIMATION_INTERVAL_MS);
  const sessionPickerRefreshTimer = setInterval(() => {
    if (!state.sessionsOpen) return;
    void refreshOpenSessionPicker(client, () => state, dispatch);
  }, SESSION_PICKER_REFRESH_MS);

  // Load the initial session + transcript in the background. Keeping it off the
  // startup path means a slow or wedged gateway can never freeze the UI or block
  // keyboard input — the title is already on screen and input is wired up below.
  // Any failure surfaces as a notice instead of hanging or crashing the TUI.
  void (async () => {
    try {
      const session = await pickInitialSession(client, context.cwd);
      const next = await hydrate(initialState(context.cwd), client, session);
      if (!shouldApplyInitialHydrate(state, session.id)) return;
      applyConfiguredLanguage(next.sessionConfig?.language, context.language);
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

  await inputLoop(client, () => state, dispatch, capabilities, resizeDrawGate.enterResize);
  // Normal exit path: kill gateway immediately so it doesn't outlive the TUI.
  killOwnedGateway();
  clearInterval(heartbeatTimer);
  clearInterval(animationTimer);
  clearInterval(sessionPickerRefreshTimer);
  controller.abort();
  resizeDrawGate.dispose();
  flushDraw();
  if (process.stdin.isTTY) process.stdin.setRawMode(false);
  if (process.stdout.isTTY) process.stdout.write("\x1b[?25h\x1b[0m\n");
  else process.stdout.write("\n");
}

async function inputLoop(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  capabilities: TerminalCapabilities,
  onResize?: () => void,
): Promise<void> {
  emitKeypressEvents(process.stdin);
  if (process.stdin.isTTY && capabilities.interactive) process.stdin.setRawMode(true);
  return new Promise((resolve) => {
    const onTerminalResize = createTerminalResizeHandler(getState, dispatch, { onResize });
    const onKeypress = async (
      text: string,
      key: { name?: string; ctrl?: boolean; meta?: boolean; shift?: boolean } | undefined,
    ) => {
      try {
        const state = getState();
        const sequence = keySequence(key) ?? text ?? "";
        if (key?.ctrl && key.name === "c") {
          process.stdin.off("keypress", onKeypress);
          process.stdout.off("resize", onTerminalResize);
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
        if (state.sessionsOpen && (key?.name === "delete" || sequence === "\x1b[3~")) {
          await deleteSelectedSession(client, getState, dispatch);
          return;
        }
        if (key?.name === "return") {
          if (state.settingInput) {
            await submitSettingInput(client, getState, dispatch);
            return;
          }
          if (state.sessionsOpen && !state.composer.trim()) {
            if (key.shift) {
              await forkSelectedSession(client, getState, dispatch);
              return;
            }
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
              process.stdout.off("resize", onTerminalResize);
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
    process.stdout.on("resize", onTerminalResize);
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
    const config = await client.getSessionConfig();
    applyConfiguredLanguage(config.language, undefined);
    dispatch({ type: "session-config", value: config, open: true });
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
  } else if (name === "language" || name === "lang") {
    if (!args[0]) {
      dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
      dispatch({ type: "open-setting-detail", detail: "language" });
    } else {
      const parsed = parseLanguage(args[0]);
      if (!parsed) {
        dispatch({ type: "notice", value: t("unsupportedLanguage") });
        return false;
      }
      const config = await client.patchSessionConfig({ language: parsed });
      setLanguage(parsed);
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
        applyConfiguredLanguage(config.language, undefined);
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

function applyConfiguredLanguage(
  language: string | null | undefined,
  explicit: string | undefined,
): void {
  if (explicit) return;
  const parsed = parseLanguage(language ?? undefined);
  if (parsed) setLanguage(parsed);
}
