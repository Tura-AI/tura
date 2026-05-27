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

export type ComposerPreviewSegment =
  | { type: "text"; value: string }
  | { type: "image"; value: string }
  | { type: "file"; value: string };

export const COMPOSER_ATTACHMENT_TOKEN_PATTERN =
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

export function composerAttachmentToken(attachment: ComposerImage): string {
  return attachmentKind(attachment) === "image"
    ? composerImageToken(attachment.id)
    : composerFileToken(attachment.id);
}

export function composerToken(
  type: ComposerPreviewSegment["type"],
  id: string,
): string {
  return type === "file" ? composerFileToken(id) : composerImageToken(id);
}

export function composerTokenForElement(element: HTMLElement): string {
  const id = element.dataset.attachmentId ?? "";
  return element.dataset.attachmentKind === "file"
    ? composerFileToken(id)
    : composerImageToken(id);
}

export function attachmentKind(attachment: ComposerImage): "image" | "file" {
  return (
    attachment.kind ??
    (attachment.mimeType?.startsWith("image/") ? "image" : "image")
  );
}

export function readImageDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.onerror = () =>
      reject(reader.error ?? new Error("Failed to read image"));
    reader.readAsDataURL(file);
  });
}

export function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

export function composerInputHeight(value: string): string {
  const lines = Math.min(
    8,
    Math.max(
      3,
      value.split(/\r\n|\r|\n/u).length + Math.floor(value.length / 88),
    ),
  );
  return `${lines * 22 + 18}px`;
}
