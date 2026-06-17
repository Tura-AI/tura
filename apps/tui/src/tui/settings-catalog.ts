export const SETTING_DETAILS = [
  "model",
  "provider",
  "agent",
  "persona",
  "language",
  "variant",
  "priority",
] as const;

export type HiddenSettingDetail = "session" | "commands" | "validator" | "stallGuard";
export type SettingDetail = (typeof SETTING_DETAILS)[number] | HiddenSettingDetail | "providerAuth";
