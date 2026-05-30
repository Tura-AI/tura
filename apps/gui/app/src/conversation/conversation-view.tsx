import {
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  type JSX,
  onCleanup,
  onMount,
  untrack,
} from "solid-js";
import ArrowDown from "lucide-solid/icons/arrow-down";
import ArrowUp from "lucide-solid/icons/arrow-up";
import FileText from "lucide-solid/icons/file-text";
import FolderOpen from "lucide-solid/icons/folder-open";
import Plus from "lucide-solid/icons/plus";
import SquareTerminal from "lucide-solid/icons/square-terminal";
import ExternalLink from "lucide-solid/icons/external-link";
import type {
  Command,
  Message,
  MessagePart,
  ServiceStatusResponse,
  Session,
} from "@tura/gateway-sdk";
import {
  type ComposerImage,
  type AppState,
  messageCreatedAt,
  partText,
  sessionTitle,
} from "../state/global-store";
import { classNames, formatTime, jsonPreview } from "../state/format";
import { t } from "../i18n";
import {
  asRecord,
  diffLines,
  formatDuration,
  isPatchRecord,
  isToolPart,
  messageDurationMs,
  toolRecords,
  toolStatus,
} from "./message-tools";
import {
  ImageLightbox,
  RichText,
  reactionEmojiValues,
  stripReactionEmoji,
} from "./message-rich-text";
import { Composer } from "./composer";

const INSPECTOR_MIN_WIDTH = 320;
const INSPECTOR_MAX_WIDTH = 680;
const INSPECTOR_COLLAPSE_WIDTH = 260;
const CONVERSATION_MAIN_MIN_WIDTH = 430;

export function ConversationView(props: {
  state: AppState;
  session?: Session;
  messages: Message[];
  slashCommands: Command[];
  onComposerText: (text: string) => void;
  onComposerImages: (images: ComposerImage[]) => void;
  onSubmit: () => void;
  compact?: boolean;
  composerToolbar?: JSX.Element;
  composerTaskList?: JSX.Element;
  conversationNotice?: JSX.Element;
  submitDisabled?: boolean;
  onToolOpen?: (part: MessagePart, parts: MessagePart[]) => void;
  compactInspector?: boolean;
  leftRailOpen?: boolean;
  leftRailWidth?: number;
  minMainWidth?: number;
  onRequestCollapseLeftRail?: () => void;
  onInspectorLayout?: (layout: { open: boolean; width: number }) => void;
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
  const groupedMessages = createMemo(() =>
    groupConversationTurns(props.messages),
  );
  const streamSignature = createMemo(() =>
    groupedMessages()
      .flatMap((message) => message.parts)
      .map(
        (part) =>
          `${part.id}:${partText(part).length}:${toolStatus(asRecord(part.state))}`,
      )
      .join("|"),
  );
  let transcriptEl: HTMLElement | undefined;
  let conversationMainEl: HTMLDivElement | undefined;
  let scrollFollowFrame: number | undefined;
  let scrollFollowObserver: ResizeObserver | undefined;
  let inspectorSessionId = props.session?.id;
  const [scrollFollowBottom, setScrollFollowBottom] = createSignal(166);
  const minMainWidth = createMemo(
    () => props.minMainWidth ?? CONVERSATION_MAIN_MIN_WIDTH,
  );

  function leftRailWidth() {
    return props.leftRailOpen ? (props.leftRailWidth ?? 0) : 0;
  }

  function mainWidthWith(leftWidth: number, rightWidth: number) {
    return viewportWidth() - leftWidth - rightWidth;
  }

  function canFitInspector(width: number, leftWidth = leftRailWidth()) {
    return mainWidthWith(leftWidth, width) >= minMainWidth();
  }

  function collapseLeftIfInspectorNeedsRoom(width = inspectorWidth()) {
    if (props.leftRailOpen && !canFitInspector(width)) {
      props.onRequestCollapseLeftRail?.();
      return true;
    }
    return false;
  }

  function inspectorMaxWidth(leftAlreadyCollapsed = false) {
    const left = leftAlreadyCollapsed ? 0 : leftRailWidth();
    return Math.min(
      INSPECTOR_MAX_WIDTH,
      Math.max(0, viewportWidth() - left - minMainWidth()),
    );
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
    const needsLeftCollapsed =
      collapseLeftIfInspectorNeedsRoom(INSPECTOR_MIN_WIDTH);
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
    if (
      !inspectorOpen() ||
      inspectorOverlay() ||
      canFitInspector(inspectorWidth())
    ) {
      return;
    }
    if (props.leftRailOpen && canFitInspector(INSPECTOR_MIN_WIDTH, 0)) {
      props.onRequestCollapseLeftRail?.();
      return;
    }
    if (!props.leftRailOpen || !canFitInspector(INSPECTOR_MIN_WIDTH, 0)) {
      setInspectorOpen(false);
    }
  });

  createEffect(() => {
    props.onInspectorLayout?.({
      open: inspectorOpen() && !inspectorOverlay(),
      width: inspectorOpen() && !inspectorOverlay() ? inspectorWidth() : 0,
    });
  });

  function transcriptAtBottom() {
    if (!transcriptEl) {
      return true;
    }
    return (
      transcriptEl.scrollHeight -
        transcriptEl.scrollTop -
        transcriptEl.clientHeight <
      28
    );
  }

  function scrollTranscriptToBottom(behavior: ScrollBehavior = "smooth") {
    if (!transcriptEl) {
      return;
    }
    setTranscriptPinned(true);
    requestAnimationFrame(() => {
      transcriptEl?.scrollTo({
        top: transcriptEl.scrollHeight,
        behavior,
      });
    });
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
    setScrollFollowBottom(
      Math.max(14, Math.round(mainRect.bottom - transcriptRect.bottom + 10)),
    );
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

  createEffect(() => {
    streamSignature();
    if (transcriptPinned()) {
      scrollTranscriptToBottom("auto");
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
          <h1>
            {props.session ? sessionTitle(props.session) : t("newSession")}
          </h1>
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
            loading={props.state.loading}
            activeToolId={selectedToolId()}
            conversationNotice={props.conversationNotice}
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
            onClick={() => scrollTranscriptToBottom()}
          >
            <ArrowDown size={18} strokeWidth={1.7} />
          </button>
        </Show>
      </div>
      <div class="conversation-bottom page-layer-bottom">
        <Show when={props.composerTaskList}>
          <div class="composer-task-dock">{props.composerTaskList}</div>
        </Show>
        <Composer
          text={props.state.composerText}
          images={props.state.composerImages}
          submitting={props.state.submitting}
          slashCommands={props.slashCommands}
          onText={props.onComposerText}
          onImages={props.onComposerImages}
          onSubmit={props.onSubmit}
          toolbar={props.composerToolbar}
          submitDisabled={props.submitDisabled}
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

function groupConversationTurns(messages: Message[]): Message[] {
  const grouped: Message[] = [];
  let assistantGroup: Message[] = [];

  function flushAssistantGroup() {
    if (assistantGroup.length === 0) {
      return;
    }
    grouped.push(mergeAssistantMessages(assistantGroup));
    assistantGroup = [];
  }

  for (const message of messages) {
    if (message.role === "assistant") {
      if (isReactionOnlyMessage(message)) {
        flushAssistantGroup();
        grouped.push(message);
        continue;
      }
      assistantGroup.push(message);
      continue;
    }
    flushAssistantGroup();
    grouped.push(message);
  }
  flushAssistantGroup();
  return grouped;
}

function mergeAssistantMessages(messages: Message[]): Message {
  const first = messages[0]!;
  const last = messages.at(-1)!;
  const withText = [...messages]
    .reverse()
    .find((message) =>
      message.parts.some((part) => !isToolPart(part) && partText(part).trim()),
    );
  const providerMessage = withText ?? last;
  return {
    ...providerMessage,
    id: messages.map((message) => message.id).join("+"),
    created_at: first.created_at ?? first.time?.created,
    updated_at: last.updated_at ?? last.time?.updated,
    time: {
      created: messageCreatedAt(first),
      updated: last.time?.updated ?? last.updated_at ?? messageCreatedAt(last),
    },
    parts: messages.flatMap((message) => message.parts),
  };
}

function ToolInspector(props: {
  parts: MessagePart[];
  serviceStatus?: ServiceStatusResponse;
  selectedId?: string;
  open: boolean;
  overlay: boolean;
  width: number;
  maxWidth: number;
  leftRailOpen?: boolean;
  leftRailWidth?: number;
  minMainWidth: number;
  onRequestCollapseLeftRail?: () => void;
  onWidth: (width: number) => void;
  onSelect: (partId: string) => void;
  onClose: () => void;
}) {
  const records = createMemo(() => toolRecords(props.parts));
  const [expandedId, setExpandedId] = createSignal<string>();
  const totalDuration = createMemo(() =>
    formatDuration(
      records().reduce(
        (duration, record) => duration + (record.durationMs ?? 0),
        0,
      ),
    ),
  );
  let dragStart = 0;
  let widthStart = 0;
  let resizing = false;

  createEffect(() => {
    if (!props.open) {
      setExpandedId(undefined);
    }
  });

  function startResize(clientX: number) {
    resizing = true;
    dragStart = clientX;
    widthStart = props.width;
    document.body.classList.add("resizing-inspector");
    window.addEventListener("mousemove", resizeMouse);
    window.addEventListener("touchmove", resizeTouch, { passive: false });
    window.addEventListener("mouseup", stopResize, { once: true });
    window.addEventListener("touchend", stopResize, { once: true });
    window.addEventListener("touchcancel", stopResize, { once: true });
  }

  function handleMouseDown(event: MouseEvent) {
    event.preventDefault();
    startResize(event.clientX);
  }

  function handleTouchStart(event: TouchEvent) {
    const touch = event.touches[0];
    if (!touch) return;
    event.preventDefault();
    startResize(touch.clientX);
  }

  function updateWidth(clientX: number) {
    if (props.overlay) {
      return;
    }
    const next = widthStart + dragStart - clientX;
    if (next <= INSPECTOR_COLLAPSE_WIDTH) {
      props.onWidth(INSPECTOR_MIN_WIDTH);
      props.onClose();
      stopResize();
      return;
    }
    if (
      props.leftRailOpen &&
      window.innerWidth -
        (props.leftRailWidth ?? 0) -
        Math.max(INSPECTOR_MIN_WIDTH, next) <
        props.minMainWidth
    ) {
      props.onRequestCollapseLeftRail?.();
    }
    if (props.maxWidth < INSPECTOR_MIN_WIDTH) {
      props.onClose();
      stopResize();
      return;
    }
    props.onWidth(
      Math.min(props.maxWidth, Math.max(INSPECTOR_MIN_WIDTH, next)),
    );
  }

  function resizeMouse(event: MouseEvent) {
    if (!resizing) return;
    updateWidth(event.clientX);
  }

  function resizeTouch(event: TouchEvent) {
    const touch = event.touches[0];
    if (!resizing || !touch) return;
    event.preventDefault();
    updateWidth(touch.clientX);
  }

  function stopResize() {
    resizing = false;
    window.removeEventListener("mousemove", resizeMouse);
    window.removeEventListener("touchmove", resizeTouch);
    document.body.classList.remove("resizing-inspector");
  }

  onCleanup(() => {
    window.removeEventListener("mousemove", resizeMouse);
    window.removeEventListener("touchmove", resizeTouch);
    document.body.classList.remove("resizing-inspector");
  });

  return (
    <aside
      class={classNames(
        "tool-inspector",
        props.open && "open",
        props.overlay && "mobile",
      )}
      data-empty={records().length === 0}
      aria-hidden={!props.open}
      style={{
        "--inspector-width": `${props.width}px`,
        "--inspector-max-width": `${props.maxWidth}px`,
      }}
    >
      <div
        class="inspector-resize"
        role="separator"
        aria-orientation="vertical"
        onMouseDown={handleMouseDown}
        onTouchStart={handleTouchStart}
      />
      <Show
        when={records().length > 0}
        fallback={
          <>
            <header>
              <span>{t("console")}</span>
              <small>{t("idle")}</small>
            </header>
            <div class="inspector-empty">{t("selectStep")}</div>
          </>
        }
      >
        <>
          <header>
            <span>{t("runCommands", { count: records().length })}</span>
            <small>{totalDuration()}</small>
            <button
              class="inspector-close"
              type="button"
              title={t("close")}
              onClick={props.onClose}
            >
              ×
            </button>
          </header>
          <div class="inspector-scroll">
            <nav
              class="inspector-steps inspector-records"
              aria-label={t("toolSteps")}
            >
              <For each={records()}>
                {(record, index) => {
                  const expanded = createMemo(() => expandedId() === record.id);
                  const groupStart = createMemo(() => {
                    const previous = records()[index() - 1];
                    return !!(
                      previous?.groupId &&
                      record.groupId &&
                      previous.groupId !== record.groupId
                    );
                  });
                  return (
                    <section
                      data-part-id={record.partId}
                      class={classNames(
                        "inspector-record",
                        expanded() && "expanded",
                        groupStart() && "group-start",
                        record.status === "running" && "running",
                        isPatchRecord(record) && "patch-record",
                      )}
                    >
                      <button
                        class="inspector-record-toggle"
                        type="button"
                        aria-expanded={expanded()}
                        onClick={() => {
                          props.onSelect(record.id);
                          setExpandedId(expanded() ? undefined : record.id);
                        }}
                      >
                        <span>{record.title}</span>
                        <small>
                          {toolStatusLabel(record.status)} ·{" "}
                          {formatDuration(record.durationMs)}
                        </small>
                      </button>
                      <Show when={expanded()}>
                        <div class="inspector-record-body">
                          <section class="inspector-block">
                            <span>{t("command")}</span>
                            <pre
                              class="inspector-code inspector-command"
                              textContent={record.command}
                            />
                          </section>
                          <Show
                            when={isPatchRecord(record)}
                            fallback={
                              <section class="inspector-block">
                                <span>{t("console")}</span>
                                <pre
                                  class="inspector-code inspector-console"
                                  textContent={record.output}
                                />
                              </section>
                            }
                          >
                            <section class="inspector-block">
                              <span>{t("patch")}</span>
                              <DiffPanel
                                output={record.output}
                                command={record.command}
                              />
                            </section>
                          </Show>
                          <footer class="inspector-status">
                            <span>{toolStatusLabel(record.status)}</span>
                            <span>
                              {serviceStatusLabel(props.serviceStatus)}
                            </span>
                            <span>
                              {t("exitCode")}:{" "}
                              {record.exitCode === undefined
                                ? "--"
                                : record.exitCode}
                            </span>
                          </footer>
                        </div>
                      </Show>
                    </section>
                  );
                }}
              </For>
            </nav>
          </div>
        </>
      </Show>
    </aside>
  );
}

function DiffPanel(props: { output: string; command: string }) {
  const lines = createMemo(() => diffLines(props.output));
  const added = createMemo(
    () => lines().filter((line) => line.kind === "add").length,
  );
  const deleted = createMemo(
    () => lines().filter((line) => line.kind === "del").length,
  );
  const file = createMemo(() => diffFileLabel(props.output) ?? props.command);
  return (
    <div class="diff-view github-diff">
      <div class="diff-head">
        <span>{file()}</span>
        <small>
          +{added()} -{deleted()}
        </small>
      </div>
      <For each={lines()}>
        {(line, index) => (
          <code
            class={classNames(
              line.kind === "add" && "diff-add",
              line.kind === "del" && "diff-del",
            )}
          >
            <span>{index() + 1}</span>
            <span>{line.text}</span>
          </code>
        )}
      </For>
    </div>
  );
}

function Transcript(props: {
  session?: Session;
  messages: Message[];
  loading: boolean;
  activeToolId?: string;
  conversationNotice?: JSX.Element;
  onTranscript: (element: HTMLElement) => void;
  onScroll: () => void;
  onTool: (part: MessagePart, parts: MessagePart[]) => void;
}) {
  const latestId = createMemo(() => props.messages.at(-1)?.id);
  const displayMessages = createMemo(() =>
    conversationReactionItems(props.messages),
  );
  return (
    <section
      class="transcript"
      ref={props.onTranscript}
      onScroll={props.onScroll}
    >
      <div class="transcript-inner page-layer-inner">
        <Show
          when={!props.loading}
          fallback={
            <div class="transcript-loading-placeholder">
              <div class="loading-bar wide" />
              <div class="loading-bar medium" />
              <div class="loading-bar" />
            </div>
          }
        >
          <Show
            when={props.session}
            fallback={<div class="center-state">{t("ready")}</div>}
          >
            <For
              each={displayMessages()}
              fallback={
                <div class="center-state">{sessionTitle(props.session!)}</div>
              }
            >
              {(item) => (
                <MessageCell
                  message={item.message}
                  reactions={item.reactions}
                  activeToolId={props.activeToolId}
                  isLatest={latestId() === item.message.id}
                  sessionStatus={props.session?.status}
                  onTool={props.onTool}
                />
              )}
            </For>
            <Show when={props.conversationNotice}>
              {props.conversationNotice}
            </Show>
          </Show>
        </Show>
      </div>
    </section>
  );
}

function MessageCell(props: {
  message: Message;
  reactions?: string[];
  activeToolId?: string;
  isLatest: boolean;
  sessionStatus?: Session["status"];
  onTool: (part: MessagePart, parts: MessagePart[]) => void;
}) {
  const textParts = createMemo(() =>
    props.message.parts.filter((part) => !isToolPart(part)),
  );
  const toolParts = createMemo(() => props.message.parts.filter(isToolPart));
  const planRunPending = createMemo(() =>
    props.message.parts.some((part) =>
      Boolean(asRecord(part.metadata).planRunPending),
    ),
  );
  const planRunError = createMemo(() =>
    props.message.parts.some((part) =>
      Boolean(asRecord(part.metadata).planRunError),
    ),
  );
  const isPending = createMemo(
    () =>
      props.message.role === "assistant" &&
      toolParts().some(
        (part) => toolStatus(asRecord(part.state)) === "running",
      ),
  );
  const isAgentWorking = createMemo(
    () =>
      props.message.role === "assistant" &&
      props.isLatest &&
      (props.sessionStatus === undefined
        ? isPending()
        : props.sessionStatus !== "idle"),
  );
  const [pulse, setPulse] = createSignal(false);
  let pulseTimer: number | undefined;
  const messagePulseSignature = createMemo(() =>
    props.message.parts
      .map(
        (part) =>
          `${part.id}:${partText(part).length}:${toolStatus(asRecord(part.state))}`,
      )
      .join("|"),
  );
  createEffect(() => {
    const signature = messagePulseSignature();
    const previousSignature = messagePulseSignatureCache.get(props.message.id);
    messagePulseSignatureCache.set(props.message.id, signature);
    if (props.message.role !== "assistant" || !props.isLatest) {
      return;
    }
    if (previousSignature === signature) {
      return;
    }
    setPulse(false);
    if (pulseTimer) {
      window.clearTimeout(pulseTimer);
    }
    requestAnimationFrame(() => setPulse(true));
    pulseTimer = window.setTimeout(() => setPulse(false), 420);
  });
  onCleanup(() => {
    if (pulseTimer) {
      window.clearTimeout(pulseTimer);
    }
  });
  const visibleTextParts = createMemo(() => {
    const visible = textParts().filter((part) => partText(part).trim());
    const showProcessText =
      props.sessionStatus === undefined
        ? isPending()
        : props.sessionStatus !== "idle";
    if (
      props.message.role !== "assistant" ||
      showProcessText ||
      visible.length <= 1
    ) {
      return visible;
    }
    return [visible[visible.length - 1]!];
  });
  const summaryText = createMemo(() =>
    visibleTextParts().map(partText).filter(Boolean).join("\n\n"),
  );
  const hasSummary = createMemo(() => summaryText().trim().length > 0);
  const assistantBlocks = createMemo(() =>
    assistantPartBlocks(
      props.message.parts,
      new Set(visibleTextParts().map((part) => part.id)),
    ),
  );
  const turnDuration = createMemo(() =>
    formatDuration(messageDurationMs(props.message)),
  );
  const showAssistantMeta = createMemo(() => hasSummary() && !isAgentWorking());

  return (
    <article
      class={classNames(
        "message",
        props.message.role,
        planRunPending() && props.isLatest && "plan-run-pending",
        planRunError() && "plan-run-error",
        pulse() && "message-arrival-pulse",
      )}
    >
      <Show when={props.message.role === "user"}>
        <div class="message-user-shell">
          <For each={textParts()}>
            {(part) => <TextPartCell part={part} streaming={false} />}
          </For>
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
            <div class="message-avatar-wrap" aria-hidden="true">
              <img
                class="agent-avatar"
                src="/assets/conversation-avatar.png"
                alt=""
              />
            </div>
            <div class="assistant-stack assistant-text">
              <For each={assistantBlocks()}>
                {(block) => (
                  <Show
                    when={block.type === "tools"}
                    fallback={
                      <div class="assistant-text-block">
                        <For each={block.parts}>
                          {(part) => (
                            <TextPartCell
                              part={part}
                              streaming={isAgentWorking()}
                            />
                          )}
                        </For>
                      </div>
                    }
                  >
                    <RunSummary
                      parts={block.parts}
                      activeToolId={props.activeToolId}
                      pending={isPending()}
                      duration={formatDuration(blockDurationMs(block.parts))}
                      onTool={(part) => props.onTool(part, block.parts)}
                    />
                  </Show>
                )}
              </For>
              <Show when={showAssistantMeta()}>
                <div class="message-head assistant-meta">
                  <span>{agentMeta(props.message)}</span>
                  <span>
                    {formatTime(messageCreatedAt(props.message))} ·{" "}
                    {turnDuration()}
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

type AssistantBlock = {
  type: "text" | "tools";
  parts: MessagePart[];
};

function assistantPartBlocks(
  parts: MessagePart[],
  visibleTextIds: Set<string>,
): AssistantBlock[] {
  const blocks: AssistantBlock[] = [];
  let toolBuffer: MessagePart[] = [];

  function flushTools() {
    if (toolBuffer.length > 0) {
      blocks.push({ type: "tools", parts: toolBuffer });
      toolBuffer = [];
    }
  }

  for (const part of parts) {
    if (isToolPart(part)) {
      toolBuffer.push(part);
      continue;
    }
    if (!visibleTextIds.has(part.id)) {
      continue;
    }
    flushTools();
    blocks.push({ type: "text", parts: [part] });
  }
  flushTools();
  return [
    ...blocks.filter((block) => block.type === "tools"),
    ...blocks.filter((block) => block.type === "text"),
  ];
}

function blockDurationMs(parts: MessagePart[]): number | undefined {
  const durations = parts
    .map((part) => messagePartDurationMs(part))
    .filter((value): value is number => value !== undefined);
  return durations.length
    ? durations.reduce((total, value) => total + value, 0)
    : undefined;
}

function messagePartDurationMs(part: MessagePart): number | undefined {
  const state = asRecord(part.state);
  const time = asRecord(state.time);
  const start =
    numericField(time, "start") ||
    numericField(time, "started") ||
    numericField(state, "started_at");
  const end =
    numericField(time, "end") ||
    numericField(time, "ended") ||
    numericField(state, "completed_at");
  if (!start) {
    return undefined;
  }
  return Math.max(0, epochMs(end ?? Date.now()) - epochMs(start));
}

function numericField(record: Record<string, unknown>, key: string) {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value)
    ? value
    : undefined;
}

function epochMs(value: number) {
  return value > 10_000_000_000 ? value : value * 1000;
}

function RunSummary(props: {
  parts: MessagePart[];
  activeToolId?: string;
  pending: boolean;
  duration: string;
  onTool: (part: MessagePart) => void;
}) {
  const recordCount = createMemo(() => toolRecords(props.parts).length);
  const selectedPart = createMemo(
    () =>
      props.parts.find((part) => part.id === props.activeToolId) ??
      preferredToolPart(props.parts),
  );
  const label = createMemo(() =>
    t(props.pending ? "runningCommands" : "runCommands", {
      count: recordCount(),
    }),
  );
  return (
    <button
      class="run-summary"
      type="button"
      title={`${label()} · ${props.duration}`}
      onClick={() => {
        const part = selectedPart();
        if (part) {
          props.onTool(part);
        }
      }}
    >
      <SquareTerminal size={14} strokeWidth={1.8} />
      <span>{label()}</span>
      <span class="run-summary-time">{props.duration}</span>
      <span class="run-summary-chevron">›</span>
    </button>
  );
}

function TextPartCell(props: { part: MessagePart; streaming: boolean }) {
  const text = createMemo(() => stripReactionEmoji(partText(props.part)));
  return (
    <div class="part text-part">
      <Show
        when={text()}
        fallback={
          <pre>{jsonPreview(props.part.state || props.part.metadata)}</pre>
        }
      >
        {(value) => (
          <TypingText
            id={props.part.id}
            text={value()}
            active={props.streaming}
          />
        )}
      </Show>
    </div>
  );
}

type ConversationReactionItem = {
  message: Message;
  reactions: string[];
};

function conversationReactionItems(
  messages: Message[],
): ConversationReactionItem[] {
  const items: ConversationReactionItem[] = [];
  for (const message of messages) {
    const reactions = messageReactionEmojis(message);
    if (
      message.role === "assistant" &&
      reactions.length > 0 &&
      messageWithoutReactionsText(message).trim().length === 0
    ) {
      const target = [...items]
        .reverse()
        .find((item) => item.message.role === "user");
      if (target) {
        target.reactions = [...target.reactions, ...reactions].slice(0, 4);
        continue;
      }
    }
    items.push({
      message,
      reactions: message.role === "user" ? reactions : [],
    });
  }
  return items;
}

function messageReactionEmojis(message: Message): string[] {
  return message.parts
    .filter((part) => !isToolPart(part))
    .flatMap((part) => reactionEmojiValues(partText(part)));
}

function messageWithoutReactionsText(message: Message): string {
  return message.parts
    .filter((part) => !isToolPart(part))
    .map((part) => stripReactionEmoji(partText(part)))
    .join("\n");
}

function isReactionOnlyMessage(message: Message): boolean {
  return (
    message.role === "assistant" &&
    messageReactionEmojis(message).length > 0 &&
    messageWithoutReactionsText(message).trim().length === 0 &&
    message.parts.every((part) => !isToolPart(part))
  );
}

const typingTextCache = new Map<string, string>();
const completedTypingTextCache = new Set<string>();
const messagePulseSignatureCache = new Map<string, string>();

function TypingText(props: { id: string; text: string; active: boolean }) {
  const [visible, setVisible] = createSignal(
    props.active && !completedTypingTextCache.has(props.text)
      ? (typingTextCache.get(props.id) ?? "")
      : props.text,
  );
  let timer: number | undefined;

  const setCachedVisible = (id: string, text: string, value: string) => {
    setVisible(value);
    typingTextCache.set(id, value);
    if (value === text) {
      completedTypingTextCache.add(text);
    }
  };

  createEffect(() => {
    const text = props.text;
    const active = props.active;
    const id = props.id;
    if (timer) {
      window.clearInterval(timer);
      timer = undefined;
    }
    if (!active || completedTypingTextCache.has(text)) {
      setCachedVisible(id, text, text);
      return;
    }
    const cached = typingTextCache.get(id);
    const current = untrack(visible);
    const seed =
      cached && text.startsWith(cached) && cached.length > current.length
        ? cached
        : current;
    if (seed === text) {
      setCachedVisible(id, text, text);
      return;
    }
    const start = text.startsWith(seed) ? seed.length : 0;
    if (start === 0) {
      setCachedVisible(id, text, "");
    }
    let index = start;
    timer = window.setInterval(() => {
      index = Math.min(
        text.length,
        index + Math.max(1, Math.ceil((text.length - index) / 24)),
      );
      const next = text.slice(0, index);
      setCachedVisible(id, text, next);
      if (index >= text.length && timer) {
        window.clearInterval(timer);
        timer = undefined;
      }
    }, 18);
  });

  onCleanup(() => {
    if (timer) {
      window.clearInterval(timer);
    }
  });

  return <RichText text={visible()} active={props.active} />;
}

export { Composer, composerFileToken, composerImageToken } from "./composer";

function toolStatusLabel(status: string): string {
  switch (status) {
    case "completed":
    case "success":
    case "done":
      return t("completed");
    case "running":
    case "in_progress":
      return t("running");
    case "failed":
    case "error":
      return t("failed");
    case "pending":
      return t("pending");
    default:
      return status;
  }
}

function serviceStatusLabel(status?: ServiceStatusResponse): string {
  if (!status) {
    return `${t("backgroundService")}: ${t("unknown")}`;
  }
  const processes = sessionProcessCount(status.session_processes);
  const lspCount = status.lsp?.length ?? 0;
  const health = status.router?.status || status.mano?.status || "unknown";
  const parts = [
    toolServiceStatusLabel(health),
    processes === 0
      ? t("serviceNoProcesses")
      : t("serviceProcesses", { count: processes }),
    lspCount > 0 ? t("serviceLsp", { count: lspCount }) : "",
  ].filter(Boolean);
  return `${t("backgroundService")}: ${parts.join(" · ")}`;
}

function toolServiceStatusLabel(status: string): string {
  switch (status) {
    case "connected":
      return t("connected");
    case "checking":
      return t("checking");
    case "error":
      return t("failed");
    default:
      return status || t("unknown");
  }
}

function sessionProcessCount(value: unknown): number {
  const record =
    value && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : {};
  const processes = record.processes;
  return Array.isArray(processes) ? processes.length : 0;
}

function diffFileLabel(output: string): string | undefined {
  const match = output.match(/^diff --git a\/(.+?) b\/(.+)$/mu);
  return match?.[2] ?? match?.[1];
}

function preferredToolPart(parts: MessagePart[]): MessagePart | undefined {
  return (
    [...parts].reverse().find((part) => part.tool !== "runtime") ?? parts.at(-1)
  );
}

function agentMeta(message: Message): string {
  const model = [message.providerID, message.modelID].filter(Boolean).join("/");
  const cost = message.cost ? `$${message.cost.toFixed(4)}` : "";
  const detail = [model, cost].filter(Boolean).join(" · ");
  return detail ? `Tura (${detail})` : "Tura";
}
