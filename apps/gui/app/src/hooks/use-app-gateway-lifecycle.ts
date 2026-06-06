import { connectGatewayEvents, errorMessage, type GatewayClient } from "@tura/gateway-sdk";
import { createEffect, onCleanup, onMount, type Accessor, type Setter } from "solid-js";
import {
  GATEWAY_CONNECT_TIMEOUT_MS,
  isGatewayTimeoutError,
  tryStartGateway,
  waitForGatewayHealth,
} from "../app-gateway-startup";
import { clampNumber, mergeSessions, normalizeThemeMode } from "../app-state-utils";
import { DEFAULT_AGENT_ID, DEFAULT_MODEL_ID } from "../config/defaults";
import { t } from "../i18n";
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
import { safe } from "../utils/safe";
import { configToDraft, defaultModel, providerIdFromModel, recordToDraft } from "../utils/settings";

export function useAppGatewayLifecycle(options: {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  gatewayUrl: Accessor<string>;
  rootClient: Accessor<GatewayClient>;
  forceNewSession: boolean;
  e2eFixture?: string;
  openSession: (sessionId: string) => Promise<void>;
}) {
  const { state, setState, gatewayUrl, rootClient, forceNewSession, e2eFixture, openSession } =
    options;

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
    if (e2eFixture) {
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

  onMount(() => {
    if (!e2eFixture) {
      void hydrate();
    }
  });

  async function hydrate(startAttempt = 0): Promise<void> {
    setState((previous) => ({
      ...previous,
      loading: true,
      connection: "connecting",
      error: undefined,
      gatewayStartupNotice: previous.gatewayStartupNotice,
    }));
    const client = rootClient();
    try {
      const [health, serviceStatus, paths, config, modelConfig, currentProject, projects] =
        await Promise.all([
          client.health(),
          safe(() => client.serviceStatus(), undefined),
          client.paths(),
          client.config(),
          safe(() => client.modelConfig(), undefined),
          client.currentProject(),
          safe(() => client.projects(), []),
        ]);
      const [productConfig, me, workspaces, productIssues, productProjects] = await Promise.all([
        safe(() => client.productConfig(), undefined),
        safe(() => client.me(), undefined),
        safe(() => client.workspaces(), []),
        safe(() => client.productIssues(), []),
        safe(() => client.productProjects(), []),
      ]);
      const directory = defaultWorkspaceDirectory({
        ...paths,
        directory: paths.directory || currentProject.project?.worktree || paths.worktree,
      });
      const workspaceProjects = projects.some((project) => samePath(project.worktree, directory))
        ? projects
        : [
            {
              id: directory,
              name: shortWorkspaceLabel(directory),
              worktree: directory,
            },
            ...projects,
          ];
      const scoped = client.withDirectory(directory);
      const [sessions, providers, agents, personas, commands, files, workspaceConfig] =
        await Promise.all([
          safe(() => scoped.sessions({ limit: 100 }), []),
          safe(() => scoped.providers(), undefined),
          safe(() => scoped.agents(), []),
          safe(() => scoped.personas(), []),
          safe(() => scoped.commands(), []),
          safe(() => scoped.files(), []),
          safe(() => scoped.workspaceConfig(), {}),
        ]);
      const providerAuthMethods = await safe(() => client.providerAuthMethods(), {});
      const providerAuthStatusEntries = await Promise.all(
        (providers?.all ?? []).map(async (provider) => [
          provider.id,
          await safe(() => client.providerAuthStatus(provider.id), undefined),
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
      const configuredModel = readConfigString(workspaceConfig, "model") ?? config.model;
      const configuredAgent = readConfigString(workspaceConfig, "active_agent") ?? config.agent;
      const configuredVariant = readConfigString(workspaceConfig, "model_variant");
      const configuredAcceleration = readConfigBoolean(
        workspaceConfig,
        "model_acceleration_enabled",
      );
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
        workspaceConfig,
        workspaceConfigDraft: recordToDraft(workspaceConfig),
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
        selectedSessionId: previous.selectedSessionId ?? selectedSessionId,
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
          : clampNumber(config.code_font_size, 9, 15, 11),
        modelVariant: previous.bootstrapped
          ? previous.modelVariant
          : (configuredVariant ?? previous.modelVariant ?? "medium"),
        accelerationEnabled: previous.bootstrapped
          ? previous.accelerationEnabled
          : (configuredAcceleration ?? previous.accelerationEnabled ?? true),
        loading: false,
        bootstrapped: true,
        connection: "connected",
        gatewayStartupNotice: undefined,
      }));
      if (selectedSessionId) {
        await openSession(selectedSessionId);
      }
    } catch (error) {
      if (isGatewayTimeoutError(error) && startAttempt < 1) {
        const started = await tryStartGateway(gatewayUrl(), setState);
        if (started) {
          await waitForGatewayHealth(gatewayUrl(), 120_000, setState);
          return hydrate(startAttempt + 1);
        }
      }
      setState((previous) => ({
        ...previous,
        loading: false,
        bootstrapped: true,
        connection: "disconnected",
        gatewayStartupNotice: undefined,
        error: isGatewayTimeoutError(error) ? t("gatewayResponseTimeout") : errorMessage(error),
      }));
    }
  }
}
