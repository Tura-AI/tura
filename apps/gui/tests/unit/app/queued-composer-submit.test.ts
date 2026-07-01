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

describe("GUI composer queue submission contract", () => {
  test("submits composer prompts as idle queued tasks instead of runtime prompts", () => {
    const submitPrompt = functionBlock(appSource, "submitPrompt");

    expect(submitPrompt).toContain('await submitQueuedPrompt(content, "session_idle")');
    expect(submitPrompt).not.toContain("directoryClient().promptAsync");
    expect(submitPrompt).not.toContain("pollSessionMessagesUntilAssistantReply");
    expect(submitPrompt).not.toContain('status: "busy"');
    expect(submitPrompt).not.toContain("userNewCommand");
    expect(submitPrompt).not.toContain("planRunPending");
    expect(submitPrompt).not.toContain("optimisticSessionId");
    expect(submitPrompt).not.toContain("optimisticId");
  });
});
