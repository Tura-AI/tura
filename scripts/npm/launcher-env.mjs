import { existsSync } from "node:fs";
import path from "node:path";

export const requiredRuntimeResources = [
  "config/provider_config.json",
  "agents/src",
  "personas/src",
  "crates/tools/src/commands",
  "crates/tools/src/command_run/schema.json",
  "commands",
];

export function missingRuntimeResources(releaseDir) {
  return requiredRuntimeResources
    .map((relativePath) => path.join(releaseDir, relativePath))
    .filter((candidate) => !existsSync(candidate));
}

export function launchEnvironment({
  baseEnv = process.env,
  providerConfig,
  releaseBin,
  releaseDir,
}) {
  return {
    ...baseEnv,
    TURA_RELEASE_BIN_DIR:
      baseEnv.TURA_RELEASE_BIN_DIR || path.dirname(releaseBin),
    // Packaged agents, personas, commands, capability schemas, and provider
    // config live beside the native executables in the platform package.
    TURA_PROJECT_ROOT: baseEnv.TURA_PROJECT_ROOT || releaseDir,
    ...(providerConfig
      ? { TURA_PROVIDER_CONFIG: baseEnv.TURA_PROVIDER_CONFIG || providerConfig }
      : {}),
  };
}
