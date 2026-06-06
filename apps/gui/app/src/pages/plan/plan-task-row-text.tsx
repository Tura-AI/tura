import FileIcon from "lucide-solid/icons/file";
import ImageIcon from "lucide-solid/icons/image";
import { For, Show, createMemo } from "solid-js";
import { t } from "../../i18n";
import { classNames } from "../../state/format";

const TASK_MEDIA_TOKEN_PATTERN =
  /\[\[(image|file):([a-zA-Z0-9_-]+)\]\]|\[(image|file):([a-zA-Z0-9_-]+)\]/gu;

type TaskTextSegment = { type: "text"; value: string } | { type: "image" | "file"; value: string };

function taskTextSegments(text: string): TaskTextSegment[] {
  const segments: TaskTextSegment[] = [];
  let cursor = 0;
  for (const match of text.matchAll(TASK_MEDIA_TOKEN_PATTERN)) {
    if (match.index > cursor) {
      segments.push({ type: "text", value: text.slice(cursor, match.index) });
    }
    segments.push({
      type: (match[1] ?? match[3]) === "file" ? "file" : "image",
      value: match[2] ?? match[4] ?? "",
    });
    cursor = match.index + match[0].length;
  }
  if (cursor < text.length) {
    segments.push({ type: "text", value: text.slice(cursor) });
  }
  return segments.length > 0 ? segments : [{ type: "text", value: text }];
}

export function TaskRowText(props: { text: string }) {
  const segments = createMemo(() => taskTextSegments(props.text));
  return (
    <>
      <For each={segments()}>
        {(segment) => (
          <Show when={segment.type !== "text"} fallback={<>{segment.value}</>}>
            <TaskMediaToken type={segment.type as "image" | "file"} id={segment.value} />
          </Show>
        )}
      </For>
    </>
  );
}

function TaskMediaToken(props: { type: "image" | "file"; id: string }) {
  const Icon = props.type === "file" ? FileIcon : ImageIcon;
  const label = createMemo(() =>
    props.type === "file" ? t("attachmentFile") : t("attachmentImage"),
  );
  return (
    <span
      class={classNames(
        "composer-attachment-token",
        "task-media-token",
        props.type === "image" && "composer-image-token",
        props.type === "file" && "composer-file-token",
      )}
      title={`${label()} ${props.id}`}
    >
      <span class="task-media-token-main">
        <Icon size={13} strokeWidth={1.7} />
        <span>{label()}</span>
      </span>
    </span>
  );
}
