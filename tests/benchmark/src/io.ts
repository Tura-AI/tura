import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import type { JsonValue } from "./contracts.js";

export async function ensureDirectory(directory: string): Promise<void> {
  await mkdir(directory, { recursive: true });
}

export async function writeJsonFile(filePath: string, value: JsonValue): Promise<void> {
  await ensureDirectory(path.dirname(filePath));
  await writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

export async function readJsonFile<T>(filePath: string): Promise<T> {
  return JSON.parse(await readFile(filePath, "utf8")) as T;
}

export async function writeTextFile(filePath: string, value: string): Promise<void> {
  await ensureDirectory(path.dirname(filePath));
  await writeFile(filePath, value, "utf8");
}

export function stableJsonHash(value: JsonValue): string {
  return createHash("sha256").update(JSON.stringify(value)).digest("hex");
}

export function toPosixPath(value: string): string {
  return value.split(path.sep).join("/");
}
