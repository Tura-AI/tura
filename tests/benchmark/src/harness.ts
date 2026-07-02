import path from "node:path";

import {
  HARNESS_REPORT_SCHEMA,
  type BenchmarkHarnessReport,
  type BenchmarkHarnessScore,
  type BenchmarkTaskReport,
  type JsonValue,
} from "./contracts.js";
import { ensureDirectory, writeJsonFile } from "./io.js";

export interface BenchmarkHarnessContext {
  runId: string;
  taskId: string;
  workspaceDirectory: string;
  harnessDirectory: string;
  reportDirectory: string;
}

export interface BenchmarkHarness {
  id: string;
  directory: string;
  score(context: BenchmarkHarnessContext): Promise<BenchmarkHarnessScore>;
}

export async function runHarnesses(
  harnesses: BenchmarkHarness[],
  context: BenchmarkHarnessContext,
): Promise<BenchmarkHarnessReport> {
  await ensureDirectory(context.reportDirectory);
  const scores: BenchmarkHarnessScore[] = [];
  for (const harness of harnesses) {
    const score = await harness.score({ ...context, harnessDirectory: harness.directory });
    scores.push(score);
    await writeJsonFile(
      path.join(context.reportDirectory, `${harness.id}-score.json`),
      score as unknown as JsonValue,
    );
  }
  const report: BenchmarkHarnessReport = {
    schema: HARNESS_REPORT_SCHEMA,
    runId: context.runId,
    taskId: context.taskId,
    harnessDirectory: context.harnessDirectory,
    scores,
    finalScore: aggregateHarnessScore(scores),
    createdAt: new Date().toISOString(),
  };
  await writeJsonFile(path.join(context.reportDirectory, "harness-report.json"), report as unknown as JsonValue);
  return report;
}

export function aggregateHarnessScore(scores: BenchmarkHarnessScore[]): number | null {
  if (scores.length === 0) return null;
  const maxKnown = scores.every((score) => typeof score.maxScore === "number" && score.maxScore > 0);
  if (maxKnown) {
    const earned = scores.reduce((total, score) => total + score.score, 0);
    const possible = scores.reduce((total, score) => total + (score.maxScore ?? 0), 0);
    return possible > 0 ? earned / possible : null;
  }
  return scores.reduce((total, score) => total + score.score, 0) / scores.length;
}

export async function writeUnifiedTaskContract(report: BenchmarkTaskReport, outputPath: string): Promise<void> {
  await writeJsonFile(outputPath, report as unknown as JsonValue);
}
