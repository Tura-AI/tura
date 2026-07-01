import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const appSource = readFileSync(resolve(import.meta.dir, "../../../app/src/app.tsx"), "utf8");

function functionBlock(source: string, name: string): string {
  const start = source.indexOf(`async function ${name}`);
  expect(start).toBeGreaterThanOrEqual(0);
  const next = source.indexOf("\n  async function", start + 1);
  return source.slice(start, next > start ? next : undefined);
}

describe("GUI composer submission contract", () => {
  test("submits ordinary composer prompts directly to runtime", () => {
    const submitPrompt = functionBlock(appSource, "submitPrompt");

    expect(submitPrompt).toContain("await submitDirectPrompt(content)");
    expect(submitPrompt).not.toContain('submitQueuedPrompt(content, "session_idle")');
  });

  test("keeps idle queue submission behind the explicit queue path", () => {
    const queuePrompt = functionBlock(appSource, "queuePrompt");

    expect(queuePrompt).toContain('await submitQueuedPrompt(content, "session_idle")');
  });
});
