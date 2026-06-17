import { sessionConfigPatchFromAssignments } from "../../commands/config-values.js";
import { isDraftSession } from "../../types/session.js";
import type { AppState, SettingDetail } from "../reducer.js";
import { settingOptions, settingsEntries } from "../render.js";

export interface PromptRuntimeSelection {
  model?: string;
  agent?: string;
  modelVariant?: string;
  modelAccelerationEnabled?: boolean;
}

const DEFAULT_MODEL_ACCELERATION_ENABLED = true;

export function selectedModel(state: AppState): string | undefined {
  let row = 0;
  for (const provider of state.providers?.all ?? []) {
    for (const [model] of Object.entries(provider.models ?? {})) {
      if (row === state.selectedModelIndex) return `${provider.id}/${model}`;
      row += 1;
    }
  }
  return undefined;
}

export function selectedPersonaID(state: AppState): string | undefined {
  const persona = state.personas[state.selectedPersonaIndex];
  const configName = persona?.config?.persona_name;
  return persona?.summary?.id ?? (typeof configName === "string" ? configName : undefined);
}

export function selectedSettingDetail(state: AppState): SettingDetail | undefined {
  return settingsEntries(state)[state.selectedSettingsIndex]?.detail;
}

export function settingPatch(
  detail: SettingDetail,
  value: unknown,
): Record<string, unknown> | undefined {
  if (detail === "model" && typeof value === "string") {
    return sessionConfigPatchFromAssignments([`model=${value}`]);
  }
  if (detail === "provider" && typeof value === "string") {
    return { active_provider: value };
  }
  if (detail === "agent" && typeof value === "string") {
    return { active_agent: value };
  }
  if (detail === "persona" && typeof value === "string") {
    return { active_persona: value };
  }
  if (detail === "language" && typeof value === "string") {
    return { language: value };
  }
  if (detail === "session" && typeof value === "string") {
    return { session_type: value };
  }
  if (detail === "variant" && typeof value === "string") {
    return { model_variant: value };
  }
  if (detail === "priority") {
    return { model_acceleration_enabled: Boolean(value) };
  }
  if (detail === "commands") {
    return { show_command_instructions: Boolean(value) };
  }
  if (detail === "validator") {
    return { validator_enabled: Boolean(value) };
  }
  if (detail === "stallGuard" && typeof value === "string") {
    return { command_run_stall_guard_profile: value };
  }
  return undefined;
}

export function selectedSettingOption(state: AppState): [string, string, unknown] | undefined {
  return settingOptions(state)[state.selectedSettingOptionIndex];
}

export function promptRuntimeSelection(state: AppState): PromptRuntimeSelection {
  if (isDraftSession(state.session)) {
    return {
      model: stringOrUndefined(state.session?.model) ?? configuredModel(state),
      agent:
        stringOrUndefined(state.session?.agent) ??
        stringOrUndefined(state.sessionConfig?.active_agent),
      modelVariant:
        stringOrUndefined(state.session?.model_variant) ??
        stringOrUndefined(state.sessionConfig?.model_variant),
      modelAccelerationEnabled:
        state.session?.model_acceleration_enabled ??
        state.sessionConfig?.model_acceleration_enabled ??
        DEFAULT_MODEL_ACCELERATION_ENABLED,
    };
  }
  return {
    model: stringOrUndefined(state.session?.model) ?? configuredModel(state),
    agent:
      stringOrUndefined(state.sessionConfig?.active_agent) ??
      stringOrUndefined(state.session?.agent),
    modelVariant:
      stringOrUndefined(state.sessionConfig?.model_variant) ??
      stringOrUndefined(state.session?.model_variant),
    modelAccelerationEnabled:
      state.sessionConfig?.model_acceleration_enabled ??
      state.session?.model_acceleration_enabled ??
      DEFAULT_MODEL_ACCELERATION_ENABLED,
  };
}

function configuredModel(state: AppState): string | undefined {
  const config = state.sessionConfig;
  const model = stringOrUndefined(config?.model);
  if (model?.includes("/")) return model;
  const provider = stringOrUndefined(config?.active_provider);
  const activeModel = stringOrUndefined(config?.active_model);
  if (provider && activeModel) return `${provider}/${activeModel}`;
  return undefined;
}

function stringOrUndefined(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  return trimmed ? trimmed : undefined;
}
