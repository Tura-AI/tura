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
  test("renders every table row while keeping only horizontal table scrolling", () => {
    expect(cssBlock(".rich-table-scroll")).not.toContain("max-height");
    expect(cssBlock(".rich-table-scroll")).toContain("overflow-x: auto;");
    expect(cssBlock(".rich-table-scroll")).toContain("overflow-y: visible;");
    expect(cssBlock(".rich-table-scroll")).toContain("scrollbar-width: none;");
    expect(cssBlock(".rich-table-scroll")).not.toContain("scrollbar-color");
    expect(cssBlock(".rich-table-scroll::-webkit-scrollbar")).toContain("display: none;");
    expect(richContentCss).toContain(".rich-table-overflow-x");
    expect(richContentCss).not.toContain(".rich-table-overflow-y");
  });

  test("keeps table rows separated while showing complete left-aligned cell content", () => {
    expect(cssBlock(".rich-table-scroll table")).toContain("min-width: 100%;");
    expect(cssBlock(".rich-table-scroll table")).toContain("width: max-content;");
    expect(cssBlock(".rich-table-scroll table")).toContain("border-collapse: separate;");
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "text-align: left;",
    );
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).not.toContain(
      "text-align: right;",
    );
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "padding: var(--space-3) 10px;",
    );
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "min-width: calc(14.5rem / 3);",
    );
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "white-space: normal;",
    );
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).toContain(
      "overflow: visible;",
    );
    expect(cssBlock(".rich-table-scroll th,\n.rich-table-scroll td")).not.toContain(
      "text-overflow: clip;",
    );
    expect(cssBlock(".rich-table-cell-content")).not.toContain("max-height");
    expect(cssBlock(".rich-table-cell-content")).toContain("overflow: visible;");
    expect(cssBlock(".rich-table-cell-content")).toContain("overflow-wrap: anywhere;");
    expect(cssBlock(".rich-table-scroll tbody tr:first-child > th")).toContain(
      "var(--line-strong)",
    );
    expect(richContentCss.indexOf(".rich-table-scroll tbody tr:first-child > th")).toBeGreaterThan(
      richContentCss.indexOf(
        ".rich-table-scroll tbody tr:not(:last-child) > th,\n.rich-table-scroll tbody tr:not(:last-child) > td",
      ),
    );
    expect(
      cssBlock(
        ".rich-table-scroll tbody tr:not(:last-child) > th,\n.rich-table-scroll tbody tr:not(:last-child) > td",
      ),
    ).toContain("var(--line) 46%");
  });

  test("lets the first column scroll horizontally with the rest of the table", () => {
    expect(
      cssBlock(".rich-table-scroll th:first-child,\n.rich-table-scroll td:first-child"),
    ).toContain("min-width: calc(5.5rem / 3);");
    expect(
      cssBlock(".rich-table-scroll th:first-child,\n.rich-table-scroll td:first-child"),
    ).not.toContain("position: sticky;");
    expect(
      cssBlock(".rich-table-scroll th:first-child,\n.rich-table-scroll td:first-child"),
    ).not.toContain("left: 0;");
    expect(richContentCss.replaceAll("\r\n", "\n")).toContain(
      ".rich-table-scroll th {\n  position: sticky;",
    );
  });
});
