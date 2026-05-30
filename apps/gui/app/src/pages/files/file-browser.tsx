import {
  For,
  Match,
  Show,
  Switch,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
  type Accessor,
  type JSX,
  type Setter,
} from "solid-js";
import { Portal } from "solid-js/web";
import ExternalLink from "lucide-solid/icons/external-link";
import FolderOpen from "lucide-solid/icons/folder-open";
import ArrowLeft from "lucide-solid/icons/arrow-left";
import CalendarDays from "lucide-solid/icons/calendar-days";
import ChartGantt from "lucide-solid/icons/chart-gantt";
import Check from "lucide-solid/icons/check";
import ChevronDown from "lucide-solid/icons/chevron-down";
import ChevronLeft from "lucide-solid/icons/chevron-left";
import ChevronRight from "lucide-solid/icons/chevron-right";
import Columns3 from "lucide-solid/icons/columns-3";
import Copy from "lucide-solid/icons/copy";
import Edit3 from "lucide-solid/icons/pencil";
import KeyRound from "lucide-solid/icons/key-round";
import MoreHorizontal from "lucide-solid/icons/ellipsis";
import Pin from "lucide-solid/icons/pin";
import Plus from "lucide-solid/icons/plus";
import Search from "lucide-solid/icons/search";
import Settings from "lucide-solid/icons/settings";
import Trash2 from "lucide-solid/icons/trash-2";
import {
  GatewayClient,
  GatewayError,
  connectGatewayEvents,
  defaultGatewayUrl,
  errorMessage,
  type Agent,
  type Command,
  type FileContentResponse,
  type FileInfo,
  type GatewayConfig,
  type Message,
  type ProviderAuthMethod,
  type ProductIssue,
  type Project,
  type PollInterval,
  type SdkProvider,
  type Session,
  type StartCondition,
  type TaskManagement,
  type PlanStatus,
} from "@tura/gateway-sdk";
import {
  Composer,
  ConversationView,
  composerFileToken,
  composerImageToken,
} from "../../conversation/conversation-view";
import { applyGatewayEvent } from "../../state/event-reducer";
import {
  activeSession,
  type ComposerImage,
  initialAppState,
  type MainTab,
  type PlanMode,
  sessionDirectory,
  sessionUpdatedAt,
  sessionTitle,
  type AppState,
  type SettingsSection,
  type ThemeMode,
} from "../../state/global-store";
import { classNames, truncate } from "../../state/format";
import { t, type TextKey } from "../../i18n";

import {
  fileGitRemark,
  formatFileSize,
  formatModifiedTime,
  parentPath,
  shortPathLabel,
  shortWorkspaceLabel,
} from "../../utils/app-format";
export function FileBrowserView(props: {
  path: string;
  directory?: string;
  files: FileInfo[];
  selectedFile?: FileInfo;
  fileContentLoadingPath?: string;
  fileContent?: {
    type: string;
    content: string;
    encoding?: string | null;
    mimeType?: string | null;
  };
  onFile: (file: FileInfo) => void;
  onUp: () => void;
  onList: () => void;
  onOpenDirectory: () => void;
  onOpenExternal: () => void;
}) {
  return (
    <section class="files-view layered-page layered-page-two">
      <header class="page-head page-layer-inner">
        <div class="page-title">
          <span>{t("fileBrowser")}</span>
          <h1>
            {shortPathLabel(props.path) ?? shortWorkspaceLabel(props.directory)}
          </h1>
        </div>
        <div class="page-actions">
          <button
            class="icon-action"
            title={t("openInExplorer")}
            aria-label={t("openInExplorer")}
            onClick={props.onOpenDirectory}
          >
            <FolderOpen size={17} />
          </button>
          <Show when={props.selectedFile}>
            <button
              class="icon-action"
              title={t("openWithSystemApp")}
              aria-label={t("openWithSystemApp")}
              onClick={props.onOpenExternal}
            >
              <ExternalLink size={17} />
            </button>
          </Show>
        </div>
      </header>
      <main class="file-canvas page-layer-middle">
        <div class="file-canvas-inner page-layer-inner">
          <Show
            when={props.selectedFile}
            fallback={
              <FileListView
                files={props.files}
                path={props.path}
                selectedFile={props.selectedFile}
                onFile={props.onFile}
                onUp={props.onUp}
              />
            }
          >
            {(file) => (
              <FilePreview
                file={file()}
                content={props.fileContent}
                loading={props.fileContentLoadingPath === file().path}
              />
            )}
          </Show>
        </div>
      </main>
    </section>
  );
}

export function FileListView(props: {
  files: FileInfo[];
  path: string;
  selectedFile?: FileInfo;
  onFile: (file: FileInfo) => void;
  onUp: () => void;
}) {
  return (
    <section class="surface-list-panel">
      <div class="surface-list-head file-list-head">
        <span>{t("name")}</span>
        <span>{t("git")}</span>
        <span>{t("size")}</span>
        <span>{t("modifiedAt")}</span>
      </div>
      <Show when={props.path}>
        <button class="surface-list-row file-list-row" onClick={props.onUp}>
          <span>..</span>
          <small>{t("parent")}</small>
          <small>--</small>
          <small>{parentPath(props.path) || "/"}</small>
        </button>
      </Show>
      <For
        each={props.files}
        fallback={<div class="surface-list-empty">{t("empty")}</div>}
      >
        {(file) => (
          <button
            class={classNames(
              "surface-list-row file-list-row",
              props.selectedFile?.path === file.path && "selected",
            )}
            onClick={() => props.onFile(file)}
            title={file.path}
          >
            <span>
              {file.type === "directory" ? `${file.name}/` : file.name}
            </span>
            <small>{fileGitRemark(file)}</small>
            <small>{formatFileSize(file)}</small>
            <small>{formatModifiedTime(file.modified_at)}</small>
          </button>
        )}
      </For>
    </section>
  );
}

export function FilePreview(props: {
  file?: FileInfo;
  content?: {
    type: string;
    content: string;
    encoding?: string | null;
    mimeType?: string | null;
  };
  loading?: boolean;
}) {
  const mediaSource = createMemo(() =>
    props.content?.encoding === "base64" && props.content.mimeType
      ? `data:${props.content.mimeType};base64,${props.content.content}`
      : undefined,
  );
  return (
    <section class="surface-preview-panel">
      <Show
        when={props.file}
        fallback={<div class="empty-type">{t("selectStep")}</div>}
      >
        {(file) => (
          <>
            <header>
              <span>{shortPathLabel(file().path)}</span>
              <small>
                {props.content?.mimeType ?? props.content?.type ?? file().type}
              </small>
            </header>
            <Switch fallback={<div class="binary-note">{t("empty")}</div>}>
              <Match when={props.loading}>
                <div class="file-preview-loading-placeholder">
                  <div class="loading-bar wide" />
                  <div class="loading-bar" />
                  <div class="loading-bar medium" />
                </div>
              </Match>
              <Match when={props.content?.type === "text"}>
                <pre>{props.content?.content}</pre>
              </Match>
              <Match
                when={
                  props.content?.type === "media" &&
                  props.content?.mimeType?.startsWith("image/")
                }
              >
                <div class="media-preview">
                  <img src={mediaSource()} alt={file().name} />
                </div>
              </Match>
              <Match
                when={
                  props.content?.type === "media" &&
                  props.content?.mimeType?.startsWith("video/")
                }
              >
                <div class="media-preview">
                  <video src={mediaSource()} controls />
                </div>
              </Match>
              <Match
                when={
                  props.content?.type === "media" &&
                  props.content?.mimeType === "application/pdf"
                }
              >
                <iframe
                  class="pdf-preview"
                  src={mediaSource()}
                  title={file().name}
                />
              </Match>
              <Match
                when={
                  props.content?.type === "media" &&
                  props.content?.mimeType?.startsWith("audio/")
                }
              >
                <div class="media-preview">
                  <audio src={mediaSource()} controls />
                </div>
              </Match>
            </Switch>
          </>
        )}
      </Show>
    </section>
  );
}
