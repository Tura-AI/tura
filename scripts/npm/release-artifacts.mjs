import { existsSync } from "node:fs";
import path from "node:path";

export const commandNames = [
  "tura-command-generate-media",
  "tura-command-read-media",
  "tura-command-web-discover"
];

export const executableNames = [
  "tura",
  "tura_exec",
  "tura_gateway",
  "tura_router",
  "tura_session_db",
  "tura_runtime",
  ...commandNames
];

export const requiredPackageFiles = [
  "crates/provider/config/provider_config.json",
  "assets/tura/icon.ico"
];

export const releaseConfigFiles = [
  ["crates/provider/config/provider_config.json", "config/provider_config.json"]
];

export const releaseRuntimeFiles = [
  ["agents/src", "agents/src"],
  ["personas/src", "personas/src"],
  ["crates/runtime/src/runtime_prompt", "crates/runtime/src/runtime_prompt"],
  ["crates/tools/src/commands", "crates/tools/src/commands"],
  ["crates/tools/src/command_run/schema.json", "crates/tools/src/command_run/schema.json"],
  ["commands/generate_media", "commands/generate_media"],
  ["commands/read_media", "commands/read_media"],
  ["commands/web_discover", "commands/web_discover"],
  ["README.md", "README.md"],
  ["scripts/ARCHITECTURE.md", "scripts/ARCHITECTURE.md"],
  ["scripts/register-cli.ps1", "scripts/register-cli.ps1"],
  ["scripts/register-cli.sh", "scripts/register-cli.sh"],
  ["scripts/unregister-cli.ps1", "scripts/unregister-cli.ps1"],
  ["scripts/unregister-cli.sh", "scripts/unregister-cli.sh"]
];

export const requiredReleaseRuntimeFiles = [
  "agents/src/balanced/agent_config.json",
  "agents/src/balanced/prompt.md",
  "agents/src/direct/agent_config.json",
  "agents/src/direct/prompt.md",
  "agents/src/direct-text-only/agent_config.json",
  "agents/src/direct-text-only/prompt.md",
  "personas/src/communication_style/communication_style.md",
  "personas/src/communication_style/cli_communication_style.md",
  "personas/src/expression_manifest.json",
  "personas/src/pidan/persona_config.json",
  "personas/src/pidan/prompt/persona.md",
  "personas/src/tura/persona_config.json",
  "personas/src/tura/prompt/persona.md",
  "personas/src/wonderful/persona_config.json",
  "personas/src/wonderful/prompt/persona.md",
  "crates/runtime/src/runtime_prompt/data_research/prompt_identity.json",
  "crates/runtime/src/runtime_prompt/data_research/prompt.md",
  "crates/runtime/src/runtime_prompt/debug/prompt_identity.json",
  "crates/runtime/src/runtime_prompt/debug/prompt.md",
  "crates/runtime/src/runtime_prompt/devops/prompt_identity.json",
  "crates/runtime/src/runtime_prompt/devops/prompt.md",
  "crates/runtime/src/runtime_prompt/editorial/prompt_identity.json",
  "crates/runtime/src/runtime_prompt/editorial/prompt.md",
  "crates/runtime/src/runtime_prompt/frontend/prompt_identity.json",
  "crates/runtime/src/runtime_prompt/frontend/prompt.md",
  "crates/runtime/src/runtime_prompt/interactive_and_3d/prompt_identity.json",
  "crates/runtime/src/runtime_prompt/interactive_and_3d/prompt.md",
  "crates/runtime/src/runtime_prompt/new_build/prompt_identity.json",
  "crates/runtime/src/runtime_prompt/new_build/prompt.md",
  "crates/runtime/src/runtime_prompt/refactoring/prompt_identity.json",
  "crates/runtime/src/runtime_prompt/refactoring/prompt.md",
  "crates/runtime/src/runtime_prompt/visual/prompt_identity.json",
  "crates/runtime/src/runtime_prompt/visual/prompt.md",
  "crates/runtime/src/runtime_prompt/website/prompt_identity.json",
  "crates/runtime/src/runtime_prompt/website/prompt.md"
];

export const releaseRuntimeExcludedDirs = [
  ".venv",
  "tests",
  "target",
  "node_modules",
  "__pycache__",
  ".pytest_cache"
];

export const supportedPlatformPackages = [
  ["win32", "x64"],
  ["linux", "x64"],
  ["darwin", "x64"],
  ["darwin", "arm64"]
];

export function platformPackageName(platform = process.platform, arch = process.arch) {
  platformTriple(platform, arch);
  return `tura-${platform}-${arch}`;
}

export function executableName(name, platform = process.platform) {
  return platform === "win32" ? `${name}.exe` : name;
}

export function platformTriple(platform = process.platform, arch = process.arch) {
  const platformName = new Map([
    ["win32", "windows"],
    ["darwin", "macos"],
    ["linux", "linux"]
  ]).get(platform);
  const archName = new Map([
    ["x64", "x64"],
    ["arm64", "arm64"]
  ]).get(arch);

  if (!platformName || !archName) {
    throw new Error(`Unsupported npm release platform: ${platform}-${arch}`);
  }
  return `${platformName}-${archName}`;
}

export function archiveExtension(platform = process.platform) {
  return platform === "win32" ? "zip" : "tar.gz";
}

export function releaseTag(version) {
  return process.env.TURA_NPM_RELEASE_TAG || `v${version}`;
}

export function releaseArchiveName(version, platform = process.platform, arch = process.arch) {
  return `tura-${releaseTag(version)}-${platformTriple(platform, arch)}.${archiveExtension(platform)}`;
}

export function releaseRoot(root) {
  return path.join(root, "target", "release");
}

export function releaseOutputRoot(root) {
  return path.join(root, "release");
}

export function guiDistCandidates(root) {
  const releaseDir = releaseRoot(root);
  return [
    path.join(releaseDir, "tura_gui"),
    path.join(releaseDir, "tura_gui_dist")
  ];
}

export function bundleCandidates(root) {
  const releaseDir = releaseRoot(root);
  return [
    path.join(releaseDir, "bundle"),
    path.join(releaseDir, "release", "bundle")
  ];
}

export function firstExistingPath(candidates) {
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

export function requiredReleaseFiles(root, platform = process.platform) {
  const releaseDir = releaseRoot(root);
  const files = executableNames.map((name) => path.join(releaseDir, executableName(name, platform)));
  files.push(path.join(releaseDir, "config", "provider_config.json"));
  const guiDist = firstExistingPath(guiDistCandidates(root));
  files.push(guiDist ? path.join(guiDist, "index.html") : path.join(releaseDir, "tura_gui", "index.html"));
  return files;
}

export function requiredDesktopReleaseFiles(root, platform = process.platform) {
  const releaseDir = releaseRoot(root);
  return [
    path.join(releaseDir, executableName("tura_gui", platform)),
    firstExistingPath(bundleCandidates(root)) ?? path.join(releaseDir, "bundle")
  ];
}

export function missingReleaseFiles(root, platform = process.platform) {
  return requiredReleaseFiles(root, platform).filter((file) => !existsSync(file));
}

export function requiredReleaseRuntimeConfigFiles(root) {
  const releaseDir = releaseRoot(root);
  return requiredReleaseRuntimeFiles.map((file) => path.join(releaseDir, file));
}

export function missingReleaseRuntimeFiles(root) {
  return requiredReleaseRuntimeConfigFiles(root).filter((file) => !existsSync(file));
}

export function missingPackageFiles(root) {
  return requiredPackageFiles.filter((file) => !existsSync(path.join(root, file)));
}
