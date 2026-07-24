export const SETTING_DETAILS = [
  "model",
  "provider",
  "agent",
  "persona",
  "language",
  "variant",
  "priority",
  "autoGitCommit",
  "maximumRuntimeLlmTurns",
  "maximumParallelRuntimeWorkers",
  "about",
] as const;

export type HiddenSettingDetail = "session" | "validator" | "stallGuard";
export type SettingDetail = (typeof SETTING_DETAILS)[number] | HiddenSettingDetail | "providerAuth";

export const DEFAULT_MAXIMUM_RUNTIME_LLM_TURNS = 256;
export const MAXIMUM_RUNTIME_LLM_TURN_OPTIONS = [64, 128, 256, 1080, 2560] as const;
export const DEFAULT_MAXIMUM_PARALLEL_RUNTIME_WORKERS = 24;
export const MAXIMUM_PARALLEL_RUNTIME_WORKER_OPTIONS = [6, 12, 24, 48, 128] as const;
