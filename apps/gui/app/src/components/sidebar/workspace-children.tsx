import { type FileInfo, type Message, type ProductIssue, type Session } from "@tura/gateway-sdk";
import { For, Match, Show, Switch, type Accessor, createMemo, createSignal } from "solid-js";
import { t } from "../../i18n";
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
  messagesBySession: Record<string, Message[]>;
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
  onSession: (session: Session) => void;
  onDeleteSession: (session: Session) => void;
  onFile: (file: FileInfo) => void;
  onFileTreeDirectory: (file: FileInfo) => void;
  onUp: () => void;
}) {
  const [expandedSessions, setExpandedSessions] = createSignal(false);
  const visibleSessions = createMemo(() =>
    visibleSessionTreeRows(props.sessions, props.selectedSessionId, {
      expandedRoots: expandedSessions(),
    }),
  );
  const hiddenSessionCount = createMemo(() =>
    hiddenRootSessionCount(props.sessions, props.selectedSessionId),
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
                messages={() => props.messagesBySession[sessionId]}
                selected={() => props.selectedSessionId === sessionId}
                attentionAcknowledged={() => {
                  const session = sessionsById().get(sessionId);
                  return session ? props.attentionAcknowledged(session) : false;
                }}
                onSession={props.onSession}
                onDelete={props.onDeleteSession}
              />
            )}
          </For>
        </Match>
        <Match when={props.activeTab === "conversation"}>
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
          <For each={visibleSessionIds()} fallback={sessionFallback()}>
            {(sessionId) => (
              <SessionButton
                session={sessionById(sessionId)}
                messages={() => props.messagesBySession[sessionId]}
                depth={() => visibleSessionDepthById().get(sessionId) ?? 0}
                selected={() => props.selectedSessionId === sessionId}
                attentionAcknowledged={() => {
                  const session = sessionsById().get(sessionId);
                  return session ? props.attentionAcknowledged(session) : false;
                }}
                onSession={props.onSession}
                onDelete={props.onDeleteSession}
              />
            )}
          </For>
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
  messages: Accessor<Message[] | undefined>;
  depth?: Accessor<number>;
  selected: Accessor<boolean>;
  attentionAcknowledged: Accessor<boolean>;
  onSession: (session: Session) => void;
  onDelete: (session: Session) => void;
}) {
  return (
    <Show when={props.session()}>
      {(session) => (
        <div
          role="button"
          tabindex={0}
          class={classNames("child-row", "session-row", props.selected() && "selected")}
          style={{ "--depth": 1, "--session-depth": props.depth?.() ?? 0 }}
          onClick={() => props.onSession(session())}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              props.onSession(session());
            }
          }}
          title={sessionHoverTitle(session())}
        >
          <span>{shortSessionTitle(sessionTitle(session()))}</span>
          <SessionRowMeta
            session={session()}
            messages={props.messages()}
            attentionAcknowledged={props.attentionAcknowledged()}
          />
          <button
            type="button"
            class="session-row-action"
            title={t("delete")}
            onClick={(event) => {
              event.stopPropagation();
              props.onDelete(session());
            }}
          >
            ×
          </button>
        </div>
      )}
    </Show>
  );
}
