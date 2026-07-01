import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "bun:test";

const tokens = readFileSync(resolve(import.meta.dir, "../../../app/src/styles/tokens.css"), "utf8");

function themeBlock(theme?: string): string {
  if (!theme) {
    return tokens.slice(0, tokens.indexOf('html[data-theme="dark"]'));
  }
  const start = tokens.indexOf(`html[data-theme="${theme}"]`);
  expect(start).toBeGreaterThanOrEqual(0);
  const next = tokens.indexOf("html[data-theme=", start + 1);
  return tokens.slice(start, next > start ? next : undefined);
}

function tokenValue(block: string, token: string): string {
  const match = block.match(new RegExp(`${token}:\\s*(#[0-9a-fA-F]{6})`));
  expect(match?.[1]).toBeDefined();
  return match![1]!;
}

describe("theme accent tokens", () => {
  test("uses low-saturation theme accents instead of vivid colors", () => {
    expect(tokenValue(themeBlock(), "--accent")).toBe("#3f4652");
    expect(tokenValue(themeBlock("dark"), "--accent")).toBe("#d8d4ca");
    expect(tokenValue(themeBlock("uruk"), "--accent")).toBe("#6d5148");
    expect(tokenValue(themeBlock("caral"), "--accent")).toBe("#000000");
    expect(tokenValue(themeBlock("liangzhu"), "--accent")).toBe("#2f7f79");
  });

  test("uses white accent text on the caral black controls", () => {
    expect(tokenValue(themeBlock("caral"), "--accent-ink")).toBe("#ffffff");
  });
});
