import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "bun:test";

const appSource = readFileSync(resolve(import.meta.dir, "../../../app/src/app.tsx"), "utf8");
const conversationSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/conversation/conversation-view.tsx"),
  "utf8",
);
const conversationOutletSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/app/conversation-page-outlet.tsx"),
  "utf8",
);
const composerLayoutCss = readFileSync(
  resolve(import.meta.dir, "../../../app/src/styles/parts/layout/composer-controls.css"),
  "utf8",
);

describe("session render cache", () => {
  test("openSession reuses cached messages before calling the gateway", () => {
    const guardIndex = appSource.indexOf("shouldFetchSessionMessages(");
    const fetchIndex = appSource.indexOf("client.messages(sessionId");

    expect(guardIndex).toBeGreaterThanOrEqual(0);
    expect(fetchIndex).toBeGreaterThan(guardIndex);
    expect(appSource).not.toContain("e2eFixture && existingMessages.length > 0");
  });

  test("uncached session switches show the existing text-free loading animation", () => {
    expect(appSource).toContain("const [loadingSessionId, setLoadingSessionId]");
    expect(appSource).toContain("setLoadingSessionId(sessionId)");
    expect(appSource).toContain("const requestId = ++sessionMessageLoadRequest");
    expect(appSource).toContain("setLoadingSessionId((current) =>");
    expect(appSource).toContain("requestId === sessionMessageLoadRequest");
    expect(appSource).toContain("current === sessionId ? undefined : current");
    expect(conversationOutletSource).toContain("selectedSessionMessagesLoading");
    expect(conversationOutletSource).toContain("<ConversationLoadingPlaceholder />");
    expect(conversationOutletSource).toContain("when={!props.selectedSessionMessagesLoading()}");
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

  test("transcript exposes explicit earlier-history loading instead of auto-loading on top scroll", () => {
    expect(appSource).toContain("const MESSAGE_PAGE_SIZE = 100");
    expect(appSource).toContain("const MESSAGE_PAGE_FETCH_LIMIT = MESSAGE_PAGE_SIZE + 1");
    expect(conversationSource).toContain("transcript-history-button");
    expect(conversationSource).toContain("showEarlierRecords");
    expect(conversationSource).toContain("requestEarlierMessages");
    expect(conversationSource).not.toContain("maybeLoadEarlierMessages");
    expect(conversationSource).not.toContain("LOAD_EARLIER_SCROLL_TOP");
  });

  test("agent avatar anchor is limited to the latest assistant message", () => {
    expect(conversationSource).toContain("latestAssistantId={latestAssistantId()}");
    expect(conversationSource).toContain("showAvatarSpace={");
    expect(conversationSource).toContain(
      'avatarMode() !== "hidden" && entry.item().message.role !== "user"',
    );
    expect(conversationSource).toContain(
      "isLatestAssistant={props.latestAssistantId === props.entry.item().message.id}",
    );
    expect(conversationSource).toContain(
      'data-agent-avatar-anchor={props.isLatestAssistant ? "" : undefined}',
    );
  });
});
