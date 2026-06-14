export const SETTING_DETAILS = [
  "model",
  "provider",
  "agent",
  "persona",
  "language",
  "session",
  "variant",
  "priority",
  "commands",
  "validator",
  "stallGuard",
] as const;

export type SettingDetail = (typeof SETTING_DETAILS)[number] | "providerAuth";
