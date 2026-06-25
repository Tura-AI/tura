import { reset, textSecondary } from "../render-terminal.js";

export function secondaryText(value: string): string {
  if (!value) return value;
  return value
    .split(/\r?\n/)
    .map((line) => `${textSecondary}${line.replaceAll(reset, `${reset}${textSecondary}`)}${reset}`)
    .join("\n");
}
