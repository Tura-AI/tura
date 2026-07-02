import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import type { JsonObject } from "./contracts.js";
import type { AgentLaunchConfig } from "./preparer.js";

export const AGENT_CLI_CONFIG_SCHEMA = "tura.benchmark.agent-cli-config.v1";
export const DEFAULT_BENCHMARK_AGENTS = ["pi", "codex", "claudecode", "opencode", "tura"] as const;

export type BenchmarkAgentId = (typeof DEFAULT_BENCHMARK_AGENTS)[number];

export interface BenchmarkAgentCliProfile {
  id: BenchmarkAgentId;
  aliases: string[];
  agentName: string;
  commandEnv: string;
  defaultCommand: string;
  versionEnv?: string;
  modelEnv?: string;
  defaultModel?: string;
  reasoningEnv?: string;
  defaultReasoning?: string;
  defaultArgs: string[];
  defaultEnv?: Record<string, string>;
  defaultVariables?: Record<string, string>;
  appendInstruction: boolean;
  pluginSkillGithubUrls: string[];
  releaseDownloadUrlEnv?: string;
  releaseSha256Env?: string;
}

export interface BenchmarkAgentCliConfig {
  schema: typeof AGENT_CLI_CONFIG_SCHEMA;
  defaultAgents: BenchmarkAgentId[];
  agents: BenchmarkAgentCliProfile[];
}

export interface ResolveBenchmarkAgentCliOptions {
  workspaceDirectory?: string;
  repoRoot?: string;
  model?: string;
  reasoning?: string;
  variables?: Record<string, string>;
  extraArgs?: string[];
  env?: NodeJS.ProcessEnv;
  agentVersion?: string;
  agentApplicationVersion?: string;
  appendInstruction?: boolean;
}

export function defaultAgentConfigPath(): string {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "config", "agents.json");
}

export async function readAgentCliConfig(configPath = defaultAgentConfigPath()): Promise<BenchmarkAgentCliConfig> {
  const config = JSON.parse(await readFile(configPath, "utf8")) as BenchmarkAgentCliConfig;
  validateAgentCliConfig(config);
  return config;
}

export function validateAgentCliConfig(config: BenchmarkAgentCliConfig): void {
  if (config.schema !== AGENT_CLI_CONFIG_SCHEMA) throw new Error("invalid benchmark agent cli config schema");
  const ids = new Set<string>();
  for (const profile of config.agents) {
    if (!DEFAULT_BENCHMARK_AGENTS.includes(profile.id)) throw new Error(`unsupported benchmark agent id: ${profile.id}`);
    if (ids.has(profile.id)) throw new Error(`duplicate benchmark agent id: ${profile.id}`);
    ids.add(profile.id);
    if (!profile.commandEnv || !profile.defaultCommand) throw new Error(`agent command mapping is incomplete: ${profile.id}`);
    if (!Array.isArray(profile.defaultArgs)) throw new Error(`agent args must be an array: ${profile.id}`);
  }
  for (const id of DEFAULT_BENCHMARK_AGENTS) {
    if (!ids.has(id)) throw new Error(`missing benchmark agent profile: ${id}`);
  }
  for (const id of config.defaultAgents) {
    if (!ids.has(id)) throw new Error(`default benchmark agent is not declared: ${id}`);
  }
}

export function normalizeBenchmarkAgentId(agentId: string, config: BenchmarkAgentCliConfig): BenchmarkAgentId {
  const normalized = agentId.trim().toLowerCase();
  const profile = config.agents.find((candidate) => candidate.id === normalized || candidate.aliases.includes(normalized));
  if (!profile) throw new Error(`unknown benchmark agent: ${agentId}`);
  return profile.id;
}

export function resolveBenchmarkAgentCli(
  agentId: string,
  options: ResolveBenchmarkAgentCliOptions,
  config: BenchmarkAgentCliConfig,
): AgentLaunchConfig {
  const normalizedId = normalizeBenchmarkAgentId(agentId, config);
  const profile = config.agents.find((candidate) => candidate.id === normalizedId);
  if (!profile) throw new Error(`missing benchmark agent profile: ${normalizedId}`);
  const env = options.env ?? process.env;
  const model = options.model ?? readEnv(env, profile.modelEnv) ?? profile.defaultModel ?? "unknown";
  const reasoning = options.reasoning ?? readEnv(env, profile.reasoningEnv) ?? profile.defaultReasoning ?? "medium";
  const variables = {
    workspace: options.workspaceDirectory ?? ".",
    repoRoot: options.repoRoot ?? ".",
    model,
    reasoning,
    ...profile.defaultVariables,
    ...options.variables,
  };
  const cliArgs = [...profile.defaultArgs.map((arg) => expandTemplate(arg, variables)), ...(options.extraArgs ?? [])];
  return {
    agentId: normalizedId,
    agentName: profile.agentName,
    agentVersion: options.agentVersion ?? readEnv(env, profile.versionEnv) ?? model,
    agentApplicationVersion: options.agentApplicationVersion ?? readEnv(env, profile.versionEnv) ?? model,
    cliLaunchCommandName: readEnv(env, profile.commandEnv) ?? profile.defaultCommand,
    cliArgs,
    pluginSkillGithubUrls: profile.pluginSkillGithubUrls,
    releaseDownloadUrl: readEnv(env, profile.releaseDownloadUrlEnv),
    releaseSha256: readEnv(env, profile.releaseSha256Env),
    appendInstruction: options.appendInstruction ?? profile.appendInstruction,
    env: materializeEnv(profile.defaultEnv),
  };
}

export function resolveBenchmarkAgentMatrix(
  agentIds: readonly string[],
  options: ResolveBenchmarkAgentCliOptions,
  config: BenchmarkAgentCliConfig,
): AgentLaunchConfig[] {
  return agentIds.map((agentId) => resolveBenchmarkAgentCli(agentId, options, config));
}

export function agentCliConfigSummary(config: BenchmarkAgentCliConfig): JsonObject {
  return {
    schema: config.schema,
    defaultAgents: [...config.defaultAgents],
    agents: config.agents.map((profile) => ({
      id: profile.id,
      aliases: profile.aliases,
      commandEnv: profile.commandEnv,
      defaultCommand: profile.defaultCommand,
      defaultArgs: profile.defaultArgs,
      modelEnv: profile.modelEnv ?? null,
      defaultModel: profile.defaultModel ?? null,
    })),
  };
}

function readEnv(env: NodeJS.ProcessEnv, name?: string): string | undefined {
  if (!name) return undefined;
  const value = env[name];
  return value && value.trim() ? value : undefined;
}

function expandTemplate(value: string, variables: Record<string, string>): string {
  return value.replace(/\{([A-Za-z0-9_]+)\}/g, (_, name: string) => variables[name] ?? "");
}

function materializeEnv(values?: Record<string, string>): Record<string, string> | undefined {
  if (!values) return undefined;
  return Object.fromEntries(Object.entries(values).filter((entry): entry is [string, string] => Boolean(entry[1])));
}
