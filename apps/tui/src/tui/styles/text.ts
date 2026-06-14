import { gray, reset } from "../render-terminal.js";

export function secondaryText(value: string): string {
  if (!value) return value;
  return value
    .split(/\r?\n/)
    .map((line) => `${gray}${line.replaceAll(reset, `${reset}${gray}`)}${reset}`)
    .join("\n");
}
