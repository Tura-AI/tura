import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "bun:test";

const titlebarSource = readFileSync(
  resolve(import.meta.dir, "../../app/src/app/shell-chrome.tsx"),
  "utf8",
);
const titlebarCss = readFileSync(
  resolve(import.meta.dir, "../../app/src/styles/parts/base/titlebar.css"),
  "utf8",
);
const pageShellCss = readFileSync(
  resolve(import.meta.dir, "../../app/src/styles/parts/base/page-shell.css"),
  "utf8",
);
const desktopTabletCss = readFileSync(
  resolve(import.meta.dir, "../../app/src/styles/parts/responsive/desktop-tablet.css"),
  "utf8",
);
const tauriCapability = JSON.parse(
  readFileSync(
    resolve(import.meta.dir, "../../../tauri/src-tauri/capabilities/default.json"),
    "utf8",
  ),
) as { permissions: string[] };

describe("custom Tauri titlebar", () => {
  test("uses the real app icon instead of a CSS placeholder mark", () => {
    expect(titlebarSource).toContain('src="/assets/brand/tura-icon.svg"');
    expect(titlebarSource).toContain('class="app-titlebar-mark"');
    expect(titlebarCss).not.toContain("border-radius: 50%");
  });

  test("grants the custom titlebar the Tauri window commands it invokes", () => {
    expect(titlebarSource).toContain("getCurrentWindow().minimize()");
    expect(titlebarSource).toContain("getCurrentWindow().toggleMaximize()");
    expect(titlebarSource).toContain("getCurrentWindow().close()");
    expect(titlebarSource).toContain("data-tauri-drag-region");
    expect(tauriCapability.permissions).toEqual(
      expect.arrayContaining([
        "core:window:allow-close",
        "core:window:allow-minimize",
        "core:window:allow-start-dragging",
        "core:window:allow-toggle-maximize",
      ]),
    );
  });

  test("anchors fixed rail chrome below the Tauri titlebar", () => {
    expect(pageShellCss).toContain(
      "top: calc(var(--app-titlebar-height) + var(--rail-open-button-top));",
    );
    expect(desktopTabletCss).toContain("inset: var(--app-titlebar-height) 0 0 0;");
    expect(desktopTabletCss).toContain(
      "height: calc(100dvh - var(--app-titlebar-height));",
    );
  });
});
