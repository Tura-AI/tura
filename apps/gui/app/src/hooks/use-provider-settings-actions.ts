import type { Accessor, Setter } from "solid-js";
import {
  GatewayClient,
  errorMessage,
  type ProviderAuthMethod,
  type TuraConfigModelPair,
} from "@tura/gateway-sdk";
import { t } from "../i18n";
import type { AppState } from "../state/global-store";
import {
  configDraftToPatch,
  configToDraft,
  draftToRecord,
  providerIdFromAuthError,
  recordToDraft,
} from "../utils/settings";
import { safe } from "../utils/safe";

type ProviderSettingsActionsOptions = {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  rootClient: Accessor<GatewayClient>;
  directoryClient: Accessor<GatewayClient>;
};

export function useProviderSettingsActions(
  options: ProviderSettingsActionsOptions,
) {
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
      : (providers?.all ?? state().providers?.all ?? []).map(
          (provider) => provider.id,
        );
    const statusEntries = await Promise.all(
      ids.map(async (id) => [
        id,
        await safe(() => client.providerAuthStatus(id), undefined),
      ]),
    );
    const providerAuthStatus = {
      ...state().providerAuthStatus,
      ...Object.fromEntries(
        statusEntries.filter(
          (entry): entry is [string, AppState["providerAuthStatus"][string]] =>
            !!entry[1],
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
        model: state().selectedModel,
        active_agent: state().selectedAgent,
        model_variant: state().modelVariant,
        model_acceleration_enabled: state().accelerationEnabled,
      };
      const configPayload = configDraftToPatch(
        state().configDraft,
        state().themeMode,
      );
      const [workspaceConfig, config] = await Promise.all([
        directoryClient().patchWorkspaceConfig(payload),
        rootClient().patchConfig(configPayload),
      ]);
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
      const modelConfig = await rootClient().putModelConfig({
        tier,
        provider: option.provider,
        model: option.model,
      });
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        modelConfig,
        selectedModel: `${option.provider}/${option.model}`,
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

  async function saveProviderKey(
    providerId: string,
    method: ProviderAuthMethod,
  ) {
    const key = state().authDrafts[providerId]?.trim();
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
        metadata: { login: method.login },
      });
      await refreshProviderSurface(providerId);
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        settingsNotice: ok ? t("connected") : t("notConfigured"),
        authDrafts: { ...previous.authDrafts, [providerId]: "" },
        providerAuthPanel:
          ok && previous.providerAuthPanel?.providerId === providerId
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
        window.open(result.url, "_blank", "noopener,noreferrer");
      }
      await refreshProviderSurface(providerId);
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        settingsNotice: result.instructions,
      }));
      if (result.method === "auto") {
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

  async function completeProviderLogin(
    providerId: string,
    code?: string,
    methodIndex = 0,
  ) {
    setState((previous) => ({
      ...previous,
      settingsSaving: true,
      error: undefined,
    }));
    try {
      const ok = await rootClient().providerOauthCallback(providerId, {
        method: methodIndex,
        code: code?.trim() || undefined,
      });
      await refreshProviderSurface(providerId);
      setState((previous) => ({
        ...previous,
        settingsSaving: false,
        settingsNotice: ok ? t("connected") : t("loginPending"),
        authCodeDrafts: { ...previous.authCodeDrafts, [providerId]: "" },
        providerAuthPanel:
          ok && previous.providerAuthPanel?.providerId === providerId
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
    startProviderLogin,
    completeProviderLogin,
    logoutProvider,
  };
}
