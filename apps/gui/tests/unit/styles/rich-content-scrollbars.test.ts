import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "bun:test";

const richContentCss = readFileSync(
  resolve(import.meta.dir, "../../../app/src/styles/parts/layout/rich-content.css"),
  "utf8",
);

function cssBlock(selector: string): string {
  const start = richContentCss.indexOf(selector);
  expect(start).toBeGreaterThanOrEqual(0);
  const open = richContentCss.indexOf("{", start);
  const close = richContentCss.indexOf("}", open);
  expect(open).toBeGreaterThan(start);
  expect(close).toBeGreaterThan(open);
  return richContentCss.slice(open + 1, close);
}

describe("rich content table scrollbars", () => {
  test("keeps tables scrollable with custom overflow controls", () => {
    expect(cssBlock(".rich-table-scroll")).toContain("max-height: min(36vh, 320px);");
    expect(cssBlock(".rich-table-scroll")).toContain("overflow: auto;");
    expect(cssBlock(".rich-table-scroll")).toContain("scrollbar-width: none;");
    expect(cssBlock(".rich-table-scroll")).not.toContain("scrollbar-color");
    expect(cssBlock(".rich-table-scroll::-webkit-scrollbar")).toContain("display: none;");
    expect(richContentCss).toContain(".rich-table-overflow-x");
    expect(richContentCss).toContain(".rich-table-overflow-y");
  });

  test("keeps table rows separated and cells capped to four lines", () => {
    expect(cssBlock(".rich-table-scroll table")).toContain("min-width: 100%;");
    expect(cssBlock(".rich-table-scroll table")).toContain("width: max-content;");
    expect(cssBlock(".rich-table-scroll table")).toContain("border-collapse: separate;");
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "text-align: left;",
    );
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "white-space: normal;",
    );
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "overflow: hidden;",
    );
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "text-overflow: clip;",
    );
    expect(cssBlock(".rich-table-cell-content")).toContain("max-height: calc(1.35em * 4);");
    expect(cssBlock(".rich-table-cell-content")).toContain("overflow: hidden;");
    expect(cssBlock(".rich-table-scroll tbody tr:not(:last-child) > th,\n.rich-table-scroll tbody tr:not(:last-child) > td")).toContain("var(--line) 46%");
  });

  test("keeps sticky table affordances stable", () => {
    expect(cssBlock(".rich-table-scroll th:first-child,\n.rich-table-scroll td:first-child"))
      .toContain("position: sticky;");
    expect(richContentCss.replaceAll("\r\n", "\n")).toContain(
      ".rich-table-scroll th {\n  position: sticky;",
    );
  });
});
