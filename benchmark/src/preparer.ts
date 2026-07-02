import { spawn, spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { cp, mkdir, rm } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  CLI_METADATA_SCHEMA,
  type BenchmarkAgentMetadata,
  type BenchmarkCliInstruction,
  type BenchmarkCliMetadata,
  type BenchmarkRepoSnapshot,
  type BenchmarkSoftwareMetadata,
  type JsonValue,
} from "./contracts.js";
import { ensureDirectory, writeJsonFile, writeTextFile } from "./io.js";

export interface BenchmarkEnvironmentConfig {
  benchmarkId: string;
  runId: string;
  repoRoot: string;
  runRoot: string;
  fixtureDirectory?: string;
  workspaceDirectory?: string;
  harnessDirectory?: string;
}

export interface PreparedBenchmarkEnvironment {
  benchmarkId: string;
  runId: string;
  repoRoot: string;
  runRoot: string;
  workspaceDirectory: string;
  harnessDirectory: string;
  startRepoSnapshot: BenchmarkRepoSnapshot;
}

export interface AgentLaunchConfig {
  agentId: string;
  agentName?: string;
  agentVersion: string;
  agentApplicationVersion?: string;
  cliLaunchCommandName: string;
  cliArgs?: string[];
  pluginSkillGithubUrls?: string[];
  releaseDownloadUrl?: string;
  releaseSha256?: string;
  appendInstruction?: boolean;
}

export interface AgentRunRequest {
  agentId: string;
  workspaceDirectory: string;
  commandName: string;
  args: string[];
  cliCommand: string;
  instruction: BenchmarkCliInstruction;
  cliMetadata: BenchmarkCliMetadata;
}

export interface AgentRunResult {
  exitCode: number | null;
  signal: NodeJS.Signals | null;
  timedOut: boolean;
  stdoutPath: string;
  stderrPath: string;
  durationMs: number;
}

export async function prepareBenchmarkEnvironment(
  config: BenchmarkEnvironmentConfig,
): Promise<PreparedBenchmarkEnvironment> {
  const workspaceDirectory = config.workspaceDirectory ?? path.join(config.runRoot, "workspace");
  const harnessDirectory = config.harnessDirectory ?? path.join(config.runRoot, "harness");
  await rm(workspaceDirectory, { recursive: true, force: true });
  await ensureDirectory(config.runRoot);
  await ensureDirectory(harnessDirectory);
  if (config.fixtureDirectory) {
    await cp(config.fixtureDirectory, workspaceDirectory, { recursive: true });
  } else {
    await mkdir(workspaceDirectory, { recursive: true });
  }
  const startRepoSnapshot = await snapshotRepoState(config.repoRoot, path.join(config.runRoot, "start-repo-snapshot.json"));
  const prepared = {
    benchmarkId: config.benchmarkId,
    runId: config.runId,
    repoRoot: config.repoRoot,
    runRoot: config.runRoot,
    workspaceDirectory,
    harnessDirectory,
    startRepoSnapshot,
  };
  await writeJsonFile(path.join(config.runRoot, "prepared-environment.json"), prepared as unknown as JsonValue);
  return prepared;
}

export function buildAgentRunRequest(
  environment: PreparedBenchmarkEnvironment,
  agent: AgentLaunchConfig,
  instruction: BenchmarkCliInstruction,
): AgentRunRequest {
  const args = [...(agent.cliArgs ?? [])];
  if (agent.appendInstruction !== false) args.push(instruction.commandLine);
  const cliCommand = [agent.cliLaunchCommandName, ...args].join(" ");
  const cliMetadata = createCliMetadata(environment.repoRoot, agent, cliCommand);
  return {
    agentId: agent.agentId,
    workspaceDirectory: environment.workspaceDirectory,
    commandName: agent.cliLaunchCommandName,
    args,
    cliCommand,
    instruction,
    cliMetadata,
  };
}

export async function executeAgentRunRequest(
  request: AgentRunRequest,
  outputDirectory: string,
  timeoutMs: number,
): Promise<AgentRunResult> {
  await ensureDirectory(outputDirectory);
  const stdoutPath = path.join(outputDirectory, `${request.agentId}.stdout.jsonl`);
  const stderrPath = path.join(outputDirectory, `${request.agentId}.stderr.log`);
  const started = Date.now();
  let stdout = "";
  let stderr = "";
  let timedOut = false;

  const result = await new Promise<{ exitCode: number | null; signal: NodeJS.Signals | null }>((resolve) => {
    const child = spawn(request.commandName, request.args, {
      cwd: request.workspaceDirectory,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
    }, timeoutMs);
    child.stdout?.setEncoding("utf8");
    child.stderr?.setEncoding("utf8");
    child.stdout?.on("data", (chunk: string) => {
      stdout += chunk;
    });
    child.stderr?.on("data", (chunk: string) => {
      stderr += chunk;
    });
    child.on("close", (exitCode, signal) => {
      clearTimeout(timer);
      resolve({ exitCode, signal });
    });
    child.on("error", (error) => {
      clearTimeout(timer);
      stderr += `${error.stack ?? error.message}\n`;
      resolve({ exitCode: 1, signal: null });
    });
  });

  await writeTextFile(stdoutPath, stdout);
  await writeTextFile(stderrPath, stderr);
  return {
    ...result,
    timedOut,
    stdoutPath,
    stderrPath,
    durationMs: Date.now() - started,
  };
}

export async function snapshotRepoState(repoRoot: string, snapshotPath: string): Promise<BenchmarkRepoSnapshot> {
  const snapshot: BenchmarkRepoSnapshot = {
    repoRoot,
    gitHead: runGit(repoRoot, ["rev-parse", "HEAD"]),
    gitStatusShort: runGit(repoRoot, ["status", "--short"]),
    capturedAt: new Date().toISOString(),
    snapshotPath,
  };
  await writeJsonFile(snapshotPath, snapshot as unknown as JsonValue);
  return snapshot;
}

export async function captureGitDiff(repoRoot: string, diffPath: string): Promise<string> {
  const diff = runGit(repoRoot, ["diff", "--binary"]) ?? "";
  await writeTextFile(diffPath, diff);
  return diff;
}

export function createCliMetadata(
  repoRoot: string,
  agent: AgentLaunchConfig,
  cliCommand: string,
): BenchmarkCliMetadata {
  return {
    schema: CLI_METADATA_SCHEMA,
    software: softwareMetadata(repoRoot),
    agent: agentMetadata(agent, cliCommand),
    createdAt: new Date().toISOString(),
  };
}

function softwareMetadata(repoRoot: string): BenchmarkSoftwareMetadata {
  const packageJson = readPackageJson(repoRoot);
  return {
    platform: process.platform,
    arch: process.arch,
    nodeVersion: process.version,
    systemSoftwareVersion: `${process.platform}/${process.arch} node ${process.version}`,
    packageName: packageJson?.name,
    packageVersion: packageJson?.version,
    gitHead: runGit(repoRoot, ["rev-parse", "HEAD"]),
  };
}

function agentMetadata(agent: AgentLaunchConfig, cliCommand: string): BenchmarkAgentMetadata {
  return {
    agentId: agent.agentId,
    agentName: agent.agentName ?? agent.agentId,
    agentVersion: agent.agentVersion,
    agentApplicationVersion: agent.agentApplicationVersion ?? agent.agentVersion,
    cliLaunchCommandName: agent.cliLaunchCommandName,
    cliCommand,
    pluginSkillGithubUrls: agent.pluginSkillGithubUrls ?? [],
    releaseDownloadUrl: agent.releaseDownloadUrl ?? null,
    releaseSha256: agent.releaseSha256 ?? null,
  };
}

function runGit(repoRoot: string, args: string[]): string | undefined {
  const result = spawnSync("git", args, { cwd: repoRoot, encoding: "utf8", windowsHide: true });
  return result.status === 0 ? result.stdout.trim() : undefined;
}

function readPackageJson(repoRoot: string): { name?: string; version?: string } | undefined {
  try {
    return JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8")) as {
      name?: string;
      version?: string;
    };
  } catch {
    return undefined;
  }
}
