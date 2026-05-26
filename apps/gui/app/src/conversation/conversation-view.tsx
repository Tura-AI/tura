import {
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  type JSX,
  onCleanup,
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
  onToolOpen?: () => void;
}) {
  const [selectedToolId, setSelectedToolId] = createSignal<string>();
  const [inspectorParts, setInspectorParts] = createSignal<MessagePart[]>([]);
  const [inspectorOpen, setInspectorOpen] = createSignal(false);
  const [inspectorWidth, setInspectorWidth] = createSignal(430);
  const [transcriptPinned, setTranscriptPinned] = createSignal(true);
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
        props.compact && "compact-conversation",
        inspectorOpen() && "inspector-open",
      )}
      style={{ "--inspector-width": `${inspectorWidth()}px` }}
    >
      <header class="page-head">
        <div class="page-title">
          <span>{t("conversation")}</span>
          <h1>
            {props.session ? sessionTitle(props.session) : t("newSession")}
          </h1>
        </div>
      </header>
      <div class="conversation-grid">
        <div class="conversation-main">
          <Transcript
            session={props.session}
            messages={groupedMessages()}
            loading={props.state.loading}
            activeToolId={selectedToolId()}
            conversationNotice={props.conversationNotice}
            onTranscript={(element) => {
              transcriptEl = element;
            }}
            onScroll={handleTranscriptScroll}
            onTool={(part, parts) => {
              if (props.compact && props.onToolOpen) {
                props.onToolOpen();
                return;
              }
              setSelectedToolId(part.id);
              setInspectorParts(parts);
              setInspectorOpen(true);
            }}
          />
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
      </div>
      <Show when={!props.compact}>
        <ToolInspector
          parts={inspectorParts()}
          serviceStatus={props.state.serviceStatus}
          selectedId={selectedToolId()}
          open={inspectorOpen()}
          width={inspectorWidth()}
          onWidth={setInspectorWidth}
          onSelect={setSelectedToolId}
          onClose={() => setInspectorOpen(false)}
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
  width: number;
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
    const next = widthStart + dragStart - clientX;
    const rail =
      Number.parseFloat(
        getComputedStyle(document.documentElement).getPropertyValue("--rail"),
      ) || 0;
    const max = Math.min(760, Math.max(320, window.innerWidth - rail - 360));
    props.onWidth(Math.min(max, Math.max(320, next)));
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
      class={classNames("tool-inspector", props.open && "open")}
      data-empty={records().length === 0}
      aria-hidden={!props.open}
      style={{ "--inspector-width": `${props.width}px` }}
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
      <Show
        when={!props.loading}
        fallback={<div class="center-state">{t("loading")}</div>}
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
    messagePulseSignature();
    if (props.message.role !== "assistant" || !props.isLatest) {
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

  return (
    <article
      class={classNames(
        "message",
        props.message.role,
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
              <Show when={hasSummary()}>
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
  return blocks;
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
        {(value) => <TypingText text={value()} active={props.streaming} />}
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

function TypingText(props: { text: string; active: boolean }) {
  const [visible, setVisible] = createSignal(props.active ? "" : props.text);
  let timer: number | undefined;

  createEffect(() => {
    const text = props.text;
    if (!props.active) {
      setVisible(text);
      return;
    }
    if (timer) {
      window.clearInterval(timer);
    }
    const current = visible();
    const start = text.startsWith(current) ? current.length : 0;
    if (start === 0) {
      setVisible("");
    }
    let index = start;
    timer = window.setInterval(() => {
      index = Math.min(
        text.length,
        index + Math.max(1, Math.ceil((text.length - index) / 24)),
      );
      setVisible(text.slice(0, index));
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

export function Composer(props: {
  text: string;
  images: ComposerImage[];
  submitting: boolean;
  slashCommands: Command[];
  onText: (text: string) => void;
  onImages: (images: ComposerImage[]) => void;
  onSubmit: () => void;
  toolbar?: JSX.Element;
  submitDisabled?: boolean;
}) {
  let fileInput: HTMLInputElement | undefined;
  let textarea: HTMLTextAreaElement | undefined;
  let editor: HTMLDivElement | undefined;
  let attachmentPressTimer: number | undefined;
  const [previewImageId, setPreviewImageId] = createSignal<string>();
  const [attachmentMenu, setAttachmentMenu] = createSignal<{
    id: string;
    x: number;
    y: number;
  }>();
  const imageById = createMemo(
    () => new Map(props.images.map((image) => [image.id, image])),
  );
  const attachmentsById = imageById;
  const previewImage = createMemo(() =>
    previewImageId() ? imageById().get(previewImageId()!) : undefined,
  );
  const imagePaths = createMemo(() =>
    props.images
      .filter((image) => attachmentKind(image) === "image")
      .map((image) => image.dataUrl),
  );
  const previewImageIndex = createMemo(() => {
    const image = previewImage();
    return image ? Math.max(0, imagePaths().indexOf(image.dataUrl)) : 0;
  });

  createEffect(() => {
    if (!attachmentMenu()) {
      return;
    }
    const close = () => setAttachmentMenu(undefined);
    document.addEventListener("pointerdown", close);
    onCleanup(() => document.removeEventListener("pointerdown", close));
  });

  onCleanup(() => {
    if (attachmentPressTimer) {
      window.clearTimeout(attachmentPressTimer);
    }
  });

  async function attachFiles(files: FileList | null) {
    const selectedFiles = Array.from(files ?? []);
    if (selectedFiles.length === 0) {
      return;
    }
    const inserted: ComposerImage[] = [];
    for (const file of selectedFiles) {
      const kind = file.type.startsWith("image/") ? "image" : "file";
      inserted.push({
        id: crypto.randomUUID(),
        name: file.name,
        dataUrl:
          kind === "image"
            ? await readImageDataUrl(file)
            : URL.createObjectURL(file),
        objectUrl: URL.createObjectURL(file),
        mimeType: file.type,
        kind,
      });
    }
    props.onImages([...props.images, ...inserted]);
    insertComposerTokens(inserted);
    if (fileInput) {
      fileInput.value = "";
    }
  }

  function insertComposerTokens(images: ComposerImage[]) {
    const tokens = images
      .map((image) => composerAttachmentToken(image))
      .join("\n");
    const before = props.text;
    const after: string = "";
    const prefix = before && !before.endsWith("\n") ? "\n" : "";
    const nextText = `${before}${prefix}${tokens}${after}`;
    props.onText(nextText);
    requestAnimationFrame(() => {
      editor?.focus();
    });
  }

  function removeAttachment(id: string) {
    props.onImages(props.images.filter((image) => image.id !== id));
    props.onText(removeComposerAttachmentToken(props.text, id));
  }

  function editorText(): string {
    if (!editor) {
      return props.text;
    }
    let value = "";
    for (const node of Array.from(editor.childNodes)) {
      if (node instanceof HTMLElement && node.dataset.attachmentId) {
        value += composerTokenForElement(node);
      } else {
        value += node.textContent ?? "";
      }
    }
    return value.replace(/\u00a0/gu, " ");
  }

  function syncEditor() {
    props.onText(editorText());
  }

  function copyEditorText(event: ClipboardEvent) {
    if (!editor || !document.getSelection()?.containsNode(editor, true)) {
      return;
    }
    event.preventDefault();
    event.clipboardData?.setData("text/plain", editorText());
  }

  function viewAttachment(attachment: ComposerImage) {
    setAttachmentMenu(undefined);
    if (attachmentKind(attachment) === "image") {
      setPreviewImageId(attachment.id);
      return;
    }
    window.open(
      attachment.objectUrl ?? attachment.dataUrl,
      "_blank",
      "noopener",
    );
  }

  function openAttachmentLocation(attachment: ComposerImage) {
    setAttachmentMenu(undefined);
    window.open(
      attachment.objectUrl ?? attachment.dataUrl,
      "_blank",
      "noopener",
    );
  }

  function openAttachmentMenu(
    event: MouseEvent | PointerEvent,
    attachment: ComposerImage,
  ) {
    event.preventDefault();
    event.stopPropagation();
    setAttachmentMenu({
      id: attachment.id,
      x: event.clientX,
      y: event.clientY,
    });
  }

  function beginAttachmentPress(
    event: PointerEvent,
    attachment: ComposerImage,
  ) {
    if (event.pointerType !== "touch") {
      return;
    }
    attachmentPressTimer = window.setTimeout(() => {
      openAttachmentMenu(event, attachment);
    }, 520);
  }

  function cancelAttachmentPress() {
    if (attachmentPressTimer) {
      window.clearTimeout(attachmentPressTimer);
      attachmentPressTimer = undefined;
    }
  }

  return (
    <footer class="bottom-composer composer">
      <Show when={props.slashCommands.length > 0}>
        <div class="slash-menu">
          <For each={props.slashCommands}>
            {(command) => (
              <button onClick={() => props.onText(`/${command.name} `)}>
                <span>/{command.name}</span>
                <small>{command.description}</small>
              </button>
            )}
          </For>
        </div>
      </Show>
      <div
        class="composer-input"
        onDragOver={(event) => {
          if (
            Array.from(event.dataTransfer?.items ?? []).some(
              (item) => item.kind === "file",
            )
          ) {
            event.preventDefault();
          }
        }}
        onDrop={(event) => {
          event.preventDefault();
          void attachFiles(event.dataTransfer?.files ?? null);
        }}
      >
        <div
          ref={editor}
          class="composer-rich-editor"
          contentEditable
          role="textbox"
          aria-multiline="true"
          data-placeholder={t("writeMessage")}
          onInput={syncEditor}
          onCopy={copyEditorText}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
              event.preventDefault();
              void props.onSubmit();
            }
          }}
          onPaste={(event) => {
            event.preventDefault();
            const text = event.clipboardData?.getData("text/plain") ?? "";
            document.execCommand("insertText", false, text);
            syncEditor();
          }}
        >
          <For each={composerPreviewSegments(props.text)}>
            {(segment) => (
              <Show when={segment.type !== "text"} fallback={segment.value}>
                {(() => {
                  const attachment = attachmentsById().get(segment.value);
                  const kind = attachment
                    ? attachmentKind(attachment)
                    : segment.type;
                  return attachment ? (
                    <span
                      class={classNames(
                        "composer-attachment-token",
                        kind === "image" && "composer-image-token",
                        kind === "file" && "composer-file-token",
                      )}
                      contentEditable={false}
                      data-attachment-id={attachment.id}
                      data-attachment-kind={kind}
                      data-image-id={
                        kind === "image" ? attachment.id : undefined
                      }
                      title={composerAttachmentToken(attachment)}
                      onContextMenu={(event) =>
                        openAttachmentMenu(event, attachment)
                      }
                      onPointerDown={(event) =>
                        beginAttachmentPress(event, attachment)
                      }
                      onPointerUp={cancelAttachmentPress}
                      onPointerLeave={cancelAttachmentPress}
                    >
                      <button
                        type="button"
                        onClick={() => viewAttachment(attachment)}
                      >
                        <Show
                          when={kind === "image"}
                          fallback={<FileText size={14} strokeWidth={1.7} />}
                        >
                          <img src={attachment.dataUrl} alt="" />
                        </Show>
                        <span>{attachment.name}</span>
                      </button>
                      <button
                        type="button"
                        title={t("remove")}
                        onClick={() => removeAttachment(attachment.id)}
                      >
                        ×
                      </button>
                    </span>
                  ) : (
                    <span>{composerToken(segment.type, segment.value)}</span>
                  );
                })()}
              </Show>
            )}
          </For>
        </div>
        <textarea
          ref={textarea}
          class="composer-raw-textarea"
          value={props.text}
          rows={3}
          style={{ height: composerInputHeight(props.text) }}
          onInput={(event) => props.onText(event.currentTarget.value)}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
              event.preventDefault();
              void props.onSubmit();
            }
          }}
          placeholder={t("writeMessage")}
        />
      </div>
      <div class="composer-toolbar">
        <button
          class="composer-attach"
          type="button"
          title={t("attachFile")}
          onClick={() => fileInput?.click()}
        >
          <Plus size={18} strokeWidth={1.7} />
        </button>
        <input
          ref={fileInput}
          class="composer-file-input"
          type="file"
          multiple
          tabIndex={-1}
          onChange={(event) => void attachFiles(event.currentTarget.files)}
        />
        <div class="composer-settings">{props.toolbar}</div>
        <button
          class="composer-send"
          type="button"
          title={t("send")}
          disabled={
            props.submitting ||
            props.submitDisabled ||
            (!props.text.trim() && props.images.length === 0)
          }
          onClick={props.onSubmit}
        >
          <ArrowUp size={16} strokeWidth={1.8} />
        </button>
      </div>
      <Show when={previewImageId() !== undefined}>
        <ImageLightbox
          paths={imagePaths()}
          index={previewImageIndex()}
          onIndex={(index) =>
            setPreviewImageId(
              props.images.filter((image) => attachmentKind(image) === "image")[
                index
              ]?.id,
            )
          }
          onClose={() => setPreviewImageId(undefined)}
        />
      </Show>
      <Show when={attachmentMenu()}>
        {(menu) => {
          const attachment = () => attachmentsById().get(menu().id);
          return (
            <div
              class="composer-attachment-menu"
              style={{
                left: `${menu().x}px`,
                top: `${menu().y}px`,
              }}
              onPointerDown={(event) => event.stopPropagation()}
            >
              <button
                type="button"
                onClick={() => {
                  const current = attachment();
                  if (current) {
                    viewAttachment(current);
                  }
                }}
              >
                <ExternalLink size={14} strokeWidth={1.7} />
                <span>{t("viewFile")}</span>
              </button>
              <button
                type="button"
                onClick={() => {
                  const current = attachment();
                  if (current) {
                    openAttachmentLocation(current);
                  }
                }}
              >
                <FolderOpen size={14} strokeWidth={1.7} />
                <span>{t("openFileLocation")}</span>
              </button>
            </div>
          );
        }}
      </Show>
    </footer>
  );
}

type ComposerPreviewSegment =
  | { type: "text"; value: string }
  | { type: "image"; value: string }
  | { type: "file"; value: string };

const COMPOSER_ATTACHMENT_TOKEN_PATTERN =
  /\[\[(image|file):([a-zA-Z0-9_-]+)\]\]/gu;

export function composerImageToken(id: string): string {
  return `[[image:${id}]]`;
}

export function composerFileToken(id: string): string {
  return `[[file:${id}]]`;
}

export function composerPreviewSegments(
  text: string,
): ComposerPreviewSegment[] {
  const segments: ComposerPreviewSegment[] = [];
  let cursor = 0;
  for (const match of text.matchAll(COMPOSER_ATTACHMENT_TOKEN_PATTERN)) {
    if (match.index > cursor) {
      segments.push({ type: "text", value: text.slice(cursor, match.index) });
    }
    segments.push({
      type: match[1] === "file" ? "file" : "image",
      value: match[2] ?? "",
    });
    cursor = match.index + match[0].length;
  }
  if (cursor < text.length) {
    segments.push({ type: "text", value: text.slice(cursor) });
  }
  return segments.length > 0 ? segments : [{ type: "text", value: text }];
}

export function removeComposerImageToken(text: string, id: string): string {
  return removeComposerAttachmentToken(text, id);
}

export function removeComposerAttachmentToken(
  text: string,
  id: string,
): string {
  return text
    .replace(
      new RegExp(
        `\\n?\\[\\[(?:image|file):${escapeRegExp(id)}\\]\\]\\n?`,
        "gu",
      ),
      "\n",
    )
    .replace(/\n{3,}/gu, "\n\n");
}

function composerAttachmentToken(attachment: ComposerImage): string {
  return attachmentKind(attachment) === "image"
    ? composerImageToken(attachment.id)
    : composerFileToken(attachment.id);
}

function composerToken(
  type: ComposerPreviewSegment["type"],
  id: string,
): string {
  return type === "file" ? composerFileToken(id) : composerImageToken(id);
}

function composerTokenForElement(element: HTMLElement): string {
  const id = element.dataset.attachmentId ?? "";
  return element.dataset.attachmentKind === "file"
    ? composerFileToken(id)
    : composerImageToken(id);
}

function attachmentKind(attachment: ComposerImage): "image" | "file" {
  return (
    attachment.kind ??
    (attachment.mimeType?.startsWith("image/") ? "image" : "image")
  );
}

function readImageDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.onerror = () =>
      reject(reader.error ?? new Error("Failed to read image"));
    reader.readAsDataURL(file);
  });
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function composerInputHeight(value: string): string {
  const lines = Math.min(
    8,
    Math.max(
      3,
      value.split(/\r\n|\r|\n/u).length + Math.floor(value.length / 88),
    ),
  );
  return `${lines * 22 + 18}px`;
}

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
