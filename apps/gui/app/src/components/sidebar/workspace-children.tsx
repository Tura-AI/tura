import { type FileInfo, type PlanStatus, type ProductIssue, type Session } from "@tura/gateway-sdk";
import Edit3 from "lucide-solid/icons/pencil";
import { For, Match, Show, Switch, type Accessor, createMemo, createSignal } from "solid-js";
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
  sessionsLoading: boolean;
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
    expandedSessions() ? 0 : hiddenRootSessionCount(props.sessions, props.selectedSessionId),
  );
  const rootFiles = createMemo(() => props.fileTree[""] ?? props.files);
  const sessionsById = createMemo(
    () => new Map(props.sessions.map((session) => [session.id, session])),
  );
  const sortedPlanSessionIds = createMemo(() =>
    rootSessions(props.sessions).map((session) => session.id),
  );
  const visibleSessionIds = createMemo(() => visibleSessions().map((row) => row.session.id));
  const visibleSessionDepthById = createMemo(
    () => new Map(visibleSessions().map((row) => [row.session.id, row.depth])),
  );
  const sessionById = (sessionId: string) => () => sessionsById().get(sessionId);
  const sessionFallback = () =>
    props.sessionsLoading ? (
      <div class="rail-session-loading" aria-label={t("loading")}>
        <span class="loading-bar medium" />
      </div>
    ) : (
      <div class="rail-empty">{t("noSessions")}</div>
    );
  return (
    <div class="workspace-children">
      <Switch>
        <Match when={props.activeTab === "plan"}>
          <For each={sortedPlanSessionIds()} fallback={sessionFallback()}>
            {(sessionId) => (
              <SessionButton
                session={sessionById(sessionId)}
                selected={() => props.selectedSessionId === sessionId}
                attentionAcknowledged={() => {
                  const session = sessionsById().get(sessionId);
                  return session ? props.attentionAcknowledged(session) : false;
                }}
                onSession={props.onSession}
                onRename={setRenaming}
              />
            )}
          </For>
        </Match>
        <Match when={props.activeTab === "conversation"}>
          <For each={visibleSessionIds()} fallback={sessionFallback()}>
            {(sessionId) => (
              <SessionButton
                session={sessionById(sessionId)}
                depth={() => visibleSessionDepthById().get(sessionId) ?? 0}
                selected={() => props.selectedSessionId === sessionId}
                attentionAcknowledged={() => {
                  const session = sessionsById().get(sessionId);
                  return session ? props.attentionAcknowledged(session) : false;
                }}
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
              {expandedSessions() ? t("collapse") : t("showMore", { count: hiddenSessionCount() })}
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
  session: Accessor<Session | undefined>;
  depth?: Accessor<number>;
  selected: Accessor<boolean>;
  attentionAcknowledged: Accessor<boolean>;
  onSession: (session: Session) => void;
  onRename: (session: Session) => void;
}) {
  return (
    <Show when={props.session()}>
      {(session) => (
        <button
          class={classNames("child-row", "session-row", props.selected() && "selected")}
          style={{ "--depth": 1, "--session-depth": props.depth?.() ?? 0 }}
          onClick={() => props.onSession(session())}
          title={sessionHoverTitle(session())}
        >
          <span>{shortSessionTitle(sessionTitle(session()))}</span>
          <SessionRowMeta
            session={session()}
            attentionAcknowledged={props.attentionAcknowledged()}
          />
          <Edit3
            class="session-rename-icon"
            size={13}
            strokeWidth={1.7}
            onClick={(event) => {
              event.stopPropagation();
              props.onRename(session());
            }}
          />
        </button>
      )}
    </Show>
  );
}
