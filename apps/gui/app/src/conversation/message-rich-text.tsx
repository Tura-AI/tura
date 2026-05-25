import { For, Match, Show, Switch, createMemo, createSignal } from "solid-js";
import ChevronLeft from "lucide-solid/icons/chevron-left";
import ChevronRight from "lucide-solid/icons/chevron-right";
import Crop from "lucide-solid/icons/crop";
import Maximize2 from "lucide-solid/icons/maximize-2";
import Minimize2 from "lucide-solid/icons/minimize-2";
import Pencil from "lucide-solid/icons/pencil";
import RotateCw from "lucide-solid/icons/rotate-cw";
import X from "lucide-solid/icons/x";
import { classNames } from "../state/format";
import { t } from "../i18n";

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
  | { kind: "emoji"; mode: "sticker" | "react"; value: string };

type RichGroup =
  | { kind: "node"; node: RichNode }
  | { kind: "gallery"; paths: string[] };

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

const TOKEN_PATTERN =
  /\[(MEDIA):([\s\S]*?):MEDIA\]|\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu;

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
            fallback={
              <RichNodeView
                node={(group as Extract<RichGroup, { kind: "node" }>).node}
              />
            }
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
  if (props.node.kind === "emoji") {
    return (
      <span class={`rich-emoji rich-${props.node.mode}`}>
        {props.node.value}
      </span>
    );
  }
  return <RichElement node={props.node} />;
}

function RichElement(props: { node: Extract<RichNode, { kind: "element" }> }) {
  const children = () => (
    <For each={props.node.children}>
      {(node) => <RichNodeView node={node} />}
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
        <pre
          class={props.node.language ? `language-${props.node.language}` : ""}
        >
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

function MediaGallery(props: {
  paths: string[];
  onOpen: (path: string) => void;
}) {
  return (
    <div class="rich-gallery">
      <For each={props.paths.filter(isImagePath)}>
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

function ImageLightbox(props: {
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
      setScale((value) =>
        Math.min(4, Math.max(0.35, value + (event.deltaY < 0 ? 0.12 : -0.12))),
      );
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
        <button
          type="button"
          title={t("fullscreen")}
          onClick={() => setFill(!fill())}
        >
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

function parseHtmlFragment(source: string): RichNode[] {
  if (typeof DOMParser === "undefined") {
    return [{ kind: "text", text: source }];
  }
  const document = new DOMParser().parseFromString(source, "text/html");
  return Array.from(document.body.childNodes).flatMap(readDomNode);
}

function readDomNode(node: Node): RichNode[] {
  if (node.nodeType === Node.TEXT_NODE) {
    return [{ kind: "text", text: node.textContent ?? "" }];
  }
  if (node.nodeType !== Node.ELEMENT_NODE) {
    return [];
  }
  const element = node as Element;
  const children = compactTextNodes(
    Array.from(element.childNodes).flatMap(readDomNode),
  );
  const tagName = element.tagName.toLowerCase();
  switch (tagName) {
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
        ? [{ kind: "element", tag: "link", href, children }]
        : children;
    }
    case "code":
      return [{ kind: "element", tag: "code", children }];
    case "span":
      return element.classList.contains("tg-spoiler")
        ? [{ kind: "element", tag: "spoiler", children }]
        : children;
    case "blockquote":
      return [{ kind: "element", tag: "blockquote", children }];
    case "pre": {
      const code = element.querySelector("code");
      const language =
        code?.className.match(/language-([A-Za-z0-9_-]+)/u)?.[1] ?? undefined;
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
  return compacted.filter(
    (node) => node.kind !== "text" || node.text.length > 0,
  );
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
      return node.kind === "emoji" ? node.value : `[MEDIA:${node.path}:MEDIA]`;
    })
    .join("");
}

function mediaSource(path: string): string {
  if (/^(https?:|data:|\/)/iu.test(path)) {
    return path;
  }
  return `/assets/${path.replace(/^.*[\\/]/u, "")}`;
}

function isImagePath(path: string): boolean {
  return /\.(png|jpe?g|gif|webp|svg)$/iu.test(path);
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
