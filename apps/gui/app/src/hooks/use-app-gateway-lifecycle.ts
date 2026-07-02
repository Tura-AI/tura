import {
  connectGatewayEvents,
  errorMessage,
  type GatewayClient,
  type Project,
  type SessionLogWorkspace,
} from "@tura/gateway-sdk";
import { createEffect, createMemo, onCleanup, onMount, type Accessor, type Setter } from "solid-js";
import {
  GATEWAY_CONNECT_TIMEOUT_MS,
  GATEWAY_HEALTH_TIMEOUT_MS,
  isGatewayTimeoutError,
  tryStartGateway,
  waitForGatewayHealth,
} from "../app-gateway-startup";
import {
  clampNumber,
  mergeSessions,
  normalizeCornerRadiusMode,
  normalizeThemeMode,
} from "../app-state-utils";
import { DEFAULT_AGENT_ID, DEFAULT_MODEL_ID } from "../config/defaults";
import { setLanguage, t } from "../i18n";
import { applyGatewayEvent } from "../state/event-reducer";
import type { AppState } from "../state/global-store";
import {
  defaultWorkspaceDirectory,
  eventBelongsToState,
  readConfigBoolean,
  readConfigString,
  samePath,
  shortWorkspaceLabel,
} from "../utils/app-format";
import { workspaceModelFromConfig } from "../utils/runtime-model";
import { safe } from "../utils/safe";
import { configToDraft, defaultModel, providerIdFromModel, recordToDraft } from "../utils/settings";

function gatewayArray<T>(value: T[] | unknown): T[] {
  return Array.isArray(value) ? value : [];
}

const BOOTSTRAP_REQUEST_TIMEOUT_MS = 20_000;
const GATEWAY_SHUTDOWN_POLL_MS = 1_000;
const GATEWAY_SHUTDOWN_PROBE_TIMEOUT_MS = 1_500;
const GATEWAY_SHUTDOWN_FAILURES = 3;

export function useAppGatewayLifecycle(options: {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  gatewayUrl: Accessor<string>;
  rootClient: Accessor<GatewayClient>;
  forceNewSession: boolean;
  disableGatewayAutostart?: boolean;
  e2eFixture?: string;
  openSession: (sessionId: string) => Promise<void>;
}) {
  const {
    state,
    setState,
    gatewayUrl,
    rootClient,
    forceNewSession,
    disableGatewayAutostart,
    e2eFixture,
    openSession,
  } = options;
  const connection = createMemo(() => state().connection);

  createEffect(() => {
    if (
      e2eFixture ||
      state().connection === "connected" ||
      state().error ||
      state().gatewayStartupNotice
    ) {
      return;
    }
    const timer = window.setTimeout(() => {
      setState((previous) =>
        previous.connection === "connected" || previous.error
          ? previous
          : {
              ...previous,
              loading: false,
              bootstrapped: true,
              connection: "disconnected",
              error: t("gatewayResponseTimeout"),
            },
      );
    }, GATEWAY_CONNECT_TIMEOUT_MS);
    onCleanup(() => window.clearTimeout(timer));
  });

  createEffect(() => {
    if (e2eFixture || connection() !== "connected") {
      return;
    }
    const baseUrl = gatewayUrl();
    const stream = connectGatewayEvents({
      baseUrl,
      onEvent: (event) =>
        setState((previous) =>
          eventBelongsToState(previous, event.directory)
            ? applyGatewayEvent(previous, event)
            : previous,
        ),
      onError: () => setState((previous) => ({ ...previous, connection: "disconnected" })),
    });
    onCleanup(() => stream.close());
  });

  createEffect(() => {
    if (e2eFixture || connection() !== "connected" || !isTauriRuntime()) {
      return;
    }
    let consecutiveFailures = 0;
    let probing = false;
    const timer = window.setInterval(() => {
      if (probing) return;
      probing = true;
      void gatewayHealthReachable(gatewayUrl())
        .then((reachable) => {
          consecutiveFailures = reachable ? 0 : consecutiveFailures + 1;
          if (consecutiveFailures >= GATEWAY_SHUTDOWN_FAILURES) {
            void closeTauriWindow();
          }
        })
        .finally(() => {
          probing = false;
        });
    }, GATEWAY_SHUTDOWN_POLL_MS);
    onCleanup(() => window.clearInterval(timer));
  });

  onMount(() => {
    if (!e2eFixture) {
      void hydrate();
    }
  });

  async function hydrate(): Promise<void> {
    setState((previous) => ({
      ...previous,
      loading: true,
      sessionsLoading: true,
      connection: "connecting",
      error: undefined,
      gatewayStartupNotice: previous.gatewayStartupNotice,
    }));
    if (!disableGatewayAutostart) {
      const started = await tryStartGateway(gatewayUrl(), setState);
      if (started) {
        await waitForGatewayHealth(gatewayUrl(), GATEWAY_HEALTH_TIMEOUT_MS, setState);
      }
    }
    const client = rootClient();
    try {
      const fallbackPaths: NonNullable<AppState["paths"]> = {
        home: "",
        state: "",
        config: "",
        worktree: "",
        directory: "",
      };
      const fallbackConfig: NonNullable<AppState["config"]> = {};
      const fallbackCurrentProject: NonNullable<AppState["currentProject"]> = { project: null };
      const [health, serviceStatus, paths, config, modelConfig, currentProject, projects] =
        await Promise.all([
          bootstrapSafe(() => client.health(), { healthy: true, version: "unknown" }),
          bootstrapSafe(() => client.serviceStatus(), undefined),
          bootstrapSafe(() => client.paths(), fallbackPaths),
          bootstrapSafe(() => client.config(), fallbackConfig),
          bootstrapSafe(() => client.modelConfig(), undefined),
          bootstrapSafe(() => client.currentProject(), fallbackCurrentProject),
          bootstrapSafe(() => client.projects(), []),
        ]);
      const sessionLogWorkspaces = await bootstrapSafe(() => client.sessionLogWorkspaces(), {
        workspaces: [],
      });
      const [productConfig, me, workspaces, productIssues, productProjects] = await Promise.all([
        bootstrapSafe(() => client.productConfig(), undefined),
        bootstrapSafe(() => client.me(), undefined),
        bootstrapSafe(() => client.workspaces(), []),
        bootstrapSafe(() => client.productIssues(), []),
        bootstrapSafe(() => client.productProjects(), []),
      ]);
      const directory = defaultWorkspaceDirectory({
        ...paths,
        directory: paths.directory || currentProject.project?.worktree || paths.worktree,
      });
      const workspaceProjects = mergeWorkspaceProjects(
        projects,
        sessionLogWorkspaces.workspaces,
        directory,
      );
      const scoped = client.withDirectory(directory);
      const [
        sessions,
        providers,
        agentsResult,
        personasResult,
        commandsResult,
        filesResult,
        workspaceConfig,
      ] = await Promise.all([
        withTimeout(scoped.sessions({ limit: 100 }), BOOTSTRAP_REQUEST_TIMEOUT_MS),
        bootstrapSafe(() => scoped.providers(), undefined),
        bootstrapSafe(() => scoped.agents(), []),
        bootstrapSafe(() => scoped.personas(), []),
        bootstrapSafe(() => scoped.commands(), []),
        bootstrapSafe(() => scoped.files(), []),
        bootstrapSafe(() => scoped.workspaceConfig(), {}),
      ]);
      const agents = gatewayArray<AppState["agents"][number]>(agentsResult);
      const personas = gatewayArray<AppState["personas"][number]>(personasResult);
      const commands = gatewayArray<AppState["commands"][number]>(commandsResult);
      const files = gatewayArray<AppState["files"][number]>(filesResult);
      const providerAuthMethods = await bootstrapSafe(() => client.providerAuthMethods(), {});
      const providerAuthStatusEntries = await Promise.all(
        (providers?.all ?? []).map(async (provider) => [
          provider.id,
          await bootstrapSafe(() => client.providerAuthStatus(provider.id), undefined),
        ]),
      );
      const providerAuthStatus = Object.fromEntries(
        providerAuthStatusEntries.filter(
          (entry): entry is [string, AppState["providerAuthStatus"][string]] => !!entry[1],
        ),
      );
      const selectedSessionId = forceNewSession
        ? undefined
        : (state().selectedSessionId ?? sessions[0]?.id);
      const configuredModel =
        workspaceModelFromConfig(workspaceConfig, modelConfig) ??
        workspaceModelFromConfig(config, modelConfig);
      const configuredAgent = readConfigString(workspaceConfig, "active_agent") ?? config.agent;
      const configuredVariant = readConfigString(workspaceConfig, "model_variant");
      const configuredLanguage = readConfigString(workspaceConfig, "language") ?? config.language;
      const effectiveWorkspaceConfig =
        configuredLanguage && !readConfigString(workspaceConfig, "language")
          ? { ...workspaceConfig, language: configuredLanguage }
          : workspaceConfig;
      const configuredAcceleration = readConfigBoolean(
        workspaceConfig,
        "model_acceleration_enabled",
      );
      setLanguage(configuredLanguage);
      setState((previous) => ({
        ...previous,
        health,
        serviceStatus,
        productConfig,
        me,
        workspaces,
        productIssues,
        productProjects,
        paths,
        config,
        modelConfig,
        configDraft: configToDraft(config),
        workspaceConfig: effectiveWorkspaceConfig,
        workspaceConfigDraft: recordToDraft(effectiveWorkspaceConfig),
        currentProject,
        projects: workspaceProjects,
        directory,
        sessions: mergeSessions(sessions, previous.sessions),
        providers,
        providerAuthMethods,
        providerAuthStatus,
        agents,
        personas,
        commands,
        files,
        selectedSessionId: forceNewSession
          ? undefined
          : (previous.selectedSessionId ?? selectedSessionId),
        selectedAgent: previous.selectedAgent ?? configuredAgent ?? DEFAULT_AGENT_ID,
        selectedModel:
          previous.selectedModel ?? configuredModel ?? defaultModel(providers) ?? DEFAULT_MODEL_ID,
        selectedProviderId:
          previous.selectedProviderId ??
          providerIdFromModel(configuredModel) ??
          providerIdFromModel(previous.selectedModel) ??
          providers?.connected[0] ??
          providers?.all[0]?.id,
        themeMode: previous.bootstrapped ? previous.themeMode : normalizeThemeMode(config.theme),
        cornerRadius: previous.bootstrapped
          ? previous.cornerRadius
          : normalizeCornerRadiusMode(config.corner_radius),
        mainFont: previous.bootstrapped
          ? previous.mainFont
          : (config.main_font ?? previous.mainFont),
        codeFont: previous.bootstrapped
          ? previous.codeFont
          : (config.code_font ?? previous.codeFont),
        mainFontSize: previous.bootstrapped
          ? previous.mainFontSize
          : clampNumber(config.main_font_size, 11, 15, 12),
        codeFontSize: previous.bootstrapped
          ? previous.codeFontSize
          : clampNumber(config.code_font_size, 10, 15, 12),
        modelVariant: previous.bootstrapped
          ? previous.modelVariant
          : (configuredVariant ?? previous.modelVariant ?? "medium"),
        accelerationEnabled: previous.bootstrapped
          ? previous.accelerationEnabled
          : (configuredAcceleration ?? previous.accelerationEnabled ?? false),
        loading: false,
        sessionsLoading: false,
        bootstrapped: true,
        connection: "connected",
        gatewayStartupNotice: undefined,
        settingsNotice: undefined,
      }));
      if (selectedSessionId) {
        await openSession(selectedSessionId);
      }
    } catch (error) {
      setState((previous) => ({
        ...previous,
        loading: false,
        sessionsLoading: false,
        bootstrapped: true,
        connection: "disconnected",
        gatewayStartupNotice: undefined,
        error: isGatewayTimeoutError(error) ? t("gatewayResponseTimeout") : errorMessage(error),
      }));
    }
  }
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function closeTauriWindow(): Promise<void> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  } catch {
    window.close();
  }
}

async function gatewayHealthReachable(gatewayUrl: string): Promise<boolean> {
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), GATEWAY_SHUTDOWN_PROBE_TIMEOUT_MS);
  try {
    const response = await fetch(`${gatewayUrl.replace(/\/+$/u, "")}/global/health`, {
      signal: controller.signal,
    });
    return response.ok;
  } catch {
    return false;
  } finally {
    window.clearTimeout(timer);
  }
}

function mergeWorkspaceProjects(
  projects: Project[],
  workspaces: SessionLogWorkspace[],
  directory: string,
): Project[] {
  const merged: Project[] = [];
  const push = (project: Project) => {
    if (!project.worktree) {
      return;
    }
    const existingIndex = merged.findIndex((item) => samePath(item.worktree, project.worktree));
    if (existingIndex >= 0) {
      merged[existingIndex] = { ...merged[existingIndex], ...project };
      return;
    }
    merged.push(project);
  };
  push({
    id: directory,
    name: shortWorkspaceLabel(directory),
    worktree: directory,
  });
  for (const workspace of workspaces) {
    push({
      id: workspace.directory,
      name: shortWorkspaceLabel(workspace.directory),
      worktree: workspace.directory,
      time: {
        created: workspace.last_updated_at,
        updated: workspace.last_updated_at,
        initialized: null,
      },
    });
  }
  for (const project of projects) {
    push(project);
  }
  return merged;
}

async function bootstrapSafe<T>(run: () => Promise<T>, fallback: T): Promise<T> {
  return safe(() => withTimeout(run(), BOOTSTRAP_REQUEST_TIMEOUT_MS), fallback);
}

function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(
      () => reject(new Error("Gateway bootstrap timeout")),
      timeoutMs,
    );
    promise.then(
      (value) => {
        window.clearTimeout(timer);
        resolve(value);
      },
      (error: unknown) => {
        window.clearTimeout(timer);
        reject(error);
      },
    );
  });
}
