export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };
export type JsonObject = { [key: string]: JsonValue };

export const CLI_METADATA_SCHEMA = "tura.benchmark.cli-metadata.v1";
export const ROUND_SCHEMA = "tura.benchmark.agent-round.v1";
export const TASK_REPORT_SCHEMA = "tura.benchmark.task-report.v1";
export const HARNESS_REPORT_SCHEMA = "tura.benchmark.harness-report.v1";
export const TASK_DECLARATION_SCHEMA = "tura.benchmark.task-declaration.v1";

export type BenchmarkTaskType = "build" | "debug" | "refactoring";

export interface TokenUsage {
  inputTokens: number;
  cacheInputTokens: number;
  outputTokens: number;
  reasoningTokens: number;
  totalTokens: number;
}

export interface BenchmarkCliInstruction {
  commandName: string;
  commandLine: string;
  args: string[];
  cwd?: string;
  env?: Record<string, string>;
  raw?: JsonValue;
}

export interface BenchmarkSoftwareMetadata {
  platform: NodeJS.Platform;
  arch: string;
  nodeVersion: string;
  systemSoftwareVersion: string;
  packageName?: string;
  packageVersion?: string;
  gitHead?: string;
}

export interface BenchmarkAgentMetadata {
  agentId: string;
  agentName: string;
  agentVersion: string;
  agentApplicationVersion: string;
  cliLaunchCommandName: string;
  cliCommand: string;
  pluginSkillGithubUrls: string[];
  releaseDownloadUrl: string | null;
  releaseSha256: string | null;
}

export interface BenchmarkCliMetadata {
  schema: typeof CLI_METADATA_SCHEMA;
  software: BenchmarkSoftwareMetadata;
  agent: BenchmarkAgentMetadata;
  createdAt: string;
}

export type BenchmarkToolCallKind = "tool" | "command";

export interface BenchmarkMessage {
  role: string;
  text: string;
}

export interface BenchmarkToolCall {
  id: string;
  kind: BenchmarkToolCallKind;
  name: string;
  commandLine: string;
  arguments: JsonValue;
  parentToolName?: string;
  parentToolCallId?: string;
  parallelGroupId?: string | number;
  durationMs?: number;
  startedAt?: string;
  endedAt?: string;
  raw?: JsonValue;
}

export interface BenchmarkAgentRoundMetadata {
  agentId: string;
  agentKind: string;
  agentMode: string;
  model: string;
  reasoning: string;
  serviceTier: string;
  priorityEnabled: boolean;
  roundSource: string;
  eventType: string;
  sessionOrTurnId: string;
  fixtureBackend?: string | null;
  fixtureSourcePath?: string | null;
  sourceAgentId?: string | null;
  sourceEventType?: string | null;
  sourceRoundIndex?: number | null;
  durationSource?: string | null;
  usageEventSource?: string | null;
  compactSummaryCount?: number;
  compactSummaryTokenIncluded?: boolean;
}

export interface BenchmarkRoundSources {
  stdoutPath?: string | null;
  providerCallsPath?: string | null;
  providerLogPath?: string | null;
  summaryPath?: string | null;
}

export interface BenchmarkAgentRound {
  schema: typeof ROUND_SCHEMA;
  roundId: string;
  roundIndex: number;
  startedAt: string;
  endedAt: string;
  input: {
    fullContext: string;
    messages?: BenchmarkMessage[];
  };
  output: {
    fullOutput: string;
    assistantMessage: string;
    messages?: BenchmarkMessage[];
  };
  messages: BenchmarkMessage[];
  usage: TokenUsage;
  providerDurationMs: number;
  toolCalls: BenchmarkToolCall[];
  sources?: BenchmarkRoundSources;
  metadata: BenchmarkAgentRoundMetadata;
  rawCallbackPath?: string;
}

export interface BenchmarkRepoSnapshot {
  repoRoot: string;
  gitHead?: string;
  gitStatusShort?: string;
  capturedAt: string;
  snapshotPath: string;
}

export interface BenchmarkTaskReportMetadata {
  startedAt: string;
  endedAt: string;
  agentVersion: string;
  agentCliCommand: string;
}

export interface BenchmarkTaskReport {
  schema: typeof TASK_REPORT_SCHEMA;
  runId: string;
  taskId: string;
  agentId: string;
  metadata: BenchmarkTaskReportMetadata;
  usage: TokenUsage & {
    providerDurationMs: number;
    llmRoundCount: number;
  };
  harnessScore: number | null;
  gitDiff: string;
  gitDiffPath?: string;
  harnessDirectory: string;
  startRepoSnapshot: BenchmarkRepoSnapshot;
  cliMetadataPath: string;
  roundsDirectory: string;
  rounds: BenchmarkAgentRound[];
  sourceSummaryPath?: string;
}

export interface BenchmarkHarnessScore {
  harnessId: string;
  score: number;
  maxScore?: number;
  passed: boolean;
  details?: JsonValue;
  artifacts?: string[];
}

export interface BenchmarkHarnessReport {
  schema: typeof HARNESS_REPORT_SCHEMA;
  runId: string;
  taskId: string;
  harnessDirectory: string;
  scores: BenchmarkHarnessScore[];
  finalScore: number | null;
  createdAt: string;
}

export interface BenchmarkTaskVariantDeclaration {
  id: string;
  label: string;
  runner: string;
  env?: Record<string, string>;
  default?: boolean;
}

export interface BenchmarkTaskDeclaration {
  schema: typeof TASK_DECLARATION_SCHEMA;
  id: string;
  type: BenchmarkTaskType;
  title: string;
  directory: string;
  summary: string;
  contract: {
    cliMetadata: typeof CLI_METADATA_SCHEMA;
    round: typeof ROUND_SCHEMA;
    taskReport: typeof TASK_REPORT_SCHEMA;
    harnessReport: typeof HARNESS_REPORT_SCHEMA;
  };
  variants: BenchmarkTaskVariantDeclaration[];
  legacyScripts: string[];
  duplicatePolicy: "merged-variant" | "none";
}

export function emptyUsage(): TokenUsage {
  return {
    inputTokens: 0,
    cacheInputTokens: 0,
    outputTokens: 0,
    reasoningTokens: 0,
    totalTokens: 0,
  };
}

export function addUsage(values: TokenUsage[]): TokenUsage {
  return values.reduce((total, usage) => ({
    inputTokens: total.inputTokens + usage.inputTokens,
    cacheInputTokens: total.cacheInputTokens + usage.cacheInputTokens,
    outputTokens: total.outputTokens + usage.outputTokens,
    reasoningTokens: total.reasoningTokens + usage.reasoningTokens,
    totalTokens: total.totalTokens + usage.totalTokens,
  }), emptyUsage());
}
