import { CliUsageError } from "../types/common.js";
import type { SessionConfig } from "../types/config.js";
import { t } from "../i18n.js";

export type CommandRunShell = "bash" | "zsh" | "shell_command";

export interface RuntimeConfigOverrides {
  model?: string;
  agent?: string;
  sessionType?: string;
  modelVariant?: string;
  modelAccelerationEnabled?: boolean;
  killProcessesOnStart?: boolean;
  validatorEnabled?: boolean;
  commandRunShell?: CommandRunShell;
}

export function parseConfigAssignment(entry: string): [string, unknown] {
  const [key, ...rest] = entry.split("=");
  if (!key || rest.length === 0) throw new CliUsageError(t("invalidConfigAssignment", { entry }));
  return [key.trim(), parseConfigValue(rest.join("="))];
}

export function parseConfigValue(value: string): unknown {
  const trimmed = value.trim();
  if (trimmed === "true") return true;
  if (trimmed === "false") return false;
  if (/^-?\d+(\.\d+)?$/.test(trimmed)) return Number(trimmed);
  try {
    return JSON.parse(trimmed);
  } catch {
    return trimmed;
  }
}

export function sessionConfigPatchFromAssignments(assignments: string[]): Partial<SessionConfig> {
  const patch: Partial<SessionConfig> = {};
  for (const entry of assignments) {
    const [key, value] = parseConfigAssignment(entry);
    assignSessionConfigValue(patch, key, value);
  }
  return patch;
}

export function runtimeOverridesFromAssignment(entry: string): RuntimeConfigOverrides {
  const [key, value] = parseConfigAssignment(entry);
  const overrides: RuntimeConfigOverrides = {};
  assignRuntimeConfigValue(overrides, key, value);
  return overrides;
}

export function mergeRuntimeOverrides(
  left: RuntimeConfigOverrides,
  right: RuntimeConfigOverrides,
): RuntimeConfigOverrides {
  return { ...left, ...right };
}

function assignSessionConfigValue(
  patch: Partial<SessionConfig>,
  key: string,
  value: unknown,
): void {
  const canonical = canonicalKey(key);
  if (canonical === "agent") {
    patch.active_agent = stringValue(value);
  } else if (canonical === "model_variant") {
    patch.model_variant = stringValue(value);
  } else if (canonical === "model_acceleration_enabled") {
    patch.model_acceleration_enabled = booleanValue(value, key);
  } else if (canonical === "service_tier") {
    patch.model_acceleration_enabled = serviceTierAcceleration(value);
  } else if (canonical === "context_message_limit") {
    patch.context_message_limit = numberValue(value, key);
  } else if (canonical === "command_run_stall_guard_check_secs") {
    patch.command_run_stall_guard_check_secs = numberValue(value, key);
  } else if (canonical === "command_run_stall_guard_identical_checks") {
    patch.command_run_stall_guard_identical_checks = numberValue(value, key);
  } else if (canonical === "kill_processes_on_start") {
    patch.kill_processes_on_start = booleanValue(value, key);
  } else if (canonical === "validator_enabled") {
    patch.validator_enabled = booleanValue(value, key);
  } else if (canonical === "show_command_instructions") {
    patch.show_command_instructions = booleanValue(value, key);
  } else if (canonical === "model") {
    patch.model = stringValue(value);
  } else if (canonical === "active_model") {
    patch.active_model = stringValue(value);
  } else if (canonical === "active_provider") {
    patch.active_provider = stringValue(value);
  } else if (canonical === "session_type") {
    patch.session_type = stringValue(value);
  } else if (canonical === "language") {
    patch.language = stringValue(value);
  } else if (canonical === "command_run_stall_guard_profile") {
    patch.command_run_stall_guard_profile = stringValue(value);
  } else {
    throw new CliUsageError(t("unsupportedSessionConfigKey", { key }));
  }
}

function assignRuntimeConfigValue(
  overrides: RuntimeConfigOverrides,
  key: string,
  value: unknown,
): void {
  const canonical = canonicalKey(key);
  if (canonical === "model") overrides.model = stringValue(value);
  else if (canonical === "agent") overrides.agent = stringValue(value);
  else if (canonical === "session_type") overrides.sessionType = stringValue(value);
  else if (canonical === "model_variant") overrides.modelVariant = stringValue(value);
  else if (canonical === "model_acceleration_enabled")
    overrides.modelAccelerationEnabled = booleanValue(value, key);
  else if (canonical === "service_tier")
    overrides.modelAccelerationEnabled = serviceTierAcceleration(value);
  else if (canonical === "kill_processes_on_start")
    overrides.killProcessesOnStart = booleanValue(value, key);
  else if (canonical === "validator_enabled") overrides.validatorEnabled = booleanValue(value, key);
  else if (canonical === "command_run_shell") overrides.commandRunShell = shellValue(value, key);
}

function canonicalKey(key: string): string {
  const normalized = key.trim().replace(/-/g, "_");
  if (normalized === "agent" || normalized === "active_agent") return "agent";
  if (
    normalized === "reasoning_effort" ||
    normalized === "model_reasoning_effort" ||
    normalized === "variant"
  )
    return "model_variant";
  if (
    normalized === "show_commands" ||
    normalized === "show_command" ||
    normalized === "display_commands"
  )
    return "show_command_instructions";
  if (
    normalized === "acceleration" ||
    normalized === "accelerated" ||
    normalized === "model_acceleration"
  ) {
    return "model_acceleration_enabled";
  }
  return normalized;
}

function stringValue(value: unknown): string {
  return String(value).trim();
}

function numberValue(value: unknown, key: string): number {
  const number = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(number)) throw new CliUsageError(t("valueRequiresNumber", { key }));
  return number;
}

function booleanValue(value: unknown, key: string): boolean {
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  const normalized = String(value).trim().toLowerCase();
  if (["true", "1", "yes", "on", "enabled", "priority"].includes(normalized)) return true;
  if (["false", "0", "no", "off", "disabled", "auto", "default", "standard"].includes(normalized))
    return false;
  throw new CliUsageError(t("valueRequiresBoolean", { key }));
}

function serviceTierAcceleration(value: unknown): boolean {
  return booleanValue(value, "service_tier");
}

export function shellValue(value: unknown, key = "command_run_shell"): CommandRunShell {
  const normalized = String(value).trim();
  if (normalized === "bash") return "bash";
  if (normalized === "zsh") return "zsh";
  if (normalized === "shll") return "shell_command";
  throw new CliUsageError(t("unsupportedCommandRunShell", { key, value: normalized }));
}
