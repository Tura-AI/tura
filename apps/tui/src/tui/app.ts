import { emitKeypressEvents } from "node:readline";
import { GatewayClient } from "../gateway/client.js";
import { ensureGatewayAvailable } from "../gateway/autostart.js";
import { userFacingError } from "../gateway/errors.js";
import { MockGatewayClient } from "../gateway/mock-client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { isDraftSession, sessionTitle } from "../types/session.js";
import { sessionConfigPatchFromAssignments } from "../commands/config-values.js";
import { initialState, reducer, type AppAction, type AppState } from "./reducer.js";
import { detectTerminalCapabilities, type TerminalCapabilities } from "./capabilities.js";
import { TUI_DRAW_INTERVAL_MS } from "./frame-rate.js";
import { parseLanguage, setLanguage, t } from "../i18n.js";
import { keySequence, printableSequence } from "./interactions/keyboard.js";
import { selectedModel, selectedPersonaID, selectedSettingDetail } from "./logic/selection.js";
import { settingsEntries, settingOptions } from "./render/settings.js";
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
  applySelectedSetting,
  startProviderOauthLogin,
  submitSettingInput,
} from "./settings-actions.js";
import { hasActiveAnimation } from "./busy-state.js";
import {
  createAndSelectSession,
  loadAndSelectSession,
  loadAndSelectSessionByID,
  submitPrompt,
} from "./session-actions.js";
import { createResizeDrawGate, createTerminalResizeHandler } from "./resize.js";
import { mediaTokenForInputPath, saveClipboardImageInput } from "./clipboard-image.js";

const GATEWAY_SHUTDOWN_POLL_MS = 1_000;
const GATEWAY_SHUTDOWN_PROBE_TIMEOUT_MS = 1_500;
const GATEWAY_SHUTDOWN_FAILURES = 3;

export type TuiKeypressKey = {
  name?: string;
  ctrl?: boolean;
  meta?: boolean;
  shift?: boolean;
  sequence?: unknown;
};

type TuiKeypressExit = () => void;

export { clearTerminalForSurfaceTransition, draw, resetDrawState } from "./draw.js";
export { createResizeDrawGate, createTerminalResizeHandler } from "./resize.js";
export {
  deleteSelectedSession,
  forkSelectedSession,
  openSessionPicker,
  refreshOpenSessionPicker,
} from "./session-picker.js";
export {
  createAndSelectSession,
  loadAndSelectSession,
  loadAndSelectSessionByID,
  submitPrompt,
} from "./session-actions.js";

function isActiveSessionIdleEvent(action: AppAction, state: AppState): boolean {
  if (action.type !== "event") return false;
  const payload = action.event.payload;
  if (!payload) return false;
  if (payload.type === "session.status") {
    const properties = payload.properties as { sessionID?: string; status?: unknown } | undefined;
    return (
      (!properties?.sessionID || properties.sessionID === state.session?.id) &&
      properties?.status === "idle"
    );
  }
  if (payload.type === "session.updated") {
    const session = (payload.properties as { info?: { id?: string; status?: unknown } } | undefined)
      ?.info;
    return Boolean(session && session.id === state.session?.id && session.status === "idle");
  }
  return false;
}

function hasActiveLiveStreams(state: AppState): boolean {
  const sessionID = state.session?.id;
  return Object.values(state.liveStreams).some(
    (stream) => !sessionID || !stream.sessionID || stream.sessionID === sessionID,
  );
}

export async function runTui(context: CliContext, initialPrompt?: string): Promise<void> {
  if (!process.stdin.isTTY || !process.stdout.isTTY) {
    throw new CliUsageError(t("tuiRequiresTty"));
  }
  const capabilities = detectTerminalCapabilities(context.display);
  let client: TuiGatewayClient;
  let devLogPath: string | undefined;
  let connectedGatewayUrl: string | undefined;
  if (context.mock) {
    client = new MockGatewayClient({ directory: context.cwd });
  } else {
    const gatewayUrl = await ensureGatewayAvailable(
      context.gatewayUrl,
      capabilities,
      context.dev,
      context.gatewayUrlExplicit,
    );
    connectedGatewayUrl = gatewayUrl;
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
  state = reducer(state, {
    type: "session-loading",
    value: context.initialSessionId
      ? { sessionID: context.initialSessionId, title: context.initialSessionId }
      : {},
  });
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
    const nextDrawAt = Math.max(now, lastDrawAt + TUI_DRAW_INTERVAL_MS);
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
    const hadActiveLiveStreams = hasActiveLiveStreams(state);
    state = reducer(state, action);
    if (action.type === "event" && action.event.payload?.type === "message.part.delta") {
      scheduleDraw();
      return;
    }
    if (isActiveSessionIdleEvent(action, state)) {
      if (hadActiveLiveStreams) {
        flushDraw();
        return;
      }
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
    if (!hasActiveAnimation(state)) return;
    dispatch({ type: "tick" });
  }, TUI_DRAW_INTERVAL_MS);
  const sessionPickerRefreshTimer = setInterval(() => {
    if (!state.sessionsOpen || state.sessionLoading) return;
    void refreshOpenSessionPicker(client, () => state, dispatch);
  }, SESSION_PICKER_REFRESH_MS);
  let requestInputExit: (() => void) | undefined;
  const gatewayShutdownTimer = connectedGatewayUrl
    ? startGatewayShutdownWatcher(connectedGatewayUrl, () => requestInputExit?.())
    : undefined;

  // Load the initial session + transcript in the background. Keeping it off the
  // startup path means a slow or wedged gateway can never freeze the UI or block
  // keyboard input — the title is already on screen and input is wired up below.
  // Any failure surfaces as a notice instead of hanging or crashing the TUI.
  void (async () => {
    try {
      const session = await pickInitialSession(client, context.cwd, context.initialSessionId);
      dispatch({
        type: "session-loading",
        value: { sessionID: session.id, title: sessionTitle(session) },
      });
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
        modelConfig: next.modelConfig,
      });
      const mockInitialComposer = context.mock ? process.env.TURA_TUI_MOCK_INITIAL_COMPOSER : "";
      if (mockInitialComposer) dispatch({ type: "composer", value: mockInitialComposer });
      dispatch({ type: "questions", value: next.questions });
      if (initialPrompt?.trim()) {
        await submitPrompt(client, () => state, dispatch, initialPrompt);
      }
    } catch (error) {
      dispatch({ type: "session-loading", value: undefined });
      dispatch({ type: "notice", value: userFacingError(error), transient: true });
    }
  })();

  await inputLoop(
    client,
    () => state,
    dispatch,
    capabilities,
    resizeDrawGate.enterResize,
    (exit) => {
      requestInputExit = exit;
    },
  );
  clearInterval(heartbeatTimer);
  clearInterval(sessionPickerRefreshTimer);
  if (gatewayShutdownTimer) clearInterval(gatewayShutdownTimer);
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
  onExitReady?: (exit: () => void) => void,
): Promise<void> {
  emitKeypressEvents(process.stdin);
  if (process.stdin.isTTY && capabilities.interactive) process.stdin.setRawMode(true);
  return new Promise((resolve) => {
    const onTerminalResize = createTerminalResizeHandler(getState, dispatch, { onResize });
    const onExit = () => {
      process.stdin.off("keypress", onKeypress);
      process.stdout.off("resize", onTerminalResize);
      resolve();
    };
    onExitReady?.(onExit);
    const onKeypress = async (text: string, key: TuiKeypressKey | undefined) => {
      await handleTuiKeypress(client, getState, dispatch, text, key, onExit);
    };
    process.stdin.on("keypress", onKeypress);
    process.stdout.on("resize", onTerminalResize);
  });
}

function startGatewayShutdownWatcher(
  gatewayUrl: string,
  onShutdown: () => void,
): ReturnType<typeof setInterval> {
  let consecutiveFailures = 0;
  let probing = false;
  return setInterval(() => {
    if (probing) return;
    probing = true;
    void gatewayHealthReachable(gatewayUrl)
      .then((reachable) => {
        consecutiveFailures = reachable ? 0 : consecutiveFailures + 1;
        if (consecutiveFailures >= GATEWAY_SHUTDOWN_FAILURES) {
          onShutdown();
        }
      })
      .finally(() => {
        probing = false;
      });
  }, GATEWAY_SHUTDOWN_POLL_MS);
}

async function gatewayHealthReachable(gatewayUrl: string): Promise<boolean> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), GATEWAY_SHUTDOWN_PROBE_TIMEOUT_MS);
  try {
    const response = await fetch(`${gatewayUrl.replace(/\/+$/u, "")}/global/health`, {
      signal: controller.signal,
    });
    return response.ok;
  } catch {
    return false;
  } finally {
    clearTimeout(timer);
  }
}

function pageSelectionDelta(
  currentIndex: number,
  pageSize: number,
  totalEntries: number,
  direction: -1 | 1,
): number {
  if (totalEntries <= 0) return 0;
  const safePageSize = Math.max(1, pageSize);
  const safeIndex = Math.max(0, Math.min(currentIndex, totalEntries - 1));
  const pageStart = Math.floor(safeIndex / safePageSize) * safePageSize;
  const lastPageStart = Math.floor((totalEntries - 1) / safePageSize) * safePageSize;
  const target =
    direction > 0
      ? pageStart + safePageSize > totalEntries - 1
        ? 0
        : pageStart + safePageSize
      : pageStart - safePageSize < 0
        ? lastPageStart
        : pageStart - safePageSize;
  return target - safeIndex;
}

export async function handleTuiKeypress(
  client: TuiGatewayClient,
  getState: () => AppState,
  dispatch: (action: Parameters<typeof reducer>[1]) => void,
  text: string,
  key: TuiKeypressKey | undefined,
  onExit?: TuiKeypressExit,
): Promise<void> {
  try {
    const state = getState();
    const sequence = keySequence(key) ?? text ?? "";
    if (key?.ctrl && key.name === "c") {
      onExit?.();
      return;
    }
    if (state.sessionLoading) return;
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
    if (
      key?.name === "left" ||
      key?.name === "right" ||
      sequence === "\x1b[D" ||
      sequence === "\x1b[C"
    ) {
      if (state.settingInput) return;
      const direction = key?.name === "left" || sequence === "\x1b[D" ? -1 : 1;
      if (state.sessionsOpen) {
        dispatch({
          type: "select-session",
          delta: pageSelectionDelta(
            state.selectedSessionIndex,
            sessionPanelPageSize(),
            state.sessions.length + 1,
            direction,
          ),
        });
      } else if (state.settingsOpen && state.settingDetail) {
        dispatch({
          type: "select-setting-option",
          delta: pageSelectionDelta(
            state.selectedSettingOptionIndex,
            settingsPanelPageSize(state),
            settingOptions(state).length,
            direction,
          ),
        });
      } else if (state.settingsOpen) {
        dispatch({
          type: "select-settings",
          delta: pageSelectionDelta(
            state.selectedSettingsIndex,
            settingsPanelPageSize(state),
            settingsEntries(state).length,
            direction,
          ),
        });
      } else return;
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
      if (state.sessionsOpen) {
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
          await loadAndSelectSession(client, getState, dispatch, target, true);
        }
        return;
      }
      if (state.modelsOpen && !state.composer.trim()) {
        const model = selectedModel(state);
        if (model) {
          const config = await client.patchSessionConfig(
            sessionConfigPatchFromAssignments([`model=${model}`]),
          );
          dispatch({ type: "session-config", value: config });
          dispatch({ type: "notice", value: undefined });
        }
        return;
      }
      if (state.personasOpen && !state.composer.trim()) {
        const persona = selectedPersonaID(state);
        if (persona) {
          const config = await client.patchSessionConfig({ active_persona: persona });
          dispatch({ type: "session-config", value: config });
          dispatch({ type: "notice", value: undefined });
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
        if (shouldExit) onExit?.();
      } else await submitPrompt(client, getState, dispatch, value);
      return;
    }
    if (state.settingsOpen && !state.settingInput) return;
    if (key?.ctrl && key.name === "j") {
      dispatch({ type: "composer", value: `${state.composer}\n` });
      return;
    }
    if (key?.ctrl && key.name === "v") {
      const path = await saveClipboardImageInput(state.cwd);
      if (path)
        dispatch({ type: "composer", value: state.composer + mediaTokenForInputPath(path) });
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
    dispatch({ type: "notice", value: userFacingError(error), transient: true });
  }
}

function panelMaxLines(): number {
  return Math.max(1, (process.stdout.rows || 40) - 4);
}

function sessionPanelPageSize(): number {
  return Math.max(1, panelMaxLines() - 4);
}

function settingsPanelPageSize(state: AppState): number {
  const headerLines = 2;
  const promptLines = 1 + (state.settingInput ? 1 : 0);
  const pageChromeLines = state.settingDetail ? 1 : 2;
  return Math.max(1, panelMaxLines() - headerLines - promptLines - pageChromeLines);
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
  else if (name === "quit" || name === "exit") return true;
  else if (name === "new") await createAndSelectSession(client, getState, dispatch);
  else if (name === "resume") {
    const id = args[0];
    if (!id) dispatch({ type: "notice", value: t("usageResume") });
    else await loadAndSelectSessionByID(client, getState, dispatch, id, true);
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
      await startProviderOauthLogin(
        client,
        getState,
        dispatch,
        providerID,
        Number.isFinite(method) ? method : 0,
      );
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
    if (!model) {
      dispatch({ type: "toggle-models" });
    } else {
      const config = await client.patchSessionConfig(
        sessionConfigPatchFromAssignments([`model=${model}`]),
      );
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: undefined });
    }
  } else if (name === "agent") {
    const agent = args[0];
    if (!agent) {
      dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
      dispatch({ type: "open-setting-detail", detail: "agent" });
    } else {
      const config = await client.patchSessionConfig({ active_agent: agent });
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: undefined });
    }
  } else if (name === "persona") {
    const persona = args[0];
    if (persona) {
      const config = await client.patchSessionConfig({ active_persona: persona });
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: undefined });
      return false;
    }
    dispatch({
      type: "personas",
      value: await client.listPersonas().catch(() => getState().personas),
      open: true,
    });
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
        const validation = await client.providerAuthValidate(providerID, {
          type: "api_key",
          kind: "api_key",
          login: "api",
          key,
          access: key,
        });
        if (validation.ok) {
          await client.setProviderAuth(providerID, { type: "api_key", key });
        }
        const status = validation.ok
          ? await client.providerAuthStatus(providerID).catch(() => validation.status ?? undefined)
          : validation.status;
        dispatch({
          type: "auth",
          statuses: status
            ? { ...getState().authStatuses, [providerID]: status }
            : getState().authStatuses,
          open: false,
        });
        dispatch({ type: "notice", value: validation.message });
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
      dispatch({ type: "notice", value: undefined });
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
      dispatch({ type: "notice", value: undefined });
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
      dispatch({ type: "notice", value: undefined });
    }
  } else if (name === "session") {
    if (!args[0]) {
      dispatch({ type: "notice", value: t("usageConfigSet") });
    } else {
      const config = await client.patchSessionConfig({ session_type: args[0] });
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: undefined });
    }
  } else if (name === "validator") {
    if (!args[0]) {
      dispatch({ type: "notice", value: t("usageConfigSet") });
    } else {
      const config = await client.patchSessionConfig({
        validator_enabled: /^(1|true|yes|on|enabled)$/iu.test(args[0]),
      });
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: undefined });
    }
  } else if (name === "stall-guard") {
    if (!args[0]) {
      dispatch({ type: "session-config", value: await client.getSessionConfig(), open: true });
      dispatch({ type: "open-setting-detail", detail: "stallGuard" });
    } else {
      const config = await client.patchSessionConfig({ command_run_stall_guard_profile: args[0] });
      dispatch({ type: "session-config", value: config, open: true });
      dispatch({ type: "notice", value: undefined });
    }
  } else if (name === "config") {
    const subcommand = args.shift() ?? "get";
    if (subcommand === "set") {
      if (args.length === 0) dispatch({ type: "notice", value: t("usageConfigSet") });
      else {
        const config = await client.patchSessionConfig(sessionConfigPatchFromAssignments(args));
        applyConfiguredLanguage(config.language, undefined);
        dispatch({ type: "session-config", value: config, open: true });
        dispatch({ type: "notice", value: undefined });
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
