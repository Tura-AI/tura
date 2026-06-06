import ChevronLeft from "lucide-solid/icons/chevron-left";
import ChevronRight from "lucide-solid/icons/chevron-right";
import Crop from "lucide-solid/icons/crop";
import Maximize2 from "lucide-solid/icons/maximize-2";
import Minimize2 from "lucide-solid/icons/minimize-2";
import Pencil from "lucide-solid/icons/pencil";
import RotateCw from "lucide-solid/icons/rotate-cw";
import X from "lucide-solid/icons/x";
import { For, Match, Show, Switch, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { t } from "../i18n";
import { classNames } from "../state/format";

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
  | { kind: "local-path"; path: string }
  | { kind: "emoji"; mode: "sticker" | "react"; value: string }
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

type RichTag =
  | "bold"
  | "italic"
  | "underline"
  | "strike"
  | "link"
  | "code"
  | "spoiler"
  | "blockquote"
  | "pre";

const TOKEN_PATTERN = /\[(MEDIA):([\s\S]*?):MEDIA\]|\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu;

export function RichText(props: { text: string; active?: boolean }) {
  const nodes = createMemo(() => parseRichText(props.text));
  const groups = createMemo(() => groupMediaNodes(nodes()));
  const [viewerIndex, setViewerIndex] = createSignal<number>();
  const galleryPaths = createMemo(() =>
    groups()
      .flatMap((group) => (group.kind === "gallery" ? group.paths : []))
      .filter((path) => isImagePath(path)),
  );
  return (
    <div class={classNames("rich-text", props.active && "typing-text")}>
      <For each={groups()}>
        {(group) => (
          <Show
            when={group.kind === "gallery"}
            fallback={<RichNodeView node={(group as Extract<RichGroup, { kind: "node" }>).node} />}
          >
            <MediaGallery
              paths={(group as Extract<RichGroup, { kind: "gallery" }>).paths}
              onOpen={(path) => setViewerIndex(galleryPaths().indexOf(path))}
            />
          </Show>
        )}
      </For>
      <Show when={viewerIndex() !== undefined}>
        <ImageLightbox
          paths={galleryPaths()}
          index={viewerIndex() ?? 0}
          onIndex={setViewerIndex}
          onClose={() => setViewerIndex(undefined)}
        />
      </Show>
    </div>
  );
}

function RichNodeView(props: { node: RichNode }) {
  if (props.node.kind === "text") {
    return <>{props.node.text}</>;
  }
  if (props.node.kind === "media") {
    return <MediaNode path={props.node.path} />;
  }
  if (props.node.kind === "local-path") {
    return <LocalPathLink path={props.node.path} />;
  }
  if (props.node.kind === "emoji") {
    return <span class={`rich-emoji rich-${props.node.mode}`}>{props.node.value}</span>;
  }
  if (props.node.kind === "table") {
    return <RichTableView caption={props.node.caption} rows={props.node.rows} />;
  }
  return <RichElement node={props.node} />;
}

function RichTableView(props: { caption: RichNode[]; rows: RichTableRow[] }) {
  const caption = createMemo(() => plainText(props.caption).trim());
  const [scrollWidth, setScrollWidth] = createSignal(0);
  const [clientWidth, setClientWidth] = createSignal(0);
  const [scrollHeight, setScrollHeight] = createSignal(0);
  const [clientHeight, setClientHeight] = createSignal(0);
  const [scrollLeft, setScrollLeft] = createSignal(0);
  const [scrollTop, setScrollTop] = createSignal(0);
  let tableScroll: HTMLDivElement | undefined;
  let xTrack: HTMLDivElement | undefined;
  let yTrack: HTMLDivElement | undefined;

  const hasXOverflow = createMemo(() => scrollWidth() > clientWidth() + 1 && clientWidth() > 0);
  const hasYOverflow = createMemo(() => scrollHeight() > clientHeight() + 1 && clientHeight() > 0);
  const xThumbPercent = createMemo(() =>
    scrollWidth() > 0 ? Math.max(4, (clientWidth() / scrollWidth()) * 100) : 0,
  );
  const yThumbPercent = createMemo(() =>
    scrollHeight() > 0 ? Math.max(8, (clientHeight() / scrollHeight()) * 100) : 0,
  );
  const xThumbOffset = createMemo(() => {
    const maxScroll = Math.max(1, scrollWidth() - clientWidth());
    return (scrollLeft() / maxScroll) * (100 - xThumbPercent());
  });
  const yThumbOffset = createMemo(() => {
    const maxScroll = Math.max(1, scrollHeight() - clientHeight());
    return (scrollTop() / maxScroll) * (100 - yThumbPercent());
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
    setScrollHeight(tableScroll?.scrollHeight ?? 0);
    setClientHeight(tableScroll?.clientHeight ?? 0);
    setScrollLeft(tableScroll?.scrollLeft ?? 0);
    setScrollTop(tableScroll?.scrollTop ?? 0);
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

  function setVerticalScroll(event: PointerEvent) {
    if (!tableScroll || !yTrack) {
      return;
    }
    const rect = yTrack.getBoundingClientRect();
    const thumbHeight = (yThumbPercent() / 100) * rect.height;
    const maxOffset = Math.max(1, rect.height - thumbHeight);
    const offset = Math.min(maxOffset, Math.max(0, event.clientY - rect.top - thumbHeight / 2));
    tableScroll.scrollTop =
      (offset / maxOffset) * (tableScroll.scrollHeight - tableScroll.clientHeight);
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
      <div ref={tableScroll} class="rich-table-scroll" tabindex="0" onScroll={updateScrollMetrics}>
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
                        <For each={cell.children}>{(node) => <RichNodeView node={node} />}</For>
                      );
                      return (
                        <Show
                          when={cell.kind === "header"}
                          fallback={
                            <td colSpan={cell.colSpan} rowSpan={cell.rowSpan}>
                              {content()}
                            </td>
                          }
                        >
                          <th colSpan={cell.colSpan} rowSpan={cell.rowSpan}>
                            {content()}
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
      <Show when={hasYOverflow()}>
        <div
          ref={yTrack}
          class="rich-table-overflow-bar rich-table-overflow-y"
          aria-hidden="true"
          onPointerDown={(event) => dragScroll(event, setVerticalScroll)}
        >
          <div
            style={{
              height: `${yThumbPercent()}%`,
              top: `${yThumbOffset()}%`,
            }}
          />
        </div>
      </Show>
      <Show when={hasXOverflow()}>
        <div
          ref={xTrack}
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

function LocalPathLink(props: { path: string }) {
  const [opening, setOpening] = createSignal(false);
  async function openLocation(event: MouseEvent) {
    event.preventDefault();
    if (opening()) {
      return;
    }
    setOpening(true);
    try {
      const query = new URLSearchParams({ path: props.path });
      const response = await fetch(`${gatewayBaseUrl()}/file/open-location?${query.toString()}`, {
        method: "POST",
      });
      if (!response.ok) {
        throw new Error(await response.text());
      }
    } catch (error) {
      console.error("Failed to open local path location", error);
    } finally {
      setOpening(false);
    }
  }
  return (
    <button
      type="button"
      class="rich-local-path"
      title={props.path}
      disabled={opening()}
      onClick={openLocation}
    >
      {props.path}
    </button>
  );
}

function RichElement(props: { node: Extract<RichNode, { kind: "element" }> }) {
  const children = () => (
    <For each={props.node.children}>{(node) => <RichNodeView node={node} />}</For>
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
        <a href={props.node.href} target="_blank" rel="noreferrer">
          {children()}
        </a>
      </Match>
      <Match when={props.node.tag === "code"}>
        <code>{children()}</code>
      </Match>
      <Match when={props.node.tag === "spoiler"}>
        <span class="rich-spoiler">{children()}</span>
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

function MediaNode(props: { path: string }) {
  const isImage = createMemo(() => isImagePath(props.path));
  return (
    <figure class="rich-media">
      <Show when={isImage()} fallback={<code>[MEDIA:{props.path}:MEDIA]</code>}>
        <img src={mediaSource(props.path)} alt="" loading="lazy" />
      </Show>
      <figcaption>{props.path}</figcaption>
    </figure>
  );
}

function MediaGallery(props: { paths: string[]; onOpen: (path: string) => void }) {
  const imagePaths = createMemo(() => props.paths.filter(isImagePath));
  return (
    <div class="rich-gallery grid">
      <For each={imagePaths()}>
        {(path) => (
          <button
            type="button"
            class="rich-gallery-item"
            onClick={() => props.onOpen(path)}
            title={path}
          >
            <img src={mediaSource(path)} alt="" loading="lazy" />
          </button>
        )}
      </For>
    </div>
  );
}

export function ImageLightbox(props: {
  paths: string[];
  index: number;
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
        src={mediaSource(currentPath())}
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
  for (const match of source.matchAll(TOKEN_PATTERN)) {
    if (match.index > cursor) {
      nodes.push(...parseHtmlFragment(source.slice(cursor, match.index)));
    }
    if (match[1] === "MEDIA") {
      nodes.push({ kind: "media", path: (match[2] ?? "").trim() });
    } else {
      const mode = match[3] === "sticker" ? "sticker" : "react";
      nodes.push({ kind: "emoji", mode, value: (match[4] ?? "").trim() });
    }
    cursor = match.index + match[0].length;
  }
  if (cursor < source.length) {
    nodes.push(...parseHtmlFragment(source.slice(cursor)));
  }
  return compactTextNodes(nodes);
}

export function reactionEmojiValues(source: string): string[] {
  return Array.from(source.matchAll(TOKEN_PATTERN))
    .filter((match) => match[3] === "react")
    .map((match) => (match[4] ?? "").trim())
    .filter(Boolean);
}

export function stickerEmojiValues(source: string): string[] {
  return Array.from(source.matchAll(TOKEN_PATTERN))
    .filter((match) => match[3] === "sticker")
    .map((match) => (match[4] ?? "").trim())
    .filter(Boolean);
}

export function stripReactionEmoji(source: string): string {
  return source.replace(/\[EMOJI:react:[\s\S]*?:EMOJI\]/gu, "");
}

function parseHtmlFragment(source: string): RichNode[] {
  const markdownTableNodes = parseMarkdownTables(source);
  if (markdownTableNodes) {
    return markdownTableNodes;
  }
  if (typeof DOMParser === "undefined") {
    return [{ kind: "text", text: source }];
  }
  const document = new DOMParser().parseFromString(source, "text/html");
  return Array.from(document.body.childNodes).flatMap(readDomNode);
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
    nodes.push(
      ...(typeof DOMParser === "undefined"
        ? splitLocalPathText(text)
        : Array.from(new DOMParser().parseFromString(text, "text/html").body.childNodes).flatMap(
            readDomNode,
          )),
    );
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
            children: splitLocalPathText(cell.trim()),
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
            children: splitLocalPathText(cell.trim()),
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
    return splitLocalPathText(node.textContent ?? "");
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
      return element.classList.contains("tg-spoiler")
        ? [{ kind: "element", tag: "spoiler", children }]
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

const LOCAL_PATH_PATTERN =
  /(?:[A-Za-z]:[\\/][^\s<>"'`]+|\\\\[^\\/\s<>"'`]+\\[^\\/\s<>"'`]+(?:\\[^\s<>"'`]+)*|\/[A-Za-z0-9_.-]+(?:\/[A-Za-z0-9_.-]+)+|\.{1,2}[\\/][^\s<>"'`]+)/gu;
const TRAILING_PATH_PUNCTUATION = /[),.;:!?]+$/u;

function splitLocalPathText(text: string): RichNode[] {
  const nodes: RichNode[] = [];
  let cursor = 0;
  for (const match of text.matchAll(LOCAL_PATH_PATTERN)) {
    const raw = match[0];
    const index = match.index ?? 0;
    const path = raw.replace(TRAILING_PATH_PUNCTUATION, "");
    if (!path || !isLocalPath(path)) {
      continue;
    }
    if (index > cursor) {
      nodes.push({ kind: "text", text: text.slice(cursor, index) });
    }
    nodes.push({ kind: "local-path", path });
    const trailing = raw.slice(path.length);
    if (trailing) {
      nodes.push({ kind: "text", text: trailing });
    }
    cursor = index + raw.length;
  }
  if (cursor < text.length) {
    nodes.push({ kind: "text", text: text.slice(cursor) });
  }
  return nodes.length > 0 ? nodes : [{ kind: "text", text }];
}

function isLocalPath(value: string): boolean {
  return /^(?:[A-Za-z]:[\\/]|\\\\|\/|\.{1,2}[\\/])/u.test(value);
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
      if (node.kind === "emoji") {
        return node.value;
      }
      if (node.kind === "table") {
        return [
          plainText(node.caption),
          ...node.rows.map((row) => row.cells.map((cell) => plainText(cell.children)).join("\t")),
        ]
          .filter(Boolean)
          .join("\n");
      }
      return node.kind === "media" ? `[MEDIA:${node.path}:MEDIA]` : node.path;
    })
    .join("");
}

function mediaSource(path: string): string {
  if (/^(https?:|data:)/iu.test(path) || path.startsWith("/assets/")) {
    return path;
  }
  const gatewayUrl = gatewayBaseUrl();
  const query = new URLSearchParams({ path });
  return `${gatewayUrl}/file/media?${query.toString()}`;
}

function isImagePath(path: string): boolean {
  return /^data:image\//iu.test(path) || /\.(png|jpe?g|gif|webp|svg)$/iu.test(path);
}

function gatewayBaseUrl(): string {
  if (typeof window === "undefined") {
    return "";
  }
  const configured = new URLSearchParams(window.location.search).get("gatewayUrl")?.trim();
  if (configured) {
    return configured.replace(/\/+$/u, "");
  }
  return window.location.origin.replace(/\/+$/u, "");
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
    if (node.kind === "media" && isImagePath(node.path)) {
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
