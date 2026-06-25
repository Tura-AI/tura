import type { MessagePart } from "@tura/gateway-sdk";
import { Show, createMemo } from "solid-js";
import { jsonPreview } from "../state/format";
import { partText } from "../state/global-store";
import { stripReactionEmoji } from "./message-rich-protocol";
import { RichText } from "./message-rich-text";

export function previewUserTextParts(parts: MessagePart[], expanded: boolean) {
  if (expanded) {
    return { parts, truncated: false };
  }
  const maxLines = 6;
  const maxChars = 420;
  let remainingLines = maxLines;
  let remainingChars = maxChars;
  let truncated = false;
  const previewParts: MessagePart[] = [];

  for (const part of parts) {
    const text = partText(part);
    if (remainingLines <= 0 || remainingChars <= 0) {
      truncated = true;
      break;
    }
    const preview = previewUserText(text, remainingLines, remainingChars);
    if (preview.text) {
      previewParts.push({ ...part, text: preview.text, content: preview.text });
    }
    remainingLines -= preview.consumedLines;
    remainingChars -= preview.consumedChars;
    if (preview.truncated) {
      truncated = true;
      break;
    }
  }

  return {
    parts: truncated ? appendUserPreviewEllipsis(previewParts, parts) : parts,
    truncated,
  };
}

function previewUserText(
  text: string,
  maxLines: number,
  maxChars: number,
): {
  text: string;
  consumedLines: number;
  consumedChars: number;
  truncated: boolean;
} {
  const normalized = text.replace(/\r\n|\r/gu, "\n");
  const lines = normalized.split("\n");
  const selected = lines.slice(0, maxLines);
  let preview = selected.join("\n");
  let truncated = lines.length > maxLines;
  if (preview.length > maxChars) {
    preview = preview.slice(0, maxChars).trimEnd();
    truncated = true;
  }
  return {
    text: preview,
    consumedLines: Math.min(lines.length, maxLines),
    consumedChars: preview.length,
    truncated,
  };
}

const USER_MEDIA_TOKEN_PATTERN = /\[MEDIA:[\s\S]*?:MEDIA\]/gu;

function appendUserPreviewEllipsis(
  parts: MessagePart[],
  originalParts: MessagePart[],
): MessagePart[] {
  if (parts.length === 0) {
    return [];
  }
  const next = [...parts];
  const last = next[next.length - 1]!;
  const text = `${partText(last).replace(/\s+$/u, "")}...`;
  next[next.length - 1] = { ...last, text, content: text };
  const visibleText = next.map(partText).join("\n");
  const visibleMedia = new Set(visibleText.match(USER_MEDIA_TOKEN_PATTERN) ?? []);
  const hiddenMedia = originalParts
    .flatMap((part) => partText(part).match(USER_MEDIA_TOKEN_PATTERN) ?? [])
    .filter((token, index, tokens) => {
      return !visibleMedia.has(token) && tokens.indexOf(token) === index;
    });
  if (hiddenMedia.length > 0) {
    next.push({
      ...last,
      id: `${last.id}:media-preview`,
      text: hiddenMedia.join("\n"),
      content: hiddenMedia.join("\n"),
    });
  }
  return next;
}

export function TextPartCell(props: {
  part: MessagePart;
  streaming: boolean;
  workspaceDirectory?: string;
}) {
  const text = createMemo(() => stripReactionEmoji(partText(props.part)));
  return (
    <div class="part text-part">
      <Show
        when={text()}
        fallback={<pre>{jsonPreview(props.part.state || props.part.metadata)}</pre>}
      >
        {(value) => (
          <TypingText
            id={props.part.id}
            text={value()}
            active={props.streaming}
            workspaceDirectory={props.workspaceDirectory}
          />
        )}
      </Show>
    </div>
  );
}

export function TypingText(props: {
  id: string;
  text: string;
  active: boolean;
  workspaceDirectory?: string;
}) {
  return (
    <RichText
      text={props.text}
      active={props.active}
      workspaceDirectory={props.workspaceDirectory}
    />
  );
}
