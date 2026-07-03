import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "bun:test";

const source = readFileSync(
  resolve(import.meta.dir, "../../../app/src/conversation/tool-inspector.tsx"),
  "utf8",
);

describe("tool inspector command footer", () => {
  test("shows command timing directly before exit code without duplicated status text", () => {
    const footer = source.match(/<footer class="inspector-status">([\s\S]*?)<\/footer>/)?.[1] ?? "";

    expect(footer).toContain("inspector-command-timing");
    expect(footer).toContain("formatCommandTiming(record().durationMs, record().timeoutMs)");
    expect(footer).toContain("inspector-exit-code");
    expect(footer.indexOf("inspector-command-timing")).toBeLessThan(
      footer.indexOf("inspector-exit-code"),
    );
    expect(footer).not.toContain("toolStatusLabel(record().status)");
    expect(footer).not.toContain("serviceStatusLabel(props.serviceStatus)");
  });
});
