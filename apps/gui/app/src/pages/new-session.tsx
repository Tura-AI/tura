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
import LayoutList from "lucide-solid/icons/layout-list";
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
import FolderOpen from "lucide-solid/icons/folder-open";
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
} from "../conversation/conversation-view";
import { applyGatewayEvent } from "../state/event-reducer";
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
} from "../state/global-store";
import { classNames, truncate } from "../state/format";
import { t, type TextKey } from "../i18n";

import { samePath, shortWorkspaceLabel } from "../utils/app-format";
import { PlanComposerControls } from "./plan/plan-composer";
export function ConversationEmptyView(props: {
  state: AppState;
  slashCommands: Command[];
  onWorkspace: (directory: string) => void;
  onCreateWorkspace: (name: string) => void | Promise<void>;
  onPickDirectory: () => Promise<void>;
  onComposerText: (value: string) => void;
  onComposerImages: (images: ComposerImage[]) => void;
  onDraftStartCondition: (value: StartCondition) => void;
  onDraftStartAt: (value: string) => void;
  onDraftPollInterval: (value: PollInterval) => void;
  onSubmit: () => void;
}) {
  const [naming, setNaming] = createSignal(false);
  const [query, setQuery] = createSignal("");
  const projects = createMemo(() => {
    const fallback = props.state.directory
      ? [
          {
            id: props.state.directory,
            name: shortWorkspaceLabel(props.state.directory),
            worktree: props.state.directory,
          } as Project,
        ]
      : [];
    const normalizedQuery = query().trim().toLowerCase();
    return (props.state.projects.length ? props.state.projects : fallback)
      .filter((project) => {
        if (!normalizedQuery) {
          return true;
        }
        return `${project.name} ${project.worktree}`
          .toLowerCase()
          .includes(normalizedQuery);
      })
      .slice(0, 10);
  });

  return (
    <section class="new-session-view">
      <div class="new-session-center">
        <h1>{t("todayQuestion")}</h1>
        <Composer
          text={props.state.composerText}
          images={props.state.composerImages}
          submitting={props.state.submitting}
          slashCommands={props.slashCommands}
          onText={props.onComposerText}
          onImages={props.onComposerImages}
          onSubmit={props.onSubmit}
          toolbar={
            <>
              <NewSessionWorkspacePicker
                projects={projects()}
                directory={props.state.directory}
                query={query()}
                onQuery={setQuery}
                onWorkspace={props.onWorkspace}
                onCreateWorkspace={() => setNaming(true)}
                onPickDirectory={props.onPickDirectory}
              />
              <PlanComposerControls
                startCondition={props.state.planDraftStartCondition}
                startAt={props.state.planDraftStartAt}
                pollInterval={props.state.planDraftPollInterval}
                onStartCondition={props.onDraftStartCondition}
                onStartAt={props.onDraftStartAt}
                onPollInterval={props.onDraftPollInterval}
              />
            </>
          }
        />
      </div>
      <Show when={naming()}>
        <NameDialog
          title={t("createWorkspace")}
          description={t("renameSessionHint")}
          initialValue="New project"
          onCancel={() => setNaming(false)}
          onSave={(value) => {
            void props.onCreateWorkspace(value);
            setNaming(false);
          }}
        />
      </Show>
    </section>
  );
}

export function NewSessionWorkspacePicker(props: {
  projects: Project[];
  directory?: string;
  query: string;
  onQuery: (value: string) => void;
  onWorkspace: (directory: string) => void;
  onCreateWorkspace: () => void;
  onPickDirectory: () => Promise<void>;
}) {
  let root: HTMLElement | undefined;
  const [open, setOpen] = createSignal(false);
  const selectedProject = createMemo(() =>
    props.projects.find((project) =>
      samePath(project.worktree, props.directory),
    ),
  );

  async function pickDirectory() {
    await props.onPickDirectory();
    setOpen(false);
  }

  createEffect(() => {
    if (!open()) {
      return;
    }
    const closeOutside = (event: PointerEvent) => {
      if (!root?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    onCleanup(() => document.removeEventListener("pointerdown", closeOutside));
  });

  return (
    <section class="plan-session-picker" ref={root}>
      <button
        type="button"
        class="plan-session-button"
        onClick={() => setOpen(!open())}
        title={selectedProject()?.worktree ?? t("chooseWorkspace")}
      >
        <FolderOpen size={15} strokeWidth={1.6} />
        <span>
          {selectedProject()?.name ??
            (props.directory
              ? shortWorkspaceLabel(props.directory)
              : t("chooseWorkspace"))}
        </span>
        <ChevronDown size={13} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div class="plan-session-menu">
          <label class="workspace-search-row">
            <Search size={14} strokeWidth={1.7} />
            <input
              class="workspace-search"
              value={props.query}
              placeholder={`${t("workspaceSearch")}...`}
              onInput={(event) => props.onQuery(event.currentTarget.value)}
            />
          </label>
          <div class="workspace-picker-list plan-session-list">
            <For each={props.projects}>
              {(project) => (
                <button
                  type="button"
                  class={classNames(
                    "workspace-pick-row",
                    samePath(project.worktree, props.directory) && "selected",
                  )}
                  onClick={() => {
                    props.onWorkspace(project.worktree);
                    setOpen(false);
                  }}
                  title={project.worktree}
                >
                  <FolderOpen size={15} strokeWidth={1.6} />
                  <span>
                    {project.name || shortWorkspaceLabel(project.worktree)}
                  </span>
                  <Show when={samePath(project.worktree, props.directory)}>
                    <Check size={14} strokeWidth={1.8} />
                  </Show>
                </button>
              )}
            </For>
          </div>
          <div class="workspace-picker-actions">
            <button type="button" onClick={props.onCreateWorkspace}>
              <span>{t("createWorkspace")}</span>
            </button>
            <button type="button" onClick={pickDirectory}>
              <span>{t("localDirectory")}</span>
            </button>
          </div>
        </div>
      </Show>
    </section>
  );
}

export function NameDialog(props: {
  title: string;
  description: string;
  initialValue: string;
  onCancel: () => void;
  onSave: (value: string) => void;
}) {
  const [value, setValue] = createSignal(props.initialValue);
  return (
    <div class="modal-scrim" onMouseDown={props.onCancel}>
      <div class="name-dialog" onMouseDown={(event) => event.stopPropagation()}>
        <header>
          <div>
            <h2>{props.title}</h2>
            <p>{props.description}</p>
          </div>
          <button type="button" onClick={props.onCancel}>
            ×
          </button>
        </header>
        <input
          value={value()}
          autofocus
          onInput={(event) => setValue(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              props.onSave(value());
            }
            if (event.key === "Escape") {
              props.onCancel();
            }
          }}
        />
        <footer>
          <button type="button" class="secondary" onClick={props.onCancel}>
            {t("cancel")}
          </button>
          <button
            type="button"
            class="primary"
            disabled={!value().trim()}
            onClick={() => props.onSave(value())}
          >
            {t("save")}
          </button>
        </footer>
      </div>
    </div>
  );
}

export function FileTreeLabel(props: { file: FileInfo; expanded?: boolean }) {
  return (
    <Show
      when={props.file.type === "directory"}
      fallback={<span>{props.file.name}</span>}
    >
      <span>{`${props.file.name}/`}</span>
    </Show>
  );
}
