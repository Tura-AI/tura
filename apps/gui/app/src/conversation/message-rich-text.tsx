import { GatewayClient } from "@tura/gateway-sdk";
import ChevronLeft from "lucide-solid/icons/chevron-left";
import ChevronRight from "lucide-solid/icons/chevron-right";
import Crop from "lucide-solid/icons/crop";
import Maximize2 from "lucide-solid/icons/maximize-2";
import Minimize2 from "lucide-solid/icons/minimize-2";
import Pencil from "lucide-solid/icons/pencil";
import RotateCw from "lucide-solid/icons/rotate-cw";
import X from "lucide-solid/icons/x";
import { For, Match, Show, Switch, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { Portal } from "solid-js/web";
import { t } from "../i18n";
import { classNames } from "../state/format";
import { openExternalUrl } from "../utils/external-url";
import { normalizeEnglishPunctuation } from "./message-punctuation";
import { RICH_TOKEN_PATTERN } from "./message-rich-protocol";
import { mediaSource } from "./message-rich-text-paths";

export {
  reactionEmojiValues,
  stickerEmojiValues,
  stripReactionEmoji,
} from "./message-rich-protocol";

type RichNode =
  | { kind: "text"; text: string }
  | {
      kind: "element";
      tag: RichTag;
      href?: string;
      language?: string;
      children: RichNode[];
    }
  | { kind: "media"; path: string }
  | { kind: "emoji"; variant: "sticker" | "react"; value: string }
  | { kind: "table"; caption: RichNode[]; rows: RichTableRow[] };

type RichTableCell = {
  kind: "header" | "data";
  children: RichNode[];
  colSpan?: number;
  rowSpan?: number;
};

type RichTableRow = {
  cells: RichTableCell[];
};

type RichGroup = { kind: "node"; node: RichNode } | { kind: "gallery"; paths: string[] };

const TABLE_CELL_MAX_CH = 96;

type RichTag =
  | "bold"
  | "italic"
  | "underline"
  | "strike"
  | "link"
  | "code"
  | "spoiler"
  | "thinkingGlyph"
  | "blockquote"
  | "pre";

export function RichText(props: {
  text: string;
  active?: boolean;
  workspaceDirectory?: string;
  gatewayUrl?: string;
  normalizePunctuation?: boolean;
}) {
  const nodes = createMemo(() => {
    const parsed = parseRichText(props.text);
    return props.normalizePunctuation ? normalizeRichTextNodes(parsed) : parsed;
  });
  const streamingText = createMemo(() =>
    props.normalizePunctuation ? normalizeEnglishPunctuation(props.text) : props.text,
  );
  const groups = createMemo(() => groupMediaNodes(nodes()));
  const [viewerIndex, setViewerIndex] = createSignal<number>();
  const galleryPaths = createMemo(() =>
    groups()
      .flatMap((group) => (group.kind === "gallery" ? group.paths : []))
      .filter((path) => isImagePath(path)),
  );
  const renderStablePlainText = createMemo(
    () => Boolean(props.active) && isPlainStreamingText(props.text),
  );
  return (
    <div class={classNames("rich-text", props.active && "typing-text")}>
      <Show
        when={renderStablePlainText()}
        fallback={
          <>
            <For each={groups()}>
              {(group) => (
                <Show
                  when={group.kind === "gallery"}
                  fallback={
                    <RichNodeView
                      node={(group as Extract<RichGroup, { kind: "node" }>).node}
                      workspaceDirectory={props.workspaceDirectory}
                      gatewayUrl={props.gatewayUrl}
                    />
                  }
                >
                  <MediaGallery
                    paths={(group as Extract<RichGroup, { kind: "gallery" }>).paths}
                    workspaceDirectory={props.workspaceDirectory}
                    gatewayUrl={props.gatewayUrl}
                    onOpen={(path) => setViewerIndex(galleryPaths().indexOf(path))}
                  />
                </Show>
              )}
            </For>
            <Show when={viewerIndex() !== undefined}>
              <Portal>
                <ImageLightbox
                  paths={galleryPaths()}
                  index={viewerIndex() ?? 0}
                  workspaceDirectory={props.workspaceDirectory}
                  gatewayUrl={props.gatewayUrl}
                  onIndex={setViewerIndex}
                  onClose={() => setViewerIndex(undefined)}
                />
              </Portal>
            </Show>
          </>
        }
      >
        {streamingText()}
      </Show>
    </div>
  );
}

function normalizeRichTextNodes(nodes: RichNode[]): RichNode[] {
  return nodes.map((node) => {
    if (node.kind === "text") {
      return { ...node, text: normalizeEnglishPunctuation(node.text) };
    }
    if (node.kind === "element") {
      return node.tag === "code" || node.tag === "pre"
        ? node
        : { ...node, children: normalizeRichTextNodes(node.children) };
    }
    if (node.kind === "table") {
      return {
        ...node,
        caption: normalizeRichTextNodes(node.caption),
        rows: node.rows.map((row) => ({
          cells: row.cells.map((cell) => ({
            ...cell,
            children: normalizeRichTextNodes(cell.children),
          })),
        })),
      };
    }
    return node;
  });
}

function isPlainStreamingText(text: string): boolean {
  RICH_TOKEN_PATTERN.lastIndex = 0;
  if (RICH_TOKEN_PATTERN.test(text)) {
    RICH_TOKEN_PATTERN.lastIndex = 0;
    return false;
  }
  RICH_TOKEN_PATTERN.lastIndex = 0;
  return !/<\/?[A-Za-z][\s\S]*?>/u.test(text);
}

function RichNodeView(props: { node: RichNode; workspaceDirectory?: string; gatewayUrl?: string }) {
  if (props.node.kind === "text") {
    return <>{props.node.text}</>;
  }
  if (props.node.kind === "media") {
    return (
      <MediaNode
        path={props.node.path}
        workspaceDirectory={props.workspaceDirectory}
        gatewayUrl={props.gatewayUrl}
      />
    );
  }
  if (props.node.kind === "emoji") {
    return <span class={`rich-emoji rich-${props.node.variant}`}>{props.node.value}</span>;
  }
  if (props.node.kind === "table") {
    return (
      <RichTableView
        caption={props.node.caption}
        rows={props.node.rows}
        workspaceDirectory={props.workspaceDirectory}
        gatewayUrl={props.gatewayUrl}
      />
    );
  }
  return (
    <RichElement
      node={props.node}
      workspaceDirectory={props.workspaceDirectory}
      gatewayUrl={props.gatewayUrl}
    />
  );
}

function RichTableView(props: {
  caption: RichNode[];
  rows: RichTableRow[];
  workspaceDirectory?: string;
  gatewayUrl?: string;
}) {
  const caption = createMemo(() => plainText(props.caption).trim());
  const [scrollWidth, setScrollWidth] = createSignal(0);
  const [clientWidth, setClientWidth] = createSignal(0);
  const [scrollLeft, setScrollLeft] = createSignal(0);
  let tableScroll: HTMLDivElement | undefined;
  let xTrack: HTMLDivElement | undefined;

  const hasXOverflow = createMemo(() => scrollWidth() > clientWidth() + 1 && clientWidth() > 0);
  const xThumbPercent = createMemo(() =>
    scrollWidth() > 0 ? Math.max(4, (clientWidth() / scrollWidth()) * 100) : 0,
  );
  const xThumbOffset = createMemo(() => {
    const maxScroll = Math.max(1, scrollWidth() - clientWidth());
    return (scrollLeft() / maxScroll) * (100 - xThumbPercent());
  });

  onMount(() => {
    updateScrollMetrics();
    requestAnimationFrame(updateScrollMetrics);
    const observer = new ResizeObserver(updateScrollMetrics);
    if (tableScroll) {
      observer.observe(tableScroll);
    }
    window.addEventListener("resize", updateScrollMetrics);
    onCleanup(() => {
      observer.disconnect();
      window.removeEventListener("resize", updateScrollMetrics);
    });
  });

  function updateScrollMetrics() {
    setScrollWidth(tableScroll?.scrollWidth ?? 0);
    setClientWidth(tableScroll?.clientWidth ?? 0);
    setScrollLeft(tableScroll?.scrollLeft ?? 0);
  }

  function setHorizontalScroll(event: PointerEvent) {
    if (!tableScroll || !xTrack) {
      return;
    }
    const rect = xTrack.getBoundingClientRect();
    const thumbWidth = (xThumbPercent() / 100) * rect.width;
    const maxOffset = Math.max(1, rect.width - thumbWidth);
    const offset = Math.min(maxOffset, Math.max(0, event.clientX - rect.left - thumbWidth / 2));
    tableScroll.scrollLeft =
      (offset / maxOffset) * (tableScroll.scrollWidth - tableScroll.clientWidth);
    updateScrollMetrics();
  }

  function dragScroll(event: PointerEvent, setter: (event: PointerEvent) => void) {
    event.preventDefault();
    setter(event);
    const move = (moveEvent: PointerEvent) => setter(moveEvent);
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
  }

  return (
    <figure class="rich-table-frame">
      <div
        ref={(element) => {
          tableScroll = element;
        }}
        class="rich-table-scroll"
        tabindex="0"
        onScroll={updateScrollMetrics}
      >
        <table>
          <Show when={caption()}>
            <caption>{caption()}</caption>
          </Show>
          <tbody>
            <For each={props.rows}>
              {(row) => (
                <tr>
                  <For each={row.cells}>
                    {(cell) => {
                      const content = () => (
                        <For each={cell.children}>
                          {(node) => (
                            <RichNodeView
                              node={node}
                              workspaceDirectory={props.workspaceDirectory}
                              gatewayUrl={props.gatewayUrl}
                            />
                          )}
                        </For>
                      );
                      return (
                        <Show
                          when={cell.kind === "header"}
                          fallback={
                            <td
                              colSpan={cell.colSpan}
                              rowSpan={cell.rowSpan}
                              style={tableCellWidthStyle(cell)}
                            >
                              <span class="rich-table-cell-content">{content()}</span>
                            </td>
                          }
                        >
                          <th
                            colSpan={cell.colSpan}
                            rowSpan={cell.rowSpan}
                            style={tableCellWidthStyle(cell)}
                          >
                            <span class="rich-table-cell-content">{content()}</span>
                          </th>
                        </Show>
                      );
                    }}
                  </For>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </div>
      <Show when={hasXOverflow()}>
        <div
          ref={(element) => {
            xTrack = element;
          }}
          class="rich-table-overflow-bar rich-table-overflow-x"
          aria-hidden="true"
          onPointerDown={(event) => dragScroll(event, setHorizontalScroll)}
        >
          <div
            style={{
              width: `${xThumbPercent()}%`,
              left: `${xThumbOffset()}%`,
            }}
          />
        </div>
      </Show>
    </figure>
  );
}

function tableCellWidthStyle(cell: RichTableCell): Record<string, string> {
  const textLength = plainText(cell.children).trim().length;
  const widthCh = Math.min(TABLE_CELL_MAX_CH, Math.ceil(textLength / 3));
  return { "--rich-table-cell-width": `${widthCh}ch` };
}

function RichElement(props: {
  node: Extract<RichNode, { kind: "element" }>;
  workspaceDirectory?: string;
  gatewayUrl?: string;
}) {
  const children = () => (
    <For each={props.node.children}>
      {(node) => (
        <RichNodeView
          node={node}
          workspaceDirectory={props.workspaceDirectory}
          gatewayUrl={props.gatewayUrl}
        />
      )}
    </For>
  );
  return (
    <Switch fallback={<span>{children()}</span>}>
      <Match when={props.node.tag === "bold"}>
        <b>{children()}</b>
      </Match>
      <Match when={props.node.tag === "italic"}>
        <i>{children()}</i>
      </Match>
      <Match when={props.node.tag === "underline"}>
        <u>{children()}</u>
      </Match>
      <Match when={props.node.tag === "strike"}>
        <s>{children()}</s>
      </Match>
      <Match when={props.node.tag === "link"}>
        <a
          href={props.node.href}
          target="_blank"
          rel="noreferrer"
          onClick={(event) => {
            event.preventDefault();
            if (props.node.href) {
              void openExternalUrl(props.node.href);
            }
          }}
        >
          {children()}
        </a>
      </Match>
      <Match when={props.node.tag === "code"}>
        <code>{children()}</code>
      </Match>
      <Match when={props.node.tag === "spoiler"}>
        <span class="rich-spoiler">{children()}</span>
      </Match>
      <Match when={props.node.tag === "thinkingGlyph"}>
        <span class="assistant-thinking-glyph">{children()}</span>
      </Match>
      <Match when={props.node.tag === "blockquote"}>
        <blockquote>{children()}</blockquote>
      </Match>
      <Match when={props.node.tag === "pre"}>
        <pre class={props.node.language ? `language-${props.node.language}` : ""}>
          <code>{plainText(props.node.children)}</code>
        </pre>
      </Match>
    </Switch>
  );
}

function MediaNode(props: { path: string; workspaceDirectory?: string; gatewayUrl?: string }) {
  const isImage = createMemo(() => isImagePath(props.path));
  const [failed, setFailed] = createSignal(false);
  return (
    <figure class="rich-media">
      <Show
        when={isImage() && !failed()}
        fallback={
          <FileMediaTile
            path={props.path}
            workspaceDirectory={props.workspaceDirectory}
            gatewayUrl={props.gatewayUrl}
          />
        }
      >
        <div class="rich-media-thumb" title={props.path}>
          <img
            src={mediaSource(props.path, props.workspaceDirectory, props.gatewayUrl)}
            alt=""
            loading="lazy"
            onError={() => setFailed(true)}
          />
        </div>
      </Show>
      <figcaption>{props.path}</figcaption>
    </figure>
  );
}

function MediaGallery(props: {
  paths: string[];
  workspaceDirectory?: string;
  gatewayUrl?: string;
  onOpen: (path: string) => void;
}) {
  return (
    <div class="rich-gallery grid">
      <For each={props.paths}>
        {(path) => (
          <GalleryMediaItem
            path={path}
            workspaceDirectory={props.workspaceDirectory}
            gatewayUrl={props.gatewayUrl}
            onOpen={props.onOpen}
          />
        )}
      </For>
    </div>
  );
}

function GalleryMediaItem(props: {
  path: string;
  workspaceDirectory?: string;
  gatewayUrl?: string;
  onOpen: (path: string) => void;
}) {
  const [failed, setFailed] = createSignal(false);
  const isImage = createMemo(() => isImagePath(props.path));
  return (
    <Show
      when={isImage() && !failed()}
      fallback={
        <FileMediaTile
          path={props.path}
          workspaceDirectory={props.workspaceDirectory}
          gatewayUrl={props.gatewayUrl}
        />
      }
    >
      <button
        type="button"
        class="rich-gallery-item"
        onClick={() => props.onOpen(props.path)}
        title={props.path}
      >
        <img
          src={mediaSource(props.path, props.workspaceDirectory, props.gatewayUrl)}
          alt=""
          loading="lazy"
          onError={() => setFailed(true)}
        />
      </button>
    </Show>
  );
}

function FileMediaTile(props: { path: string; workspaceDirectory?: string; gatewayUrl?: string }) {
  return (
    <button
      type="button"
      class="rich-file-tile"
      title={props.path}
      onClick={() => void openMediaFile(props.path, props.workspaceDirectory, props.gatewayUrl)}
    >
      <span class="rich-file-name">{fileName(props.path)}</span>
      <span class="rich-file-ext">{fileExtension(props.path) || t("open")}</span>
    </button>
  );
}

async function openMediaFile(path: string, workspaceDirectory?: string, gatewayUrl?: string) {
  await new GatewayClient({ baseUrl: gatewayUrl, directory: workspaceDirectory }).openFile(path);
}

export function ImageLightbox(props: {
  paths: string[];
  index: number;
  workspaceDirectory?: string;
  gatewayUrl?: string;
  onIndex: (index: number) => void;
  onClose: () => void;
}) {
  const [scale, setScale] = createSignal(1);
  const [fill, setFill] = createSignal(false);
  const [rotation, setRotation] = createSignal(0);
  const currentPath = createMemo(() => props.paths[props.index] ?? "");

  function move(delta: number) {
    const count = props.paths.length;
    if (count === 0) {
      return;
    }
    props.onIndex((props.index + delta + count) % count);
    setScale(1);
    setRotation(0);
  }

  function handleWheel(event: WheelEvent) {
    event.preventDefault();
    if (event.ctrlKey || event.metaKey) {
      setScale((value) => Math.min(4, Math.max(0.35, value + (event.deltaY < 0 ? 0.12 : -0.12))));
      return;
    }
    move(event.deltaY > 0 ? 1 : -1);
  }

  return (
    <div class="media-lightbox" onWheel={handleWheel}>
      <div class="media-window-actions">
        <button type="button" title={t("minimize")} onClick={props.onClose}>
          <Minimize2 size={18} strokeWidth={1.7} />
        </button>
        <button type="button" title={t("fullscreen")} onClick={() => setFill(!fill())}>
          <Maximize2 size={18} strokeWidth={1.7} />
        </button>
        <button type="button" title={t("close")} onClick={props.onClose}>
          <X size={18} strokeWidth={1.7} />
        </button>
      </div>
      <button
        class="media-edge media-edge-left"
        type="button"
        onClick={() => move(-1)}
        title={t("previous")}
      >
        <ChevronLeft size={30} strokeWidth={1.5} />
      </button>
      <button
        class="media-edge media-edge-right"
        type="button"
        onClick={() => move(1)}
        title={t("next")}
      >
        <ChevronRight size={30} strokeWidth={1.5} />
      </button>
      <img
        class={classNames("media-lightbox-image", fill() && "fill")}
        src={mediaSource(currentPath(), props.workspaceDirectory, props.gatewayUrl)}
        alt=""
        style={{
          transform: `scale(${scale()}) rotate(${rotation()}deg)`,
        }}
      />
      <div class="media-tool-actions">
        <button type="button" title={t("crop")}>
          <Crop size={18} strokeWidth={1.7} />
        </button>
        <button
          type="button"
          title={t("rotate")}
          onClick={() => setRotation((value) => value + 90)}
        >
          <RotateCw size={18} strokeWidth={1.7} />
        </button>
        <button type="button" title={t("draw")}>
          <Pencil size={18} strokeWidth={1.7} />
        </button>
      </div>
    </div>
  );
}

export function parseRichText(source: string): RichNode[] {
  if (!source) {
    return [];
  }
  const nodes: RichNode[] = [];
  let cursor = 0;
  for (const match of source.matchAll(RICH_TOKEN_PATTERN)) {
    if (match.index > cursor) {
      nodes.push(...parseHtmlFragment(source.slice(cursor, match.index)));
    }
    if (match[1] === "MEDIA") {
      nodes.push({ kind: "media", path: (match[2] ?? "").trim() });
    } else if (match[3] === "sticker" || match[3] === "react") {
      const value = (match[4] ?? "").trim();
      if (value) {
        nodes.push({ kind: "emoji", variant: match[3], value });
      }
    }
    cursor = match.index + match[0].length;
  }
  if (cursor < source.length) {
    nodes.push(...parseHtmlFragment(source.slice(cursor)));
  }
  return compactTextNodes(nodes);
}

function parseHtmlFragment(source: string): RichNode[] {
  const markdownTableNodes = parseMarkdownTables(source);
  if (markdownTableNodes) {
    return markdownTableNodes;
  }
  return parseInlineRichText(source);
}

function parseInlineRichText(source: string): RichNode[] {
  const normalized = preserveUnknownAngleBrackets(normalizeHtmlBlockBreaks(source));
  if (typeof DOMParser === "undefined") {
    return normalized ? [{ kind: "text", text: normalized }] : [];
  }
  const document = new DOMParser().parseFromString(normalized, "text/html");
  return Array.from(document.body.childNodes).flatMap(readDomNode);
}

function normalizeHtmlBlockBreaks(source: string): string {
  return source
    .replace(/<br\s*\/?>/giu, "\n")
    .replace(/<li(?:\s[^>]*)?>/giu, "\n")
    .replace(/<\/li>/giu, "\n")
    .replace(
      /<\/?(?:address|article|aside|details|div|figcaption|figure|footer|h[1-6]|header|hr|main|nav|ol|p|section|summary|ul)(?:\s[^>]*)?>/giu,
      "\n",
    );
}

const SUPPORTED_HTML_TAGS = new Set([
  "a",
  "address",
  "article",
  "aside",
  "b",
  "blockquote",
  "br",
  "caption",
  "code",
  "del",
  "details",
  "div",
  "em",
  "figcaption",
  "figure",
  "footer",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "header",
  "hr",
  "i",
  "li",
  "main",
  "nav",
  "ol",
  "p",
  "pre",
  "s",
  "section",
  "span",
  "strong",
  "summary",
  "table",
  "tbody",
  "td",
  "tfoot",
  "th",
  "thead",
  "tr",
  "u",
  "ul",
]);

function preserveUnknownAngleBrackets(source: string): string {
  return source.replace(/<\/?([A-Za-z][A-Za-z0-9_-]*)(?:\s[^<>]*)?\/?>/gu, (match, tagName) =>
    SUPPORTED_HTML_TAGS.has(String(tagName).toLowerCase()) ? match : escapeHtml(match),
  );
}

function parseMarkdownTables(source: string): RichNode[] | undefined {
  const lines = source.split("\n");
  const nodes: RichNode[] = [];
  let textLines: string[] = [];
  let foundTable = false;

  function flushText() {
    if (textLines.length === 0) {
      return;
    }
    const text = textLines.join("\n");
    nodes.push(...parseInlineRichText(text));
    textLines = [];
  }

  for (let index = 0; index < lines.length; index += 1) {
    const header = markdownTableCells(lines[index]);
    const separator = markdownTableCells(lines[index + 1] ?? "");
    if (
      header.length >= 2 &&
      separator.length === header.length &&
      separator.every(isMarkdownTableSeparator)
    ) {
      flushText();
      const rows: RichTableRow[] = [
        {
          cells: header.map((cell) => ({
            kind: "header",
            children: parseInlineRichText(cell.trim()),
          })),
        },
      ];
      index += 2;
      while (index < lines.length) {
        const cells = markdownTableCells(lines[index]);
        if (cells.length === 0) {
          break;
        }
        rows.push({
          cells: normalizeMarkdownCells(cells, header.length).map((cell) => ({
            kind: "data",
            children: parseInlineRichText(cell.trim()),
          })),
        });
        index += 1;
      }
      nodes.push({ kind: "table", caption: [], rows });
      foundTable = true;
      index -= 1;
      continue;
    }
    textLines.push(lines[index]);
  }
  flushText();
  return foundTable ? compactTextNodes(nodes) : undefined;
}

function markdownTableCells(line: string): string[] {
  const trimmed = line.trim();
  if (!trimmed.includes("|")) {
    return [];
  }
  const body = trimmed.replace(/^\|/u, "").replace(/\|$/u, "");
  return body.split("|").map((cell) => cell.trim());
}

function normalizeMarkdownCells(cells: string[], count: number): string[] {
  if (cells.length >= count) {
    return cells.slice(0, count);
  }
  return [...cells, ...Array.from({ length: count - cells.length }, () => "")];
}

function isMarkdownTableSeparator(value: string): boolean {
  return /^:?-{3,}:?$/u.test(value.trim());
}

function readDomNode(node: Node): RichNode[] {
  if (node.nodeType === Node.TEXT_NODE) {
    return [{ kind: "text", text: node.textContent ?? "" }];
  }
  if (node.nodeType !== Node.ELEMENT_NODE) {
    return [];
  }
  const element = node as Element;
  const children = compactTextNodes(Array.from(element.childNodes).flatMap(readDomNode));
  const tagName = element.tagName.toLowerCase();
  switch (tagName) {
    case "table":
      return [readTableElement(element)];
    case "b":
    case "strong":
      return [{ kind: "element", tag: "bold", children }];
    case "i":
    case "em":
      return [{ kind: "element", tag: "italic", children }];
    case "u":
      return [{ kind: "element", tag: "underline", children }];
    case "s":
    case "del":
      return [{ kind: "element", tag: "strike", children }];
    case "a": {
      const href = element.getAttribute("href") ?? "";
      return isSafeUrl(href)
        ? [
            {
              kind: "element",
              tag: "link",
              href,
              children: [{ kind: "text", text: element.textContent ?? "" }],
            },
          ]
        : children;
    }
    case "code":
      return [
        {
          kind: "element",
          tag: "code",
          children: [{ kind: "text", text: element.textContent ?? "" }],
        },
      ];
    case "span":
      if (element.classList.contains("tg-spoiler")) {
        return [{ kind: "element", tag: "spoiler", children }];
      }
      return element.classList.contains("assistant-thinking-glyph")
        ? [{ kind: "element", tag: "thinkingGlyph", children }]
        : children;
    case "blockquote":
      return [{ kind: "element", tag: "blockquote", children }];
    case "pre": {
      const code = element.querySelector("code");
      const language = code?.className.match(/language-([A-Za-z0-9_-]+)/u)?.[1] ?? undefined;
      const text = code?.textContent ?? element.textContent ?? "";
      return [
        {
          kind: "element",
          tag: "pre",
          language,
          children: [{ kind: "text", text }],
        },
      ];
    }
    case "br":
      return [{ kind: "text", text: "\n" }];
    default:
      return children;
  }
}

function readTableElement(element: Element): RichNode {
  const caption = element.querySelector(":scope > caption");
  const rows = Array.from(element.querySelectorAll("tr")).map((row) => ({
    cells: Array.from(row.children)
      .filter((cell) => {
        const tag = cell.tagName.toLowerCase();
        return tag === "th" || tag === "td";
      })
      .map((cell) => {
        const kind: RichTableCell["kind"] = cell.tagName.toLowerCase() === "th" ? "header" : "data";
        return {
          kind,
          children: compactTextNodes(Array.from(cell.childNodes).flatMap(readDomNode)),
          colSpan: numericSpan(cell.getAttribute("colspan")),
          rowSpan: numericSpan(cell.getAttribute("rowspan")),
        };
      }),
  }));
  return {
    kind: "table",
    caption: caption ? compactTextNodes(Array.from(caption.childNodes).flatMap(readDomNode)) : [],
    rows: rows.filter((row) => row.cells.length > 0),
  };
}

function numericSpan(value: string | null): number | undefined {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 1 && parsed <= 24 ? parsed : undefined;
}

function compactTextNodes(nodes: RichNode[]): RichNode[] {
  const compacted: RichNode[] = [];
  for (const node of nodes) {
    const previous = compacted.at(-1);
    if (node.kind === "text" && previous?.kind === "text") {
      previous.text += node.text;
    } else {
      compacted.push(node);
    }
  }
  return compacted.filter((node) => node.kind !== "text" || node.text.length > 0);
}

function plainText(nodes: RichNode[]): string {
  return nodes
    .map((node) => {
      if (node.kind === "text") {
        return node.text;
      }
      if (node.kind === "element") {
        return plainText(node.children);
      }
      if (node.kind === "table") {
        return [
          plainText(node.caption),
          ...node.rows.map((row) => row.cells.map((cell) => plainText(cell.children)).join("\t")),
        ]
          .filter(Boolean)
          .join("\n");
      }
      if (node.kind === "media") {
        return `[MEDIA:${node.path}:MEDIA]`;
      }
      return node.value;
    })
    .join("");
}

function isImagePath(path: string): boolean {
  return /^data:image\//iu.test(path) || /\.(png|jpe?g|gif|webp|svg)$/iu.test(path);
}

function fileName(path: string): string {
  const normalized = path.replace(/\\/gu, "/").replace(/[?#].*$/u, "");
  return decodeURIComponent(normalized.split("/").filter(Boolean).at(-1) || path);
}

function fileExtension(path: string): string {
  const name = fileName(path);
  const match = name.match(/\.([A-Za-z0-9]+)$/u);
  return match?.[1]?.toLowerCase() ?? "";
}

function groupMediaNodes(nodes: RichNode[]): RichGroup[] {
  const groups: RichGroup[] = [];
  let media: string[] = [];

  function flushMedia() {
    if (media.length > 0) {
      groups.push({ kind: "gallery", paths: media });
      media = [];
    }
  }

  for (const node of nodes) {
    if (node.kind === "media") {
      media.push(node.path);
      continue;
    }
    flushMedia();
    groups.push({ kind: "node", node });
  }
  flushMedia();
  return groups;
}

function isSafeUrl(value: string): boolean {
  return /^https?:\/\//iu.test(value);
}

function escapeHtml(value: string): string {
  return value.replace(/&/gu, "&amp;").replace(/</gu, "&lt;").replace(/>/gu, "&gt;");
}
