import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "bun:test";

const tokens = readFileSync(resolve(import.meta.dir, "../../../app/src/styles/tokens.css"), "utf8");
const defaults = readFileSync(
  resolve(import.meta.dir, "../../../app/src/config/defaults.ts"),
  "utf8",
);
const indexHtml = readFileSync(resolve(import.meta.dir, "../../../app/index.html"), "utf8");
const fontInstaller = readFileSync(
  resolve(import.meta.dir, "../../../scripts/install-fonts.mjs"),
  "utf8",
);

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
  const line = block
    .split(/\r?\n/u)
    .map((item) => item.trim())
    .find((item) => item.startsWith(`${token}:`));
  const value = line
    ?.slice(token.length + 1)
    .trim()
    .replace(/;$/u, "");
  expect(value).toBeDefined();
  return value!;
}

function colorTokenValue(block: string, token: string): string {
  const value = tokenValue(block, token);
  expect(value).toMatch(/^#[0-9a-fA-F]{6}$/u);
  return value;
}

describe("theme accent tokens", () => {
  test("uses low-saturation theme accents instead of vivid colors", () => {
    expect(colorTokenValue(themeBlock(), "--accent")).toBe("#3f4652");
    expect(colorTokenValue(themeBlock("dark"), "--accent")).toBe("#d8d4ca");
    expect(colorTokenValue(themeBlock("uruk"), "--accent")).toBe("#6d5148");
    expect(tokenValue(themeBlock("caral"), "--accent")).toBe("var(--ink)");
    expect(colorTokenValue(themeBlock("liangzhu"), "--accent")).toBe("#2f7f79");
  });

  test("uses white accent text on the caral black controls", () => {
    expect(colorTokenValue(themeBlock("caral"), "--accent-ink")).toBe("#ffffff");
  });
});

describe("corner radius tokens", () => {
  test("derives all nonzero radii from one global scale", () => {
    const root = themeBlock();

    expect(root).toContain("--corner-radius-scale: 1;");
    expect(root).toContain("--radius: calc(8px * var(--corner-radius-scale));");
    expect(root).toContain("--radius-small: calc(6px * var(--corner-radius-scale));");
    expect(root).toContain("--radius-large: calc(14px * var(--corner-radius-scale));");
    expect(root).toContain("--radius-xl: calc(18px * var(--corner-radius-scale));");
    expect(root).toContain("--radius-pill: calc(16px * var(--corner-radius-scale));");
  });

  describe("typography tokens", () => {
    test("defaults to the Archivo and IBM Plex multilingual font system", () => {
      expect(defaults).toContain('"Archivo"');
      expect(defaults).toContain('"IBM Plex Sans SC"');
      expect(defaults).toContain('"IBM Plex Mono"');
      expect(tokens).toContain('"Archivo"');
      expect(tokens).toContain('"IBM Plex Sans SC"');
      expect(tokens).toContain('"IBM Plex Mono"');
    });

    test("loads GUI fonts from locally installed Google Fonts assets", () => {
      expect(indexHtml).toContain("/assets/fonts/google/fonts.css");
      expect(fontInstaller).toContain('family: "Archivo"');
      expect(fontInstaller).toContain('family: "IBM Plex Sans"');
      expect(fontInstaller).toContain('family: "IBM Plex Mono"');
      expect(fontInstaller).toContain('family: "LXGW Marker Gothic"');
      expect(fontInstaller).toContain("fonts.googleapis.com/css2");
    });
  });
});
