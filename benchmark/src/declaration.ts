import { readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";

import {
  CLI_METADATA_SCHEMA,
  HARNESS_REPORT_SCHEMA,
  ROUND_SCHEMA,
  TASK_DECLARATION_SCHEMA,
  TASK_REPORT_SCHEMA,
  type BenchmarkTaskDeclaration,
  type BenchmarkTaskType,
} from "./contracts.js";

const TASK_TYPES = new Set<BenchmarkTaskType>(["build", "debug", "refactoring"]);

export async function discoverTaskDeclarations(root: string): Promise<BenchmarkTaskDeclaration[]> {
  const declarations: BenchmarkTaskDeclaration[] = [];
  for (const type of TASK_TYPES) {
    const typeDirectory = path.join(root, type);
    if (!(await isDirectory(typeDirectory))) continue;
    for (const entry of await readdir(typeDirectory, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const declarationPath = path.join(typeDirectory, entry.name, "benchmark.task.json");
      declarations.push(await readTaskDeclaration(declarationPath));
    }
  }
  return declarations.sort((left, right) => left.id.localeCompare(right.id));
}

export async function readTaskDeclaration(filePath: string): Promise<BenchmarkTaskDeclaration> {
  const declaration = JSON.parse(await readFile(filePath, "utf8")) as BenchmarkTaskDeclaration;
  validateTaskDeclaration(declaration, path.dirname(filePath));
  return declaration;
}

export function validateTaskDeclaration(declaration: BenchmarkTaskDeclaration, directory?: string): void {
  if (declaration.schema !== TASK_DECLARATION_SCHEMA) throw new Error(`invalid benchmark declaration schema for ${declaration.id}`);
  if (!TASK_TYPES.has(declaration.type)) throw new Error(`invalid benchmark task type for ${declaration.id}`);
  if (!declaration.id || !declaration.title || !declaration.directory) throw new Error("benchmark declaration identity is incomplete");
  if (declaration.contract.cliMetadata !== CLI_METADATA_SCHEMA) throw new Error(`invalid cli metadata contract for ${declaration.id}`);
  if (declaration.contract.round !== ROUND_SCHEMA) throw new Error(`invalid round contract for ${declaration.id}`);
  if (declaration.contract.taskReport !== TASK_REPORT_SCHEMA) throw new Error(`invalid task report contract for ${declaration.id}`);
  if (declaration.contract.harnessReport !== HARNESS_REPORT_SCHEMA) throw new Error(`invalid harness report contract for ${declaration.id}`);
  if (!Array.isArray(declaration.variants) || declaration.variants.length === 0) throw new Error(`no variants declared for ${declaration.id}`);
  if (declaration.variants.filter((variant) => variant.default).length > 1) throw new Error(`multiple default variants for ${declaration.id}`);
  for (const variant of declaration.variants) {
    if (!variant.id || !variant.label || !variant.runner) throw new Error(`invalid variant declaration for ${declaration.id}`);
    if (directory && path.isAbsolute(variant.runner)) throw new Error(`variant runner must be relative for ${declaration.id}`);
  }
}

async function isDirectory(directory: string): Promise<boolean> {
  try {
    return (await stat(directory)).isDirectory();
  } catch {
    return false;
  }
}
