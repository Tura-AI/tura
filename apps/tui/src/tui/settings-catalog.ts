export const SETTING_DETAILS = [
  "model",
  "provider",
  "agent",
  "persona",
  "language",
  "variant",
  "priority",
  "about",
] as const;

export type HiddenSettingDetail = "session" | "validator" | "stallGuard";
export type SettingDetail = (typeof SETTING_DETAILS)[number] | HiddenSettingDetail | "providerAuth";
