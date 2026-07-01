import {
  type GatewayClient,
  errorMessage,
  type ProviderAuthMethod,
  type TuraConfigModelPair,
} from "@tura/gateway-sdk";
import type { Accessor, Setter } from "solid-js";
import { setLanguage, t } from "../i18n";
import type { AppState } from "../state/global-store";
import { safe } from "../utils/safe";
import { openExternalUrl } from "../utils/external-url";
import {
  configDraftToPatch,
  configToDraft,
  draftToRecord,
  providerIdFromAuthError,
  recordToDraft,
} from "../utils/settings";
import { workspaceModelPatch } from "../utils/runtime-model";

type ProviderSettingsActionsOptions = {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  rootClient: Accessor<GatewayClient>;
  directoryClient: Accessor<GatewayClient>;
};

export function useProviderSettingsActions(options: ProviderSettingsActionsOptions) {
  const { state, setState, rootClient, directoryClient } = options;

  async function refreshProviderSurface(providerId?: string) {
    const client = rootClient();
    const [providers, providerAuthMethods] = await Promise.all([
      safe(() => directoryClient().providers(), state().providers),
      safe(() => client.providerAuthMethods(), state().providerAuthMethods),
    ]);
    const modelConfig = await safe(() => client.modelConfig(), state().modelConfig);
    const ids = providerId
      ? [providerId]
      : (providers?.all ?? state().providers?.all ?? []).map((provider) => provider.id);
    const statusEntries = await Promise.all(
      ids.map(async (id) => [id, await safe(() => client.providerAuthStatus(id), undefined)]),
    );
    const providerAuthStatus = {
      ...state().providerAuthStatus,
      ...Object.fromEntries(
        statusEntries.filter(
          (entry): entry is [string, AppState["providerAuthStatus"][string]] => !!entry[1],
        ),
      ),
    };
    setState((previous) => ({
      ...previous,
      providers,
      modelConfig,
      providerAuthMethods,
      providerAuthStatus,
    }));
  }

  function handleProviderAuthError(error: unknown): boolean {
    const providerId = providerIdFromAuthError(error, state());
    if (!providerId) {
      return false;
    }
    setState((previous) => ({
      ...previous,
      selectedProviderId: providerId,
      providerAuthPanel: {
        providerId,
        reason: errorMessage(error),
      },
      error: errorMessage(error),
    }));
    void refreshProviderSurface(providerId);
    return true;
  }

  async function saveRuntimeSettings() {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      error: undefined,
    }));
    try {
      const payload: Record<string, unknown> = {
        ...draftToRecord(state().workspaceConfigDraft),
        ...workspaceModelPatch(state().selectedModel),
        active_agent: state().selectedAgent,
        model_variant: state().modelVariant,
        model_acceleration_enabled: state().accelerationEnabled,
      };
      const configPayload = configDraftToPatch(state().configDraft, state().themeMode);
      const [workspaceConfig, config] = await Promise.all([
        directoryClient().patchWorkspaceConfig(payload),
        rootClient().patchConfig(configPayload),
      ]);
      setLanguage(stringField(workspaceConfig, "language"));
      setState((previous) => ({
        ...previous,
        config,
        configDraft: configToDraft(config),
        workspaceConfig,
        workspaceConfigDraft: recordToDraft(workspaceConfig),
        settingsSaving: false,
        settingsNotice: t("saved"),
      }));
    } catch (error) {
      if (!handleProviderAuthError(error)) {
        setState((previous) => ({
          ...previous,
          settingsSaving: false,
          error: errorMessage(error),
        }));
      } else {
        setState((previous) => ({ ...previous, settingsSaving: false }));
      }
    }
  }

  async function updateModelTier(tier: string, option: TuraConfigModelPair) {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      error: undefined,
    }));
    try {
      const selectedModel = `${option.provider}/${option.model}`;
      const [modelConfig, workspaceConfig] = await Promise.all([
        rootClient().putModelConfig({
          tier,
          provider: option.provider,
          model: option.model,
        }),
        directoryClient().patchWorkspaceConfig(workspaceModelPatch(selectedModel)),
      ]);
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        modelConfig,
        workspaceConfig,
        workspaceConfigDraft: recordToDraft(workspaceConfig),
        selectedModel,
        selectedProviderId: option.provider,
        settingsNotice: modelConfig.error ?? t("saved"),
      }));
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
    }
  }

  async function saveProviderKey(providerId: string, method: ProviderAuthMethod) {
    const draftKey = providerAuthDraftKey(providerId, method);
    const key = state().authDrafts[draftKey]?.trim();
    if (!key) {
      return;
    }
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      error: undefined,
    }));
    try {
      const ok = await rootClient().setProviderAuth(providerId, {
        type: method.type,
        key,
        access: key,
        metadata: { login: method.login, token_env: method.token_env },
      });
      await refreshProviderSurface(providerId);
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        settingsNotice: ok ? t("connected") : t("notConfigured"),
        authDrafts: { ...previous.authDrafts, [draftKey]: "" },
        providerAuthPanel:
          ok && previous.providerAuthPanel?.providerId === providerId
            ? undefined
            : previous.providerAuthPanel,
      }));
      if (ok) {
        void validateProvider(providerId);
      }
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
    }
  }

  async function validateProvider(providerId: string, method?: ProviderAuthMethod) {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      error: undefined,
    }));
    try {
      const draftKey = method ? providerAuthDraftKey(providerId, method) : undefined;
      const draftKeyValue = draftKey ? state().authDrafts[draftKey]?.trim() : undefined;
      const result = await rootClient().providerAuthValidate(providerId, {
        type: method?.type,
        kind: method?.kind,
        login: method?.login,
        token_env: method?.token_env,
        key: draftKeyValue || undefined,
        access: draftKeyValue || undefined,
      });
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        settingsNotice: result.message,
        providerValidationReceipts: {
          ...previous.providerValidationReceipts,
          [providerId]: result,
        },
        providerAuthStatus: result.status
          ? {
              ...previous.providerAuthStatus,
              [providerId]: result.status,
            }
          : previous.providerAuthStatus,
      }));
      await refreshProviderSurface(providerId);
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
    }
  }

  async function startProviderLogin(providerId: string, methodIndex: number) {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      error: undefined,
    }));
    try {
      const result = await rootClient().providerOauthAuthorize(providerId, {
        method: methodIndex,
      });
      if (result.url) {
        await openExternalUrl(result.url);
      }
      await refreshProviderSurface(providerId);
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        settingsNotice: result.instructions,
      }));
      if (result.method === "auto") {
        void waitForProviderAuthenticated(providerId);
      } else if (providerId === "github-copilot") {
        void completeProviderLogin(providerId, "", methodIndex);
      }
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
    }
  }

  async function waitForProviderAuthenticated(providerId: string, timeoutMs = 5 * 60_000) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const status = await safe(() => rootClient().providerAuthStatus(providerId), undefined);
      if (status) {
        setState((previous) => ({
          ...previous,
          providerAuthStatus: {
            ...previous.providerAuthStatus,
            [providerId]: status,
          },
          settingsNotice: status.authenticated ? t("connected") : previous.settingsNotice,
          providerAuthPanel:
            status.authenticated && previous.providerAuthPanel?.providerId === providerId
              ? undefined
              : previous.providerAuthPanel,
        }));
        if (status.authenticated) {
          await refreshProviderSurface(providerId);
          return;
        }
      }
      await sleep(1000);
    }
    setState((previous) => ({
      ...previous,
      settingsNotice: t("loginPending"),
    }));
  }

  async function completeProviderLogin(providerId: string, code?: string, methodIndex = 0) {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      error: undefined,
    }));
    try {
      const result = await rootClient().providerOauthCallback(providerId, {
        method: methodIndex,
        code: code?.trim() || undefined,
      });
      await refreshProviderSurface(providerId);
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        settingsNotice: result.ok ? t("connected") : result.message,
        authCodeDrafts: result.ok
          ? { ...previous.authCodeDrafts, [providerId]: "" }
          : previous.authCodeDrafts,
        providerValidationReceipts: {
          ...previous.providerValidationReceipts,
          [providerId]: result,
        },
        providerAuthStatus: result.status
          ? {
              ...previous.providerAuthStatus,
              [providerId]: result.status,
            }
          : previous.providerAuthStatus,
        providerAuthPanel:
          result.ok && previous.providerAuthPanel?.providerId === providerId
            ? undefined
            : previous.providerAuthPanel,
      }));
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
    }
  }

  async function logoutProvider(providerId: string) {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      settingsNotice: undefined,
      error: undefined,
    }));
    try {
      const result = await rootClient().providerAuthLogout(providerId);
      await refreshProviderSurface(providerId);
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        settingsNotice: result.message,
      }));
    } catch (error) {
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        error: errorMessage(error),
      }));
    }
  }

  return {
    refreshProviderSurface,
    handleProviderAuthError,
    saveRuntimeSettings,
    updateModelTier,
    saveProviderKey,
    validateProvider,
    startProviderLogin,
    completeProviderLogin,
    logoutProvider,
  };
}

function stringField(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  return typeof value === "string" && value.trim() ? value : undefined;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function providerAuthDraftKey(providerId: string, method: ProviderAuthMethod): string {
  return [providerId, method.token_env || method.login_env || method.kind].join("::");
}
