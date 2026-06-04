import {
  type FileInfo,
  type PlanStatus,
  type ProductIssue,
  type Session,
} from "@tura/gateway-sdk";
import Edit3 from "lucide-solid/icons/pencil";
import { For, Match, Show, Switch, createMemo, createSignal } from "solid-js";
import { t } from "../../i18n";
import { NameDialog } from "../../pages/new-session";
import { SessionRowMeta } from "../../pages/plan/plan-view";
import { classNames } from "../../state/format";
import { sessionTitle, type MainTab } from "../../state/global-store";
import {
  hiddenRootSessionCount,
  rootSessions,
  visibleSessionTreeRows,
} from "../../state/session-tree";
import { sessionHoverTitle, shortSessionTitle } from "../../utils/app-format";
import { FileTreeRows } from "./file-tree-rows";

export function WorkspaceChildren(props: {
  activeTab: MainTab;
  expandedGroup?: string;
  sessions: Session[];
  attentionAcknowledged: (session: Session) => boolean;
  selectedSessionId?: string;
  productIssues: ProductIssue[];
  filePath: string;
  files: FileInfo[];
  fileTree: Record<string, FileInfo[]>;
  fileLoadingPath?: string;
  expandedFileTreePaths: Set<string>;
  selectedFile?: FileInfo;
  onIssue: (issue: ProductIssue) => void;
  onGroup: (id: string) => void;
  onStatus: (session: Session, status: PlanStatus) => void;
  onSession: (session: Session) => void;
  onRenameSession: (sessionId: string, title: string) => void;
  onFile: (file: FileInfo) => void;
  onFileTreeDirectory: (file: FileInfo) => void;
  onUp: () => void;
}) {
  const [expandedSessions, setExpandedSessions] = createSignal(false);
  const [renaming, setRenaming] = createSignal<Session>();
  const visibleSessions = createMemo(() =>
    visibleSessionTreeRows(props.sessions, props.selectedSessionId, {
      expandedRoots: expandedSessions(),
    }),
  );
  const hiddenSessionCount = createMemo(() =>
    expandedSessions()
      ? 0
      : hiddenRootSessionCount(props.sessions, props.selectedSessionId),
  );
  const rootFiles = createMemo(() => props.fileTree[""] ?? props.files);
  const sortedPlanSessions = createMemo(() => rootSessions(props.sessions));
  return (
    <div class="workspace-children">
      <Switch>
        <Match when={props.activeTab === "plan"}>
          <For
            each={sortedPlanSessions()}
            fallback={<div class="rail-empty">{t("noSessions")}</div>}
          >
            {(session) => (
              <SessionButton
                session={session}
                selected={props.selectedSessionId === session.id}
                attentionAcknowledged={props.attentionAcknowledged(session)}
                onSession={props.onSession}
                onRename={setRenaming}
              />
            )}
          </For>
        </Match>
        <Match when={props.activeTab === "conversation"}>
          <For
            each={visibleSessions()}
            fallback={<div class="rail-empty">{t("noSessions")}</div>}
          >
            {(row) => (
              <SessionButton
                session={row.session}
                depth={row.depth}
                selected={props.selectedSessionId === row.session.id}
                attentionAcknowledged={props.attentionAcknowledged(row.session)}
                onSession={props.onSession}
                onRename={setRenaming}
              />
            )}
          </For>
          <Show when={hiddenSessionCount() > 0}>
            <button
              type="button"
              class="child-row rail-more"
              style={{ "--depth": 1 }}
              onClick={() => setExpandedSessions((value) => !value)}
            >
              {expandedSessions()
                ? t("collapse")
                : t("showMore", { count: hiddenSessionCount() })}
            </button>
          </Show>
          <Show when={renaming()}>
            {(session) => (
              <NameDialog
                title={t("renameSession")}
                description={t("renameSessionHint")}
                initialValue={sessionTitle(session())}
                onCancel={() => setRenaming(undefined)}
                onSave={(value) => {
                  props.onRenameSession(session().id, value);
                  setRenaming(undefined);
                }}
              />
            )}
          </Show>
        </Match>
        <Match when={props.activeTab === "files"}>
          <FileTreeRows
            files={rootFiles()}
            fileTree={props.fileTree}
            activePath={props.filePath}
            loadingPath={props.fileLoadingPath}
            expandedPaths={props.expandedFileTreePaths}
            selectedFile={props.selectedFile}
            depth={0}
            onFile={props.onFile}
            onDirectory={props.onFileTreeDirectory}
          />
        </Match>
      </Switch>
    </div>
  );
}

function SessionButton(props: {
  session: Session;
  depth?: number;
  selected: boolean;
  attentionAcknowledged: boolean;
  onSession: (session: Session) => void;
  onRename: (session: Session) => void;
}) {
  return (
    <button
      class={classNames(
        "child-row",
        "session-row",
        props.selected && "selected",
      )}
      style={{ "--depth": 1, "--session-depth": props.depth ?? 0 }}
      onClick={() => props.onSession(props.session)}
      title={sessionHoverTitle(props.session)}
    >
      <span>{shortSessionTitle(sessionTitle(props.session))}</span>
      <SessionRowMeta
        session={props.session}
        attentionAcknowledged={props.attentionAcknowledged}
      />
      <Edit3
        class="session-rename-icon"
        size={13}
        strokeWidth={1.7}
        onClick={(event) => {
          event.stopPropagation();
          props.onRename(props.session);
        }}
      />
    </button>
  );
}
