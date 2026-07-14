import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "bun:test";

const loadingPlaceholderSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/app/loading-placeholders.tsx"),
  "utf8",
);
const loadingCss = readFileSync(
  resolve(import.meta.dir, "../../../app/src/styles/parts/base/loading.css"),
  "utf8",
);
const appShellSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/app/app-shell.tsx"),
  "utf8",
);

function cssBlock(selector: string): string {
  const start = loadingCss.indexOf(selector);
  expect(start).toBeGreaterThanOrEqual(0);
  const open = loadingCss.indexOf("{", start);
  const close = loadingCss.indexOf("}", open);
  expect(open).toBeGreaterThan(start);
  expect(close).toBeGreaterThan(open);
  return loadingCss.slice(open + 1, close);
}

describe("text loading placeholder layout", () => {
  test("renders one assistant text block instead of multi-line loose bars", () => {
    const componentStart = loadingPlaceholderSource.indexOf(
      "export function TranscriptTextLoadingLines",
    );
    expect(componentStart).toBeGreaterThanOrEqual(0);
    const component = loadingPlaceholderSource.slice(componentStart);

    expect(component).toContain('class="message assistant transcript-loading-placeholder"');
    expect(component).toContain('class="assistant-response"');
    expect(component).toContain('class="assistant-text-block"');
    expect(component).toContain('class="rich-text"');
    expect(component.match(/text-loading-line/gu)).toHaveLength(1);
    expect(component).not.toContain("<For");
  });

  test("uses the final rich text line height and no standalone transcript padding", () => {
    expect(cssBlock(".transcript-loading-placeholder")).not.toContain("padding");
    expect(cssBlock(".transcript-loading-placeholder .text-loading-line")).toContain(
      "height: calc(var(--font-ui) * 1.92);",
    );
    expect(cssBlock(".transcript-loading-placeholder .text-loading-line")).toContain(
      "background: transparent;",
    );
  });

  test("does not render the main-column placeholder underneath the gateway overlay", () => {
    expect(appShellSource).toContain("function showGatewayLoadingOverlay()");
    expect(appShellSource).toContain("<Show when={!showGatewayLoadingOverlay()}");
    expect(appShellSource).toContain("<Show when={showGatewayLoadingOverlay()}");
  });
});
