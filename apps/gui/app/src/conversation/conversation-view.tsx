import type {
  AgentAvatarConfig,
  Command,
  Message,
  MessagePart,
  PersonaMediaConfig,
  Session,
} from "@tura/gateway-sdk";
import ArrowDown from "lucide-solid/icons/arrow-down";
import {
  For,
  type Accessor,
  Index,
  type JSX,
  Show,
  type Setter,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import { TranscriptTextLoadingLines } from "../app/loading-placeholders";
import {
  AgentAvatarCanvas,
  agentAvatarMedia,
  type AvatarDisplayMode,
} from "../components/avatar/agent-avatar-canvas";
import { t } from "../i18n";
import { classNames, formatTime } from "../state/format";
import {
  type AppState,
  type ComposerImage,
  messageCreatedAt,
  partText,
  sessionTitle,
} from "../state/global-store";
import {
  avatarConfigForAgent,
  conversationReactionItems,
  type ConversationReactionItem,
  latestSticker,
  messagesWithSessionThinking,
  personaMediaForAvatar,
} from "./conversation-data";
import { groupConversationTurns } from "./conversation-turns";
import { assistantPartBlocks, assistantToolBlockForPart } from "./assistant-blocks";
import { Composer } from "./composer";
import { TextPartCell, previewUserTextParts } from "./message-text-parts";
import { RunSummary, blockDurationMs } from "./run-summary";
import { sessionShowsBusyAnimation } from "./session-animation";
import { ToolInspector } from "./tool-inspector";
import {
  asRecord,
  formatDuration,
  isToolPart,
  messageDurationMs,
  toolStatus,
} from "./message-tools";

const INSPECTOR_MIN_WIDTH = 320;
const INSPECTOR_MAX_WIDTH = 680;
const CONVERSATION_MAIN_MIN_WIDTH = 430;
const AGENT_AVATAR_SIZE = 56;
const AGENT_AVATAR_GAP = 8;
const AGENT_AVATAR_BOTTOM_SNAP = 48;
const AGENT_AVATAR_BOTTOM_SETTLE_MS = 0;
const VIRTUAL_MESSAGE_ESTIMATED_HEIGHT = 64;
const VIRTUAL_MESSAGE_OVERSCAN = 300;
const LOAD_EARLIER_SCROLL_TOP = 480;

export function ConversationView(props: {
  state: AppState;
  session?: Session;
  messages: Message[];
  onLoadEarlierMessages?: () => Promise<boolean>;
  slashCommands: Command[];
  onComposerText: (text: string) => void;
  onComposerImages: (images: ComposerImage[]) => void;
  onSubmit: () => void;
  onStop?: () => void;
  onQueueSubmit?: () => void;
  compact?: boolean;
  composerToolbar?: JSX.Element;
  conversationNotice?: JSX.Element;
  submitDisabled?: boolean;
  running?: boolean;
  onToolOpen?: (part: MessagePart, parts: MessagePart[]) => void;
  compactInspector?: boolean;
  leftRailOpen?: boolean;
  leftRailWidth?: number;
  minMainWidth?: number;
  onRequestCollapseLeftRail?: () => void;
  onInspectorLayout?: (layout: { open: boolean; overlay: boolean; width: number }) => void;
  closeInspectorSignal?: number;
}) {
  const [selectedToolId, setSelectedToolId] = createSignal<string>();
  const [inspectorParts, setInspectorParts] = createSignal<MessagePart[]>([]);
  const [inspectorOpen, setInspectorOpen] = createSignal(false);
  const [inspectorOverlay, setInspectorOverlay] = createSignal(false);
  const [inspectorWidth, setInspectorWidth] = createSignal(430);
  const [transcriptPinned, setTranscriptPinned] = createSignal(true);
  const [viewportWidth, setViewportWidth] = createSignal(
    typeof window === "undefined" ? 0 : window.innerWidth,
  );
  const groupedMessages = createMemo(() => groupConversationTurns(props.messages));
  const selectedAgentAvatar = createMemo(() =>
    avatarConfigForAgent(
      props.state.agents,
      props.state.selectedAgent,
      props.state.workspaceConfig,
    ),
  );
  const selectedAgentAvatarMedia = createMemo(() =>
    agentAvatarMedia(
      personaMediaForAvatar(props.state.personas, selectedAgentAvatar()),
      selectedAgentAvatar().role,
    ),
  );
  const latestStickerEmoji = createMemo(() => latestSticker(props.messages));
  const latestMessageId = createMemo(() => groupedMessages().at(-1)?.id);
  let transcriptEl: HTMLElement | undefined;
  let conversationMainEl: HTMLDivElement | undefined;
  let scrollFollowFrame: number | undefined;
  let scrollFollowObserver: ResizeObserver | undefined;
  let inspectorSessionId = props.session?.id;
  const [scrollFollowBottom, setScrollFollowBottom] = createSignal(166);
  const minMainWidth = createMemo(() => props.minMainWidth ?? CONVERSATION_MAIN_MIN_WIDTH);
  const leftRailOpen = createMemo(() => props.leftRailOpen ?? false);
  const configuredLeftRailWidth = createMemo(() => props.leftRailWidth ?? 0);

  function leftRailWidth() {
    return leftRailOpen() ? configuredLeftRailWidth() : 0;
  }

  function mainWidthWith(leftWidth: number, rightWidth: number) {
    return viewportWidth() - leftWidth - rightWidth;
  }

  function canFitInspector(width: number, leftWidth = leftRailWidth()) {
    return mainWidthWith(leftWidth, width) >= minMainWidth();
  }

  function collapseLeftIfInspectorNeedsRoom(width = inspectorWidth()) {
    if (leftRailOpen() && !canFitInspector(width)) {
      props.onRequestCollapseLeftRail?.();
      return true;
    }
    return false;
  }

  function inspectorMaxWidth(leftAlreadyCollapsed = false) {
    const left = leftAlreadyCollapsed ? 0 : leftRailWidth();
    return Math.min(INSPECTOR_MAX_WIDTH, Math.max(0, viewportWidth() - left - minMainWidth()));
  }

  function requestInspectorWidth(width: number) {
    const collapsedLeft = collapseLeftIfInspectorNeedsRoom(width);
    const max = inspectorMaxWidth(collapsedLeft);
    if (max < INSPECTOR_MIN_WIDTH) {
      setInspectorOverlay(true);
      setInspectorWidth(INSPECTOR_MIN_WIDTH);
      return true;
    }
    setInspectorOverlay(false);
    setInspectorWidth(Math.min(max, Math.max(INSPECTOR_MIN_WIDTH, width)));
    return true;
  }

  function openInspectorFor(part: MessagePart, parts: MessagePart[]) {
    if (inspectorOpen() && selectedToolId() === part.id) {
      setInspectorOpen(false);
      setInspectorOverlay(false);
      return;
    }
    const needsLeftCollapsed = collapseLeftIfInspectorNeedsRoom(INSPECTOR_MIN_WIDTH);
    setSelectedToolId(part.id);
    setInspectorParts(parts);
    const max = inspectorMaxWidth(needsLeftCollapsed);
    if (max < INSPECTOR_MIN_WIDTH) {
      setInspectorOverlay(true);
      setInspectorWidth(INSPECTOR_MIN_WIDTH);
      setInspectorOpen(true);
      return;
    }
    setInspectorOverlay(false);
    if (requestInspectorWidth(inspectorWidth())) {
      setInspectorOpen(true);
    }
  }

  onMount(() => {
    const resize = () => setViewportWidth(window.innerWidth);
    window.addEventListener("resize", resize);
    onCleanup(() => window.removeEventListener("resize", resize));
  });

  createEffect(() => {
    const sessionId = props.session?.id;
    if (sessionId === inspectorSessionId) {
      return;
    }
    inspectorSessionId = sessionId;
    setInspectorOpen(false);
    setInspectorOverlay(false);
    setSelectedToolId(undefined);
    setInspectorParts([]);
  });

  createEffect(() => {
    props.closeInspectorSignal;
    setInspectorOpen(false);
    setInspectorOverlay(false);
    setSelectedToolId(undefined);
    setInspectorParts([]);
  });

  createEffect(() => {
    if (!inspectorOpen() || inspectorOverlay() || canFitInspector(inspectorWidth())) {
      return;
    }
    if (leftRailOpen() && canFitInspector(INSPECTOR_MIN_WIDTH, 0)) {
      props.onRequestCollapseLeftRail?.();
      return;
    }
    if (!leftRailOpen() || !canFitInspector(INSPECTOR_MIN_WIDTH, 0)) {
      setInspectorOpen(false);
    }
  });

  createEffect(() => {
    props.onInspectorLayout?.({
      open: inspectorOpen() && !inspectorOverlay(),
      overlay: inspectorOpen() && inspectorOverlay(),
      width: inspectorOpen() && !inspectorOverlay() ? inspectorWidth() : 0,
    });
  });

  function transcriptAtBottom() {
    if (!transcriptEl) {
      return true;
    }
    return transcriptEl.scrollHeight - transcriptEl.scrollTop - transcriptEl.clientHeight < 28;
  }

  function scrollTranscriptToBottom(behavior: ScrollBehavior = "smooth") {
    if (!transcriptEl) {
      return;
    }
    setTranscriptPinned(true);
    const scroll = () => {
      transcriptEl?.scrollTo({
        top: transcriptEl?.scrollHeight ?? 0,
        behavior,
      });
    };
    scroll();
    requestAnimationFrame(scroll);
  }

  function handleTranscriptScroll() {
    setTranscriptPinned(transcriptAtBottom());
  }

  function updateScrollFollowBottom() {
    if (!conversationMainEl || !transcriptEl) {
      return;
    }
    const mainRect = conversationMainEl.getBoundingClientRect();
    const transcriptRect = transcriptEl.getBoundingClientRect();
    setScrollFollowBottom(Math.max(14, Math.round(mainRect.bottom - transcriptRect.bottom + 10)));
  }

  function queueScrollFollowBottomUpdate() {
    if (scrollFollowFrame) {
      cancelAnimationFrame(scrollFollowFrame);
    }
    scrollFollowFrame = requestAnimationFrame(() => {
      scrollFollowFrame = undefined;
      updateScrollFollowBottom();
    });
  }

  onMount(() => {
    scrollFollowObserver = new ResizeObserver(queueScrollFollowBottomUpdate);
    if (conversationMainEl) {
      scrollFollowObserver.observe(conversationMainEl);
    }
    if (transcriptEl) {
      scrollFollowObserver.observe(transcriptEl);
    }
    window.addEventListener("resize", queueScrollFollowBottomUpdate);
    queueScrollFollowBottomUpdate();
    onCleanup(() => {
      window.removeEventListener("resize", queueScrollFollowBottomUpdate);
      scrollFollowObserver?.disconnect();
      if (scrollFollowFrame) {
        cancelAnimationFrame(scrollFollowFrame);
      }
    });
  });

  let lastAutoScrolledMessageId: string | undefined;
  createEffect(() => {
    const messageId = latestMessageId();
    if (!messageId || lastAutoScrolledMessageId === messageId) {
      return;
    }
    lastAutoScrolledMessageId = messageId;
    if (transcriptPinned()) {
      scrollTranscriptToBottom("auto");
    }
  });

  createEffect(() => {
    if (!inspectorOpen()) {
      return;
    }
    const selectedId = selectedToolId();
    if (!selectedId) {
      return;
    }
    const currentMessage = groupedMessages().find((message) =>
      message.parts.some((part) => part.id === selectedId),
    );
    if (currentMessage) {
      setInspectorParts(assistantToolBlockForPart(currentMessage.parts, selectedId)?.parts ?? []);
    }
  });

  return (
    <section
      class={classNames(
        "conversation-view",
        "layered-page",
        "layered-page-three",
        props.compact && "compact-conversation",
        inspectorOpen() && !inspectorOverlay() && "inspector-open",
      )}
      style={{
        "--inspector-width": `${inspectorWidth()}px`,
        "--inspector-max-width": `${inspectorMaxWidth()}px`,
      }}
    >
      <header class="page-head page-layer-inner">
        <div class="page-title">
          <span>{t("conversation")}</span>
          <h1>{props.session ? sessionTitle(props.session) : t("newSession")}</h1>
        </div>
      </header>
      <div class="conversation-grid page-layer-middle">
        <div
          ref={conversationMainEl}
          class="conversation-main"
          style={{
            "--scroll-follow-bottom": `${scrollFollowBottom()}px`,
          }}
        >
          <Transcript
            session={props.session}
            messages={groupedMessages()}
            onLoadEarlierMessages={props.onLoadEarlierMessages}
            loading={props.state.loading}
            activeToolId={selectedToolId()}
            conversationNotice={props.conversationNotice}
            avatarMedia={selectedAgentAvatarMedia()}
            avatarSettings={selectedAgentAvatar()}
            expressionEmoji={latestStickerEmoji()}
            workspaceDirectory={props.state.directory}
            onTranscript={(element) => {
              transcriptEl = element;
              scrollFollowObserver?.observe(element);
              queueScrollFollowBottomUpdate();
            }}
            onScroll={handleTranscriptScroll}
            onTool={(part, parts) => {
              if (props.compact && props.onToolOpen && !props.compactInspector) {
                props.onToolOpen(part, parts);
                return;
              }
              openInspectorFor(part, parts);
            }}
          />
        </div>
        <Show when={!transcriptPinned()}>
          <button
            class="scroll-follow"
            type="button"
            title={t("scrollToBottom")}
            onClick={() => scrollTranscriptToBottom("auto")}
          >
            <ArrowDown size={18} strokeWidth={1.7} />
          </button>
        </Show>
      </div>
      <div class="conversation-bottom page-layer-bottom">
        <Composer
          text={props.state.composerText}
          images={props.state.composerImages}
          submitting={props.state.submitting}
          slashCommands={props.slashCommands}
          onText={props.onComposerText}
          onImages={props.onComposerImages}
          onSubmit={props.onSubmit}
          onStop={props.onStop}
          onQueueSubmit={props.onQueueSubmit}
          toolbar={props.composerToolbar}
          submitDisabled={props.submitDisabled}
          running={props.running}
        />
      </div>
      <Show when={!props.compact || props.compactInspector}>
        <ToolInspector
          parts={inspectorParts()}
          serviceStatus={props.state.serviceStatus}
          selectedId={selectedToolId()}
          open={inspectorOpen()}
          overlay={inspectorOverlay()}
          width={inspectorWidth()}
          maxWidth={inspectorOverlay() ? viewportWidth() : inspectorMaxWidth()}
          leftRailOpen={props.leftRailOpen}
          leftRailWidth={props.leftRailWidth}
          minMainWidth={minMainWidth()}
          onRequestCollapseLeftRail={props.onRequestCollapseLeftRail}
          onWidth={requestInspectorWidth}
          onSelect={setSelectedToolId}
          onClose={() => {
            setInspectorOpen(false);
            setInspectorOverlay(false);
          }}
        />
      </Show>
    </section>
  );
}

function Transcript(props: {
  session?: Session;
  messages: Message[];
  onLoadEarlierMessages?: () => Promise<boolean>;
  loading: boolean;
  activeToolId?: string;
  conversationNotice?: JSX.Element;
  avatarMedia: PersonaMediaConfig;
  avatarSettings: AgentAvatarConfig;
  expressionEmoji?: string;
  workspaceDirectory?: string;
  onTranscript: (element: HTMLElement) => void;
  onScroll: () => void;
  onTool: (part: MessagePart, parts: MessagePart[]) => void;
}) {
  const displayMessages = createMemo(() =>
    conversationReactionItems(messagesWithSessionThinking(props.messages, props.session)),
  );
  const messageLayoutSignature = createMemo(() =>
    displayMessages()
      .map((item) =>
        [
          item.message.id,
          item.message.role,
          item.message.parts
            .map((part) => `${part.id}:${part.type}:${toolStatus(asRecord(part.state))}`)
            .join(","),
        ].join(":"),
      )
      .join("|"),
  );
  const latestId = createMemo(() => displayMessages().at(-1)?.message.id);
  const latestAssistantId = createMemo(() => {
    const items = displayMessages();
    for (let index = items.length - 1; index >= 0; index -= 1) {
      const message = items[index]?.message;
      if (message?.role === "assistant") {
        return message.id;
      }
    }
    return undefined;
  });
  const [scrollTop, setScrollTop] = createSignal(0);
  const [clientHeight, setClientHeight] = createSignal(0);
  const [heightVersion, setHeightVersion] = createSignal(0);
  const [floatingAvatar, setFloatingAvatar] = createSignal<
    { left: number; top: number } | undefined
  >();
  let transcriptEl: HTMLElement | undefined;
  let transcriptInnerEl: HTMLDivElement | undefined;
  let avatarFrame: number | undefined;
  let loadEarlierPromise: Promise<boolean> | undefined;
  let bottomSettleTimer: number | undefined;
  let avatarResizeObserver: ResizeObserver | undefined;
  let measuredHeightFrame: number | undefined;
  const pendingMeasuredHeights = new Map<string, { height: number; top: number }>();
  const measuredHeights = new Map<string, number>();
  const virtualEntryCache = new Map<string, VirtualMessageEntry>();
  let lastScrollUpdateAt = 0;
  let lastScrolledAwayFromBottomAt = 0;
  let measuredSessionId = props.session?.id;

  const virtualLayout = createMemo(() => {
    heightVersion();
    const items = displayMessages();
    const offsets: number[] = [];
    let totalHeight = 0;
    for (const item of items) {
      offsets.push(totalHeight);
      totalHeight += measuredHeights.get(item.message.id) ?? VIRTUAL_MESSAGE_ESTIMATED_HEIGHT;
    }
    return { offsets, totalHeight };
  });

  const virtualItems = createMemo(() => {
    const items = displayMessages();
    const layout = virtualLayout();
    const start = Math.max(0, scrollTop() - VIRTUAL_MESSAGE_OVERSCAN);
    const end = scrollTop() + clientHeight() + VIRTUAL_MESSAGE_OVERSCAN;
    return items
      .map((item, index) => ({
        item,
        index,
        top: layout.offsets[index] ?? 0,
        height: measuredHeights.get(item.message.id) ?? VIRTUAL_MESSAGE_ESTIMATED_HEIGHT,
      }))
      .filter((entry) => entry.top + entry.height >= start && entry.top <= end)
      .map((entry) => virtualEntryFor(virtualEntryCache, entry.item, entry.index, entry.top));
  });

  function hideFloatingAvatar() {
    setFloatingAvatar(undefined);
  }

  function updateFloatingAvatar() {
    if (!transcriptEl) {
      hideFloatingAvatar();
      return;
    }
    if (props.avatarSettings.display_mode === "hidden") {
      hideFloatingAvatar();
      return;
    }
    const transcriptRect = transcriptEl.getBoundingClientRect();
    const viewportTop = transcriptEl.scrollTop;
    const viewportBottom = viewportTop + transcriptEl.clientHeight;
    if (transcriptEl.clientHeight < AGENT_AVATAR_SIZE) {
      hideFloatingAvatar();
      return;
    }

    const targetMessageId = latestAssistantId();
    const targetRow = targetMessageId
      ? Array.from(transcriptEl.querySelectorAll<HTMLElement>(".transcript-virtual-row")).find(
          (row) => row.dataset.messageId === targetMessageId,
        )
      : undefined;
    const targetAnchors = targetRow
      ? Array.from(targetRow.querySelectorAll<HTMLElement>("[data-agent-avatar-anchor]"))
      : [];
    const anchors =
      targetAnchors.length > 0
        ? targetAnchors
        : Array.from(transcriptEl.querySelectorAll<HTMLElement>("[data-agent-avatar-anchor]"));

    let selected:
      | {
          element: HTMLElement;
          top: number;
          bottom: number;
        }
      | undefined;
    for (const block of anchors) {
      const rect = block.getBoundingClientRect();
      const blockTop = rect.top - transcriptRect.top + viewportTop;
      const blockBottom = blockTop + rect.height;
      const visibleHeight = Math.min(blockBottom, viewportBottom) - Math.max(blockTop, viewportTop);
      if (visibleHeight <= 0) {
        continue;
      }
      if (!selected || blockBottom > selected.bottom) {
        selected = { element: block, top: blockTop, bottom: blockBottom };
      }
    }

    if (!selected) {
      hideFloatingAvatar();
      return;
    }

    const remainingScrollBottom = Math.max(0, transcriptEl.scrollHeight - viewportBottom);
    const bottomScrollSettling =
      remainingScrollBottom <= 1 &&
      performance.now() - lastScrollUpdateAt < AGENT_AVATAR_BOTTOM_SETTLE_MS;
    if (bottomScrollSettling && !bottomSettleTimer) {
      const delay = Math.max(
        0,
        AGENT_AVATAR_BOTTOM_SETTLE_MS - (performance.now() - lastScrollUpdateAt),
      );
      bottomSettleTimer = window.setTimeout(() => {
        bottomSettleTimer = undefined;
        queueFloatingAvatarUpdate();
      }, delay);
    }
    const selectedBottom =
      (remainingScrollBottom > 1 && remainingScrollBottom <= AGENT_AVATAR_BOTTOM_SNAP) ||
      bottomScrollSettling
        ? viewportBottom
        : Math.min(selected.bottom, viewportBottom);
    const topInTranscript = Math.min(
      Math.max(selectedBottom - AGENT_AVATAR_SIZE, Math.max(selected.top, viewportTop)),
      viewportBottom - AGENT_AVATAR_SIZE,
    );
    const selectedRect = selected.element.getBoundingClientRect();
    const top = transcriptRect.top + topInTranscript - viewportTop;
    const left = Math.max(
      transcriptRect.left + 4,
      selectedRect.left - AGENT_AVATAR_SIZE - AGENT_AVATAR_GAP,
    );
    setFloatingAvatar({
      left: Math.round(left),
      top: Math.round(top),
    });
  }

  function queueFloatingAvatarUpdate() {
    if (avatarFrame) {
      cancelAnimationFrame(avatarFrame);
    }
    avatarFrame = requestAnimationFrame(() => {
      avatarFrame = undefined;
      updateFloatingAvatar();
    });
  }

  function updateTranscriptViewport() {
    if (!transcriptEl) {
      return;
    }
    setScrollTop(transcriptEl.scrollTop);
    setClientHeight(transcriptEl.clientHeight);
  }

  function markManualScrollAwayFromBottom() {
    const element = transcriptEl;
    if (!element) {
      return;
    }
    if (element.scrollHeight - element.scrollTop - element.clientHeight >= 28) {
      lastScrolledAwayFromBottomAt = performance.now();
    }
  }

  function flushMeasuredHeights() {
    measuredHeightFrame = undefined;
    if (pendingMeasuredHeights.size === 0) {
      return;
    }
    const wasAtBottom = transcriptEl
      ? transcriptEl.scrollHeight - transcriptEl.scrollTop - transcriptEl.clientHeight < 28
      : false;
    const recentlyScrolledAway = performance.now() - lastScrolledAwayFromBottomAt < 500;
    let scrollDelta = 0;
    let changed = false;
    for (const [messageId, measurement] of pendingMeasuredHeights) {
      const next = Math.max(1, Math.round(measurement.height));
      const previous = measuredHeights.get(messageId);
      if (previous === next) {
        continue;
      }
      measuredHeights.set(messageId, next);
      changed = true;
      if (transcriptEl && measurement.top < transcriptEl.scrollTop) {
        scrollDelta += next - (previous ?? VIRTUAL_MESSAGE_ESTIMATED_HEIGHT);
      }
    }
    pendingMeasuredHeights.clear();
    if (!changed) {
      return;
    }
    setHeightVersion((version) => version + 1);
    if (!transcriptEl) {
      return;
    }
    if (wasAtBottom && !recentlyScrolledAway) {
      requestAnimationFrame(() => transcriptEl?.scrollTo({ top: transcriptEl.scrollHeight }));
      return;
    }
    if (scrollDelta !== 0) {
      transcriptEl.scrollTop += scrollDelta;
      updateTranscriptViewport();
    }
  }

  function updateMeasuredHeight(messageId: string, height: number, top: number) {
    pendingMeasuredHeights.set(messageId, { height, top });
    if (measuredHeightFrame) {
      return;
    }
    measuredHeightFrame = requestAnimationFrame(flushMeasuredHeights);
  }

  function maybeLoadEarlierMessages() {
    if (
      !transcriptEl ||
      !props.onLoadEarlierMessages ||
      transcriptEl.scrollTop > LOAD_EARLIER_SCROLL_TOP
    ) {
      return;
    }
    if (loadEarlierPromise) {
      return;
    }
    const previousHeight = transcriptEl.scrollHeight;
    loadEarlierPromise = props
      .onLoadEarlierMessages()
      .then((loaded) => {
        if (!loaded || !transcriptEl) {
          return false;
        }
        requestAnimationFrame(() => {
          if (!transcriptEl) {
            return;
          }
          transcriptEl.scrollTop += Math.max(0, transcriptEl.scrollHeight - previousHeight);
          updateTranscriptViewport();
          queueFloatingAvatarUpdate();
        });
        return true;
      })
      .finally(() => {
        loadEarlierPromise = undefined;
      });
  }

  onMount(() => {
    avatarResizeObserver = new ResizeObserver(() => {
      updateTranscriptViewport();
      queueFloatingAvatarUpdate();
    });
    if (transcriptEl) {
      avatarResizeObserver.observe(transcriptEl);
      updateTranscriptViewport();
    }
    if (transcriptInnerEl) {
      avatarResizeObserver.observe(transcriptInnerEl);
    }
    window.addEventListener("resize", queueFloatingAvatarUpdate);
    queueFloatingAvatarUpdate();
    onCleanup(() => {
      window.removeEventListener("resize", queueFloatingAvatarUpdate);
      avatarResizeObserver?.disconnect();
      if (bottomSettleTimer) {
        window.clearTimeout(bottomSettleTimer);
      }
      if (avatarFrame) {
        cancelAnimationFrame(avatarFrame);
      }
      if (measuredHeightFrame) {
        cancelAnimationFrame(measuredHeightFrame);
      }
    });
  });

  createEffect(() => {
    messageLayoutSignature();
    props.session?.status;
    props.loading;
    props.avatarSettings.display_mode;
    queueFloatingAvatarUpdate();
  });
  createEffect(() => {
    const sessionId = props.session?.id;
    if (sessionId === measuredSessionId) {
      return;
    }
    measuredSessionId = sessionId;
    virtualEntryCache.clear();
    measuredHeights.clear();
    setHeightVersion((version) => version + 1);
    requestAnimationFrame(() => {
      updateTranscriptViewport();
      queueFloatingAvatarUpdate();
    });
  });
  const avatarMode = createMemo<AvatarDisplayMode>(
    () => props.avatarSettings.display_mode ?? "static",
  );
  const isThinking = createMemo(() => sessionShowsBusyAnimation(props.session?.status));

  return (
    <section
      class="transcript"
      ref={(element) => {
        transcriptEl = element;
        props.onTranscript(element);
        avatarResizeObserver?.observe(element);
        updateTranscriptViewport();
        queueFloatingAvatarUpdate();
      }}
      onScroll={() => {
        lastScrollUpdateAt = performance.now();
        updateTranscriptViewport();
        props.onScroll();
        markManualScrollAwayFromBottom();
        maybeLoadEarlierMessages();
        queueFloatingAvatarUpdate();
      }}
      onWheel={(event) => {
        if (event.deltaY < 0) {
          lastScrolledAwayFromBottomAt = performance.now();
        }
      }}
      onPointerDown={markManualScrollAwayFromBottom}
    >
      <div
        ref={(element) => {
          transcriptInnerEl = element;
          avatarResizeObserver?.observe(element);
          queueFloatingAvatarUpdate();
        }}
        class="transcript-inner page-layer-inner"
      >
        <Show when={!props.loading} fallback={<TranscriptTextLoadingLines />}>
          <Show when={props.session} fallback={<div class="center-state">{t("ready")}</div>}>
            <Show
              when={displayMessages().length > 0}
              fallback={<div class="center-state">{sessionTitle(props.session!)}</div>}
            >
              <div
                class="transcript-virtual-space"
                style={{ height: `${virtualLayout().totalHeight}px` }}
                data-virtual-count={displayMessages().length}
                data-mounted-count={virtualItems().length}
              >
                <For each={virtualItems()}>
                  {(entry) => (
                    <VirtualMessageCell
                      entry={entry}
                      activeToolId={props.activeToolId}
                      latestId={latestId()}
                      sessionStatus={props.session?.status}
                      workspaceDirectory={props.workspaceDirectory}
                      showAvatarSpace={avatarMode() !== "hidden"}
                      onTool={props.onTool}
                      onMeasure={updateMeasuredHeight}
                    />
                  )}
                </For>
              </div>
            </Show>
            <Show when={props.conversationNotice}>{props.conversationNotice}</Show>
          </Show>
        </Show>
      </div>
      <Show when={avatarMode() !== "hidden" && floatingAvatar()}>
        {(avatar) => (
          <div
            class="floating-agent-avatar"
            aria-hidden="true"
            style={{
              left: `${avatar().left}px`,
              top: `${avatar().top}px`,
            }}
          >
            <AgentAvatarCanvas
              media={props.avatarMedia}
              settings={props.avatarSettings}
              expressionEmoji={avatarMode() === "dynamic" ? props.expressionEmoji : undefined}
              expressionId={avatarMode() === "static" ? "vigilant" : undefined}
              interactive={avatarMode() === "dynamic"}
              thinking={isThinking()}
            />
          </div>
        )}
      </Show>
    </section>
  );
}

type VirtualMessageEntry = {
  id: string;
  item: Accessor<ConversationReactionItem>;
  index: Accessor<number>;
  top: Accessor<number>;
  setItem: Setter<ConversationReactionItem>;
  setIndex: Setter<number>;
  setTop: Setter<number>;
};

function virtualEntryFor(
  cache: Map<string, VirtualMessageEntry>,
  item: ConversationReactionItem,
  index: number,
  top: number,
): VirtualMessageEntry {
  const id = item.message.id;
  const existing = cache.get(id);
  if (existing) {
    existing.setItem(() => item);
    existing.setIndex(index);
    existing.setTop(top);
    return existing;
  }
  const [itemValue, setItem] = createSignal(item);
  const [indexValue, setIndex] = createSignal(index);
  const [topValue, setTop] = createSignal(top);
  const entry = {
    id,
    item: itemValue,
    index: indexValue,
    top: topValue,
    setItem,
    setIndex,
    setTop,
  };
  cache.set(id, entry);
  return entry;
}

function VirtualMessageCell(props: {
  entry: VirtualMessageEntry;
  activeToolId?: string;
  latestId?: string;
  sessionStatus?: Session["status"];
  workspaceDirectory?: string;
  showAvatarSpace: boolean;
  onTool: (part: MessagePart, parts: MessagePart[]) => void;
  onMeasure: (messageId: string, height: number, top: number) => void;
}) {
  let rowEl: HTMLDivElement | undefined;
  let observer: ResizeObserver | undefined;

  function measure() {
    if (!rowEl) {
      return;
    }
    props.onMeasure(props.entry.item().message.id, rowEl.offsetHeight, props.entry.top());
  }

  onMount(() => {
    observer = new ResizeObserver(measure);
    if (rowEl) {
      observer.observe(rowEl);
    }
    requestAnimationFrame(measure);
    onCleanup(() => observer?.disconnect());
  });

  createEffect(() => {
    props.entry.item().message.id;
    props.entry.top();
    requestAnimationFrame(measure);
  });

  return (
    <div
      ref={rowEl}
      class="transcript-virtual-row"
      data-message-id={props.entry.item().message.id}
      data-virtual-index={props.entry.index()}
      style={{ transform: `translateY(${props.entry.top()}px)` }}
    >
      <MessageCell
        message={props.entry.item().message}
        reactions={props.entry.item().reactions}
        activeToolId={props.activeToolId}
        isLatest={props.latestId === props.entry.item().message.id}
        sessionStatus={props.sessionStatus}
        workspaceDirectory={props.workspaceDirectory}
        showAvatarSpace={props.showAvatarSpace}
        onTool={props.onTool}
      />
    </div>
  );
}

function MessageCell(props: {
  message: Message;
  reactions?: string[];
  activeToolId?: string;
  isLatest: boolean;
  sessionStatus?: Session["status"];
  workspaceDirectory?: string;
  showAvatarSpace: boolean;
  onTool: (part: MessagePart, parts: MessagePart[]) => void;
}) {
  const textParts = createMemo(() => props.message.parts.filter((part) => !isToolPart(part)));
  const toolParts = createMemo(() => props.message.parts.filter(isToolPart));
  const planRunPending = createMemo(() =>
    props.message.parts.some((part) => Boolean(asRecord(part.metadata).planRunPending)),
  );
  const planRunError = createMemo(() =>
    props.message.parts.some((part) => Boolean(asRecord(part.metadata).planRunError)),
  );
  const isPending = createMemo(
    () =>
      props.message.role === "assistant" &&
      toolParts().some((part) => toolStatus(asRecord(part.state)) === "running"),
  );
  const isAgentWorking = createMemo(
    () =>
      props.message.role === "assistant" &&
      props.isLatest &&
      sessionShowsBusyAnimation(props.sessionStatus),
  );
  const visibleTextParts = createMemo(() => textParts().filter((part) => partText(part).trim()));
  const summaryText = createMemo(() =>
    visibleTextParts().map(partText).filter(Boolean).join("\n\n"),
  );
  const hasSummary = createMemo(() => summaryText().trim().length > 0);
  const assistantBlocks = createMemo(() =>
    assistantPartBlocks(props.message.parts, new Set(visibleTextParts().map((part) => part.id))),
  );
  const turnDuration = createMemo(() => formatDuration(messageDurationMs(props.message)));
  const showAssistantMeta = createMemo(() => hasSummary() && !isAgentWorking());
  const [userExpanded, setUserExpanded] = createSignal(false);
  const userTextSignature = createMemo(() =>
    textParts()
      .map((part) => partText(part))
      .join("\n"),
  );
  const userPreview = createMemo(() => previewUserTextParts(textParts(), userExpanded()));
  const userCollapsed = createMemo(() => props.message.role === "user" && userPreview().truncated);
  const userToggleable = createMemo(() => userCollapsed() || userExpanded());

  createEffect(() => {
    props.message.id;
    userTextSignature();
    setUserExpanded(false);
  });

  function toggleUserMessage() {
    if (userToggleable()) {
      setUserExpanded((expanded) => !expanded);
    }
  }

  return (
    <article
      class={classNames(
        "message",
        props.message.role,
        props.message.role !== "user" && !props.showAvatarSpace && "avatar-hidden",
        planRunPending() && props.isLatest && "plan-run-pending",
        planRunError() && "plan-run-error",
      )}
    >
      <Show when={props.message.role === "user"}>
        <div
          class={classNames(
            "message-user-shell",
            userCollapsed() && "user-message-collapsed",
            userExpanded() && "user-message-expanded",
            userToggleable() && "user-message-toggleable",
          )}
          role={userToggleable() ? "button" : undefined}
          tabIndex={userToggleable() ? 0 : undefined}
          aria-expanded={userToggleable() ? userExpanded() : undefined}
          onClick={toggleUserMessage}
          onKeyDown={(event) => {
            if ((event.key === "Enter" || event.key === " ") && userToggleable()) {
              event.preventDefault();
              toggleUserMessage();
            }
          }}
        >
          <Index each={userPreview().parts}>
            {(part) => (
              <TextPartCell
                part={part()}
                streaming={false}
                workspaceDirectory={props.workspaceDirectory}
              />
            )}
          </Index>
          <Show when={(props.reactions?.length ?? 0) > 0}>
            <div class="message-reactions" aria-label={t("messageReactions")}>
              <For each={props.reactions}>
                {(reaction) => <span class="message-reaction">{reaction}</span>}
              </For>
            </div>
          </Show>
        </div>
      </Show>
      <Show when={props.message.role !== "user"}>
        <div class="message-body">
          <div class="assistant-response">
            <div class="message-avatar-wrap" aria-hidden="true"></div>
            <div
              class={classNames(
                "assistant-stack assistant-text",
                isAgentWorking() && !hasSummary() && "assistant-thinking-anchor",
              )}
              data-agent-avatar-anchor
              data-agent-text-block={hasSummary() || isAgentWorking() ? "" : undefined}
            >
              <Index each={assistantBlocks()}>
                {(block) => (
                  <Show
                    when={block().type === "tools"}
                    fallback={
                      <div class="assistant-text-block" data-agent-text-block>
                        <Index each={block().parts}>
                          {(part) => (
                            <TextPartCell
                              part={part()}
                              streaming={isAgentWorking()}
                              workspaceDirectory={props.workspaceDirectory}
                            />
                          )}
                        </Index>
                      </div>
                    }
                  >
                    <RunSummary
                      parts={block().parts}
                      activeToolId={props.activeToolId}
                      pending={isPending()}
                      duration={formatDuration(blockDurationMs(block().parts))}
                      onTool={(part) => props.onTool(part, block().parts)}
                    />
                  </Show>
                )}
              </Index>
              <Show when={showAssistantMeta()}>
                <div class="message-head assistant-meta">
                  <span>{agentMeta(props.message)}</span>
                  <span>
                    {formatTime(messageCreatedAt(props.message))} · {turnDuration()}
                  </span>
                </div>
              </Show>
              <Show when={isAgentWorking()}>
                <div class="assistant-thinking">正在思考</div>
              </Show>
            </div>
          </div>
        </div>
      </Show>
    </article>
  );
}

function numericField(record: Record<string, unknown>, key: string) {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

export { Composer, composerFileToken, composerImageToken } from "./composer";

function agentMeta(message: Message): string {
  const runtime = messageRuntimeMeta(message);
  const provider = runtime.providerID ?? message.providerID;
  const modelId = runtime.modelID ?? message.modelID;
  const model = [provider, modelId].filter(Boolean).join("/");
  const costValue = runtime.cost > 0 ? runtime.cost : (message.cost ?? 0);
  const cost = costValue > 0 ? `$${costValue.toFixed(4)}` : "";
  return [model, cost].filter(Boolean).join(" · ") || "Tura";
}

function messageRuntimeMeta(message: Message): {
  cost: number;
  providerID?: string;
  modelID?: string;
} {
  let cost = 0;
  let providerID: string | undefined;
  let modelID: string | undefined;

  for (const part of message.parts) {
    const state = asRecord(part.state);
    const candidates = [asRecord(part.metadata), asRecord(state.metadata)];
    for (const metadata of candidates) {
      const usage = asRecord(metadata.usage);
      cost += numericField(usage, "total_cost") ?? 0;

      const provider = asRecord(metadata.provider);
      providerID ??=
        stringField(provider, "provider_name") ??
        stringField(provider, "providerID") ??
        stringField(provider, "provider_id") ??
        stringField(metadata, "providerID") ??
        stringField(metadata, "provider_id");
      modelID ??=
        stringField(provider, "model_name") ??
        stringField(provider, "modelID") ??
        stringField(provider, "model_id") ??
        stringField(metadata, "modelID") ??
        stringField(metadata, "model_id");
    }
  }

  return { cost, providerID, modelID };
}

function stringField(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
