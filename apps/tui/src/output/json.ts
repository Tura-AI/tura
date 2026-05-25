import type { RunResult } from "../types/session.js";

export function printJson(value: unknown): void {
  process.stdout.write(`${JSON.stringify(value, null, 2)}\n`);
}

export function printRunJson(result: RunResult): void {
  printJson(result);
}
