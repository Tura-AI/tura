import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "bun:test";

const appSource = readFileSync(resolve(import.meta.dir, "../../../app/src/app.tsx"), "utf8");
const conversationSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/conversation/conversation-view.tsx"),
  "utf8",
);
const composerLayoutCss = readFileSync(
  resolve(import.meta.dir, "../../../app/src/styles/parts/layout/composer-controls.css"),
  "utf8",
);

describe("session render cache", () => {
  test("openSession reuses cached messages before calling the gateway", () => {
    const guardIndex = appSource.indexOf("shouldFetchSessionMessages(existingMessages");
    const fetchIndex = appSource.indexOf("client.messages(sessionId");

    expect(guardIndex).toBeGreaterThanOrEqual(0);
    expect(fetchIndex).toBeGreaterThan(guardIndex);
    expect(appSource).not.toContain("e2eFixture && existingMessages.length > 0");
  });

  test("transcript renders a bounded virtual window instead of keeping visited rows mounted", () => {
    expect(conversationSource).toContain("MAX_TRANSCRIPT_RENDERED_MESSAGES = 100");
    expect(conversationSource).toContain("boundedVirtualWindow(visibleEntries");
    expect(conversationSource).toContain("pruneVirtualEntryCache");
    expect(conversationSource).not.toContain("transcriptMountedRowsBySession");
    expect(conversationSource).not.toContain("cachedMountedMessageIdsForSession");
    expect(conversationSource).not.toContain("syncMountedTranscriptRows");

    const virtualItemsStart = conversationSource.indexOf("const virtualItems = createMemo");
    const virtualItemsEnd = conversationSource.indexOf("const showTranscriptLoadingTransition");
    const virtualItemsBlock = conversationSource.slice(virtualItemsStart, virtualItemsEnd);
    expect(virtualItemsBlock).toContain(".filter((entry) => entry.top + entry.height >= start");
    expect(virtualItemsBlock).not.toContain("mountedMessageIds");
  });

  test("transcript hides preparing rows behind the existing text loading transition", () => {
    expect(conversationSource).toContain("showTranscriptLoadingTransition");
    expect(conversationSource).toContain("<TranscriptTextLoadingLines />");
    expect(conversationSource).toContain("transcript-render-preparing");
    expect(composerLayoutCss).toContain(".transcript-virtual-space.transcript-render-preparing");
    expect(composerLayoutCss).toContain("visibility: hidden;");
  });
});
