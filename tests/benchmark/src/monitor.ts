import path from "node:path";

import {
  TASK_REPORT_SCHEMA,
  addUsage,
  type BenchmarkAgentRound,
  type BenchmarkCliMetadata,
  type BenchmarkRepoSnapshot,
  type BenchmarkTaskReport,
  type JsonValue,
} from "./contracts.js";
import { writeJsonFile } from "./io.js";
import { parseAgentRound, saveAgentRound } from "./parser.js";
import { captureGitDiff } from "./preparer.js";

export interface BenchmarkMonitorConfig {
  runId: string;
  taskId: string;
  agentId: string;
  runDirectory: string;
  repoRoot: string;
  harnessDirectory: string;
  startRepoSnapshot: BenchmarkRepoSnapshot;
  cliMetadata: BenchmarkCliMetadata;
}

export interface FinishTaskOptions {
  harnessScore?: number | null;
  gitDiff?: string;
  gitDiffPath?: string;
  endedAt?: string;
}

export class BenchmarkMonitor {
  readonly roundsDirectory: string;
  readonly cliMetadataPath: string;
  readonly taskReportPath: string;
  private readonly rounds: BenchmarkAgentRound[] = [];
  private readonly startedAt = new Date().toISOString();

  constructor(private readonly config: BenchmarkMonitorConfig) {
    this.roundsDirectory = path.join(config.runDirectory, "rounds");
    this.cliMetadataPath = path.join(config.runDirectory, "cli-metadata.json");
    this.taskReportPath = path.join(config.runDirectory, "task-report.json");
  }

  async startTask(): Promise<void> {
    await writeJsonFile(this.cliMetadataPath, this.config.cliMetadata as unknown as JsonValue);
  }

  async recordRound(callback: unknown): Promise<BenchmarkAgentRound> {
    const round = parseAgentRound(callback, this.rounds.length);
    const rawPath = await saveAgentRound(this.roundsDirectory, round);
    const persisted = { ...round, rawCallbackPath: rawPath };
    this.rounds.push(persisted);
    await writeJsonFile(rawPath, persisted as unknown as JsonValue);
    return persisted;
  }

  async finishTask(options: FinishTaskOptions = {}): Promise<BenchmarkTaskReport> {
    const gitDiffPath = options.gitDiffPath ?? path.join(this.config.runDirectory, "git-diff.patch");
    const gitDiff = options.gitDiff ?? (await captureGitDiff(this.config.repoRoot, gitDiffPath));
    const usage = addUsage(this.rounds.map((round) => round.usage));
    const report: BenchmarkTaskReport = {
      schema: TASK_REPORT_SCHEMA,
      runId: this.config.runId,
      taskId: this.config.taskId,
      agentId: this.config.agentId,
      metadata: {
        startedAt: this.startedAt,
        endedAt: options.endedAt ?? new Date().toISOString(),
        agentVersion: this.config.cliMetadata.agent.agentVersion,
        agentCliCommand: this.config.cliMetadata.agent.cliCommand,
      },
      usage: {
        ...usage,
        providerDurationMs: this.rounds.reduce((total, round) => total + round.providerDurationMs, 0),
        llmRoundCount: this.rounds.length,
      },
      harnessScore: options.harnessScore ?? null,
      gitDiff,
      gitDiffPath,
      harnessDirectory: this.config.harnessDirectory,
      startRepoSnapshot: this.config.startRepoSnapshot,
      cliMetadataPath: this.cliMetadataPath,
      roundsDirectory: this.roundsDirectory,
      rounds: this.rounds,
    };
    await writeJsonFile(this.taskReportPath, report as unknown as JsonValue);
    return report;
  }
}
