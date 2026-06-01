import type {
  AgentUpsertRequest,
  StoredAgent,
  TuraConfigModelPair,
} from "@tura/gateway-sdk";
import type { Setter } from "solid-js";
import {
  AVATAR_WORKSPACE_CONFIG_KEY,
  normalizeAvatarSettings,
} from "../components/avatar/agent-avatar-canvas";
import { SettingsView } from "../pages/settings/settings-view";
import type { AppState } from "../state/global-store";

export function SettingsPageOutlet(props: {
  state: AppState;
  setState: Setter<AppState>;
  onRuntimeSetting: (
    updater: (previous: AppState) => AppState,
    options?: { debounce?: boolean },
  ) => void;
  onModelTier: (tier: string, option: TuraConfigModelPair) => Promise<void>;
  onRefreshAgents: () => Promise<void>;
  onGetAgent: (agentId: string) => Promise<StoredAgent | undefined>;
  onSaveAgent: (
    agentId: string | undefined,
    payload: AgentUpsertRequest,
  ) => Promise<void>;
  onDeleteAgent: (agentId: string) => Promise<void>;
}) {
  return (
    <SettingsView
      state={props.state}
      section={props.state.settingsSection}
      onProvider={(providerId) =>
        props.setState((previous) => ({
          ...previous,
          selectedProviderId: providerId,
        }))
      }
      onModelTier={props.onModelTier}
      onRefreshAgents={props.onRefreshAgents}
      onGetAgent={props.onGetAgent}
      onSaveAgent={props.onSaveAgent}
      onDeleteAgent={props.onDeleteAgent}
      onSavePersonalization={(avatar) =>
        props.onRuntimeSetting((previous) => ({
          ...previous,
          workspaceConfigDraft: {
            ...previous.workspaceConfigDraft,
            [AVATAR_WORKSPACE_CONFIG_KEY]: JSON.stringify(
              normalizeAvatarSettings(avatar),
            ),
          },
        }))
      }
      onConfigureProviders={() =>
        props.setState((previous) => ({
          ...previous,
          settingsSection: "providers",
        }))
      }
      onTheme={(themeMode) =>
        props.onRuntimeSetting((previous) => ({
          ...previous,
          themeMode,
          configDraft: {
            ...previous.configDraft,
            theme: themeMode,
          },
        }))
      }
      onMainFont={(mainFont) =>
        props.onRuntimeSetting((previous) => ({
          ...previous,
          mainFont,
          configDraft: {
            ...previous.configDraft,
            main_font: mainFont,
          },
        }))
      }
      onCodeFont={(codeFont) =>
        props.onRuntimeSetting((previous) => ({
          ...previous,
          codeFont,
          configDraft: {
            ...previous.configDraft,
            code_font: codeFont,
          },
        }))
      }
      onMainFontSize={(mainFontSize) =>
        props.onRuntimeSetting((previous) => ({
          ...previous,
          mainFontSize,
          configDraft: {
            ...previous.configDraft,
            main_font_size: String(mainFontSize),
          },
        }))
      }
      onCodeFontSize={(codeFontSize) =>
        props.onRuntimeSetting((previous) => ({
          ...previous,
          codeFontSize,
          configDraft: {
            ...previous.configDraft,
            code_font_size: String(codeFontSize),
          },
        }))
      }
      onProviderSearch={(providerSearch) =>
        props.setState((previous) => ({ ...previous, providerSearch }))
      }
      onOpenProviderAuth={(providerId) =>
        props.setState((previous) => ({
          ...previous,
          selectedProviderId: providerId,
          providerAuthPanel: { providerId },
        }))
      }
    />
  );
}
