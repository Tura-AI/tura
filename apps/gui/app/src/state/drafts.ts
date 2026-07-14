import type { PollInterval, PlanStatus, StartCondition } from "@tura/gateway-sdk";
import type { ComposerImage, SettingsSection, ThemeMode } from "./global-store";

export type DraftState = {
  issueDraft: string;
  issueSearch: string;
  planDraftLane?: PlanStatus;
  planDraftStartCondition: StartCondition;
  planDraftStartAt: string;
  planDraftPollInterval: PollInterval;
  planDraftSessionId?: string;
  planPreviewSessionId?: string;
  composerText: string;
  composerImages: ComposerImage[];
  configDraft: Record<string, string>;
  workspaceConfigDraft: Record<string, string>;
  authDrafts: Record<string, string>;
  authCodeDrafts: Record<string, string>;
  providerSearch: string;
  settingsSection: SettingsSection;
  themeMode: ThemeMode;
};

export function draftStateDefaults(): DraftState {
  return {
    issueDraft: "",
    issueSearch: "",
    planDraftStartCondition: "user_action",
    planDraftStartAt: "",
    planDraftPollInterval: { m: 0, d: 0, h: 1, s: 0 },
    composerText: "",
    composerImages: [],
    configDraft: {},
    workspaceConfigDraft: {},
    authDrafts: {},
    authCodeDrafts: {},
    providerSearch: "",
    settingsSection: "appearance",
    themeMode: "light",
  };
}
