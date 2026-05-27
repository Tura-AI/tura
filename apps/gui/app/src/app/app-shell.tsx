import {
  For,
  Show,
  Switch,
  Match,
  createSignal,
  type Accessor,
  type Setter,
} from "solid-js";
import { Portal } from "solid-js/web";
import PanelLeftOpen from "lucide-solid/icons/panel-left-open";
import { Composer, ConversationView } from "../conversation/conversation-view";
import { classNames } from "../state/format";
import { t } from "../i18n";
import { WorkspaceTree } from "../components/sidebar";
import { NewSessionView } from "../pages/new-session";
import {
  MainTabs,
  SettingsRail,
  SettingsView,
} from "../pages/settings/settings-view";
import { ProviderAuthDialog } from "../pages/settings/provider-settings";
import { PlanView } from "../pages/plan/plan-view";
import { FileBrowserView } from "../pages/files/file-browser";
import {
  PlanComposerControls,
  PlanComposerTaskList,
} from "../pages/plan/plan-composer";
import type { AppState } from "../state/global-store";
import { parentPath } from "../utils/app-format";
import { providerIdFromModel } from "../utils/settings";
import {
  defaultPollInterval,
  hasVisibleSessionTasks,
  localDateTimeToUtcIso,
  sessionTaskState,
  taskNonceId,
  taskStartCondition,
  utcIsoToLocalDateTime,
} from "../features/plan/tasks";

type AppShellViewModel = {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  [key: string]: any;
};

const RAIL_DEFAULT_WIDTH = 238;
const RAIL_MIN_WIDTH = 180;
const RAIL_MAX_WIDTH = 360;
const RAIL_COLLAPSE_WIDTH = 120;

export function AppShell(props: { view: AppShellViewModel }) {
  const {
    state,
    closeSettings,
    changeMainTab,
    expandedRailGroup,
    setExpandedRailGroup,
    toggleRailGroup,
    selectedSession,
    selectedMessages,
    slashCommands,
    openBlankSession,
    openSession,
    newSession,
    useWorkspaceDirectory,
    createNamedWorkspace,
    pickExistingWorkspaceDirectory,
    setState,
    submitPrompt,
    updatePlanTicketStatus,
    sessionAttentionAcknowledged,
    deletePlanTask,
    openPlanSession,
    selectDraftSession,
    createPlanTicket,
    createSessionFromPlanTask,
    updatePlanTicketTask,
    fileTree,
    fileLoadingPath,
    fileContentLoadingPath,
    expandedFileTreePaths,
    expandedWorkspace,
    setExpandedWorkspace,
    setWorkspaceTreeTouched,
    loadFiles,
    openFile,
    toggleFileTreeDirectory,
    renameSession,
    openSettings,
    openIssueConversation,
    refreshProduct,
    switchWorkspace,
    toggleWorkspace,
    directory,
    openCurrentDirectory,
    openSelectedFile,
    saveRuntimeSettings,
    validateSelectedModel,
    saveProviderKey,
    startProviderLogin,
    completeProviderLogin,
    logoutProvider,
    e2eFixture,
    providerAuthPanel,
  } = props.view;
  const [railWidth, setRailWidth] = createSignal(RAIL_DEFAULT_WIDTH);
  const [lastRailWidth, setLastRailWidth] = createSignal(RAIL_DEFAULT_WIDTH);
  const [railCollapsed, setRailCollapsed] = createSignal(
    typeof window !== "undefined" &&
      window.matchMedia("(max-width: 760px)").matches,
  );
  const [railDragging, setRailDragging] = createSignal(false);

  function openRail() {
    const width = Math.min(
      RAIL_MAX_WIDTH,
      Math.max(RAIL_MIN_WIDTH, lastRailWidth()),
    );
    setRailWidth(width);
    setLastRailWidth(width);
    setRailCollapsed(false);
  }

  function collapseRailAfterCompactSelection() {
    if (
      typeof window !== "undefined" &&
      window.matchMedia("(max-width: 760px)").matches
    ) {
      setRailCollapsed(true);
      setRailWidth(0);
    }
  }

  function previewRailResize(clientX: number) {
    if (clientX <= RAIL_COLLAPSE_WIDTH) {
      setRailCollapsed(true);
      setRailWidth(0);
      return;
    }
    setRailCollapsed(false);
    setRailWidth(Math.min(RAIL_MAX_WIDTH, Math.max(RAIL_MIN_WIDTH, clientX)));
  }

  function commitRailResize(clientX: number) {
    if (clientX <= RAIL_COLLAPSE_WIDTH) {
      setRailCollapsed(true);
      setRailWidth(0);
      return;
    }
    const nextWidth = Math.min(
      RAIL_MAX_WIDTH,
      Math.max(RAIL_MIN_WIDTH, clientX),
    );
    setRailWidth(nextWidth);
    setLastRailWidth(nextWidth);
    setRailCollapsed(false);
  }

  function beginRailResize(event: PointerEvent) {
    event.preventDefault();
    const pointerId = event.pointerId;
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(pointerId);
    setRailDragging(true);

    function resize(moveEvent: PointerEvent) {
      previewRailResize(moveEvent.clientX);
    }

    function finish(upEvent: PointerEvent) {
      if (target.hasPointerCapture(pointerId)) {
        target.releasePointerCapture(pointerId);
      }
      window.removeEventListener("pointermove", resize);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
      setRailDragging(false);
      commitRailResize(upEvent.clientX);
    }

    window.addEventListener("pointermove", resize);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
  }

  function beginRailMouseResize(event: MouseEvent) {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    setRailDragging(true);

    function resize(moveEvent: MouseEvent) {
      previewRailResize(moveEvent.clientX);
    }

    function finish(upEvent: MouseEvent) {
      window.removeEventListener("mousemove", resize);
      window.removeEventListener("mouseup", finish);
      setRailDragging(false);
      commitRailResize(upEvent.clientX);
    }

    window.addEventListener("mousemove", resize);
    window.addEventListener("mouseup", finish);
  }

  return (
    <>
      <main
        class={classNames(
          "workbench",
          railCollapsed() && "rail-collapsed",
          railDragging() && "rail-resizing",
          state().activeTab === "settings" && "settings-workbench",
        )}
        style={{ "--rail": `${railWidth()}px` }}
      >
        <aside
          class={classNames(
            "rail",
            state().activeTab === "settings" && "settings-mode",
          )}
        >
          <Show
            when={state().activeTab === "settings"}
            fallback={
              <>
                <div class="brand">
                  <div class="brand-mark" />
                  <div>
                    <strong>Tura</strong>
                  </div>
                </div>
                <MainTabs
                  active={state().previousMainTab}
                  onChange={(activeTab) => {
                    void changeMainTab(activeTab);
                    collapseRailAfterCompactSelection();
                  }}
                />
                <WorkspaceTree
                  activeTab={state().activeTab}
                  projects={state().projects}
                  directory={state().directory}
                  sessions={state().sessions}
                  selectedSessionId={state().selectedSessionId}
                  productIssues={state().productIssues}
                  filePath={state().filePath}
                  files={state().files}
                  fileTree={fileTree()}
                  fileLoadingPath={fileLoadingPath()}
                  expandedFileTreePaths={expandedFileTreePaths()}
                  selectedFile={state().selectedFile}
                  expandedWorkspace={expandedWorkspace()}
                  expandedGroup={expandedRailGroup()}
                  attentionAcknowledged={sessionAttentionAcknowledged}
                  onWorkspace={toggleWorkspace}
                  onBlankSession={() => {
                    openBlankSession();
                    collapseRailAfterCompactSelection();
                  }}
                  onGroup={toggleRailGroup}
                  onIssue={openIssueConversation}
                  onStatus={updatePlanTicketStatus}
                  onSession={(sessionId) => {
                    const session = state().sessions.find(
                      (item) => item.id === sessionId,
                    );
                    if (state().activeTab === "plan" && session) {
                      void openPlanSession(session);
                      collapseRailAfterCompactSelection();
                      return;
                    }
                    if (state().activeTab === "new") {
                      setState((previous) => ({
                        ...previous,
                        activeTab: "conversation",
                        previousMainTab: "new",
                      }));
                    }
                    void openSession(sessionId);
                    collapseRailAfterCompactSelection();
                  }}
                  onRenameSession={renameSession}
                  onFile={(file) => {
                    void openFile(file);
                    collapseRailAfterCompactSelection();
                  }}
                  onFileTreeDirectory={toggleFileTreeDirectory}
                  onUp={() => loadFiles(parentPath(state().filePath))}
                  onSettings={() => {
                    openSettings("workspace");
                    collapseRailAfterCompactSelection();
                  }}
                />
                <button
                  class="settings-entry"
                  type="button"
                  onClick={() => {
                    openSettings("general");
                    collapseRailAfterCompactSelection();
                  }}
                >
                  {t("settings")}
                </button>
              </>
            }
          >
            <SettingsRail
              active={state().settingsSection}
              onBack={closeSettings}
              onSection={(settingsSection) =>
                setState((previous) => ({ ...previous, settingsSection }))
              }
            />
          </Show>
        </aside>
        <div
          class="rail-resize-handle"
          role="separator"
          aria-orientation="vertical"
          aria-label={t("sidebar")}
          onPointerDown={beginRailResize}
          onMouseDown={beginRailMouseResize}
        />

        <section class="main-column">
          <Show when={railCollapsed()}>
            <button
              class="rail-open-button"
              type="button"
              title={t("sidebar")}
              aria-label={t("sidebar")}
              onClick={openRail}
            >
              <PanelLeftOpen size={17} strokeWidth={1.8} />
            </button>
          </Show>
          <Show when={state().error}>
            {(error) => (
              <div class="error-strip">
                <span>{error()}</span>
                <button
                  onClick={() =>
                    setState((previous) => ({ ...previous, error: undefined }))
                  }
                >
                  ×
                </button>
              </div>
            )}
          </Show>
          <Switch>
            <Match when={state().activeTab === "new"}>
              <NewSessionView
                state={state()}
                slashCommands={slashCommands()}
                onWorkspace={useWorkspaceDirectory}
                onCreateWorkspace={createNamedWorkspace}
                onPickDirectory={pickExistingWorkspaceDirectory}
                onComposerText={(composerText) =>
                  setState((previous) => ({ ...previous, composerText }))
                }
                onComposerImages={(composerImages) =>
                  setState((previous) => ({ ...previous, composerImages }))
                }
                onDraftStartCondition={(planDraftStartCondition) =>
                  setState((previous) => ({
                    ...previous,
                    planDraftStartCondition,
                  }))
                }
                onDraftStartAt={(planDraftStartAt) =>
                  setState((previous) => ({ ...previous, planDraftStartAt }))
                }
                onDraftPollInterval={(planDraftPollInterval) =>
                  setState((previous) => ({
                    ...previous,
                    planDraftPollInterval,
                  }))
                }
                onSubmit={submitPrompt}
              />
            </Match>
            <Match when={state().activeTab === "plan"}>
              <PlanView
                state={state()}
                previewSession={state().sessions.find(
                  (session) => session.id === state().planPreviewSessionId,
                )}
                previewMessages={
                  state().planPreviewSessionId
                    ? (state().messagesBySession[
                        state().planPreviewSessionId!
                      ] ?? [])
                    : []
                }
                slashCommands={slashCommands()}
                onPlanMode={(planMode) =>
                  setState((previous) => ({ ...previous, planMode }))
                }
                onClosePanel={() =>
                  setState((previous) => ({
                    ...previous,
                    planPreviewSessionId: undefined,
                    planDraftLane: undefined,
                    planDraftSessionId: undefined,
                    editingTask: undefined,
                  }))
                }
                onSearch={(issueSearch) =>
                  setState((previous) => ({ ...previous, issueSearch }))
                }
                onDraftLane={(planDraftLane) =>
                  setState((previous) => ({
                    ...previous,
                    planDraftLane,
                    editingTask: undefined,
                  }))
                }
                onDraftStartCondition={(planDraftStartCondition) =>
                  setState((previous) => ({
                    ...previous,
                    planDraftStartCondition,
                  }))
                }
                onDraftStartAt={(planDraftStartAt) =>
                  setState((previous) => ({ ...previous, planDraftStartAt }))
                }
                onDraftPollInterval={(planDraftPollInterval) =>
                  setState((previous) => ({
                    ...previous,
                    planDraftPollInterval,
                  }))
                }
                onDraftSession={(planDraftSessionId) =>
                  void selectDraftSession(planDraftSessionId)
                }
                onCreateTicket={createPlanTicket}
                onStatus={updatePlanTicketStatus}
                attentionAcknowledged={sessionAttentionAcknowledged}
                onTask={updatePlanTicketTask}
                onEditTask={(session, task, composerText) =>
                  setState((previous) => ({
                    ...previous,
                    composerText,
                    editingTask: {
                      sessionId: session.id,
                      nonce_id: taskNonceId(task),
                    },
                  }))
                }
                onDeleteTask={deletePlanTask}
                onCreateSessionFromTask={createSessionFromPlanTask}
                onOpenSession={openPlanSession}
                onComposerText={(composerText) =>
                  setState((previous) => ({ ...previous, composerText }))
                }
                onComposerImages={(composerImages) =>
                  setState((previous) => ({ ...previous, composerImages }))
                }
                onSubmit={submitPrompt}
                onOpenFullConversation={() =>
                  setState((previous) => ({
                    ...previous,
                    activeTab: "conversation",
                    previousMainTab: "new",
                  }))
                }
              />
            </Match>
            <Match when={state().activeTab === "files"}>
              <FileBrowserView
                path={state().filePath}
                directory={state().directory}
                files={state().files}
                selectedFile={state().selectedFile}
                fileContent={state().fileContent}
                fileContentLoadingPath={fileContentLoadingPath()}
                onFile={openFile}
                onUp={() => loadFiles(parentPath(state().filePath))}
                onOpenDirectory={openCurrentDirectory}
                onList={() =>
                  setState((previous) => ({
                    ...previous,
                    selectedFile: undefined,
                    fileContent: undefined,
                  }))
                }
                onOpenExternal={openSelectedFile}
              />
            </Match>
            <Match when={state().activeTab === "conversation"}>
              <ConversationView
                state={state()}
                session={selectedSession()}
                messages={selectedMessages()}
                slashCommands={slashCommands()}
                onComposerText={(composerText) =>
                  setState((previous) => ({ ...previous, composerText }))
                }
                onComposerImages={(composerImages) =>
                  setState((previous) => ({ ...previous, composerImages }))
                }
                onSubmit={submitPrompt}
                composerToolbar={
                  selectedSession() ? (
                    <PlanComposerControls
                      startCondition={taskStartCondition(
                        sessionTaskState(selectedSession()!),
                      )}
                      startAt={utcIsoToLocalDateTime(
                        sessionTaskState(selectedSession()!).start_at,
                      )}
                      pollInterval={
                        sessionTaskState(selectedSession()!).poll_interval ??
                        defaultPollInterval()
                      }
                      onStartCondition={(start_condition) =>
                        updatePlanTicketTask(selectedSession()!, {
                          status: "todo",
                        })
                      }
                      onStartAt={(value) => {
                        const start_at = localDateTimeToUtcIso(value);
                        if (start_at) {
                          void updatePlanTicketTask(selectedSession()!, {
                            start_at,
                          });
                        }
                      }}
                      onPollInterval={(poll_interval) =>
                        updatePlanTicketTask(selectedSession()!, {
                          poll_interval,
                        })
                      }
                    />
                  ) : undefined
                }
                composerTaskList={
                  selectedSession() &&
                  hasVisibleSessionTasks(selectedSession()!) ? (
                    <PlanComposerTaskList
                      session={selectedSession()!}
                      selected_nonce_id={state().editingTask?.nonce_id}
                      onEdit={(task, composerText) =>
                        setState((previous) => ({
                          ...previous,
                          composerText,
                          editingTask: {
                            sessionId: selectedSession()!.id,
                            nonce_id: taskNonceId(task),
                          },
                        }))
                      }
                      onDelete={(task) =>
                        deletePlanTask(selectedSession()!, task)
                      }
                      onCreateSession={(task) =>
                        createSessionFromPlanTask(selectedSession()!, task)
                      }
                    />
                  ) : undefined
                }
              />
            </Match>
            <Match when={state().activeTab === "settings"}>
              <SettingsView
                state={state()}
                section={state().settingsSection}
                onProvider={(providerId) =>
                  setState((previous) => ({
                    ...previous,
                    selectedProviderId: providerId,
                  }))
                }
                onModel={(selectedModel) =>
                  setState((previous) => ({
                    ...previous,
                    selectedModel,
                    selectedProviderId:
                      providerIdFromModel(selectedModel) ??
                      previous.selectedProviderId,
                  }))
                }
                onAgent={(selectedAgent) =>
                  setState((previous) => ({ ...previous, selectedAgent }))
                }
                onVariant={(modelVariant) =>
                  setState((previous) => ({ ...previous, modelVariant }))
                }
                onAcceleration={(accelerationEnabled) =>
                  setState((previous) => ({
                    ...previous,
                    accelerationEnabled,
                  }))
                }
                onTheme={(themeMode) =>
                  setState((previous) => ({
                    ...previous,
                    themeMode,
                    configDraft: {
                      ...previous.configDraft,
                      theme: themeMode,
                    },
                  }))
                }
                onConfigDraft={(key, value) =>
                  setState((previous) => ({
                    ...previous,
                    configDraft: {
                      ...previous.configDraft,
                      [key]: value,
                    },
                  }))
                }
                onWorkspaceConfigDraft={(key, value) =>
                  setState((previous) => ({
                    ...previous,
                    workspaceConfigDraft: {
                      ...previous.workspaceConfigDraft,
                      [key]: value,
                    },
                  }))
                }
                onAuthDraft={(providerId, value) =>
                  setState((previous) => ({
                    ...previous,
                    authDrafts: {
                      ...previous.authDrafts,
                      [providerId]: value,
                    },
                  }))
                }
                onAuthCode={(providerId, value) =>
                  setState((previous) => ({
                    ...previous,
                    authCodeDrafts: {
                      ...previous.authCodeDrafts,
                      [providerId]: value,
                    },
                  }))
                }
                onProviderSearch={(providerSearch) =>
                  setState((previous) => ({ ...previous, providerSearch }))
                }
                onOpenProviderAuth={(providerId) =>
                  setState((previous) => ({
                    ...previous,
                    selectedProviderId: providerId,
                    providerAuthPanel: { providerId },
                  }))
                }
                onSaveSettings={saveRuntimeSettings}
                onValidateModel={validateSelectedModel}
                onSaveKey={saveProviderKey}
                onStartLogin={startProviderLogin}
                onCompleteLogin={completeProviderLogin}
                onLogout={logoutProvider}
              />
            </Match>
          </Switch>
        </section>
      </main>
      <Show when={state().providerAuthPanel}>
        {(panel) => (
          <Portal>
            <ProviderAuthDialog
              state={state()}
              panel={panel()}
              onCancel={() =>
                setState((previous) => ({
                  ...previous,
                  providerAuthPanel: undefined,
                }))
              }
              onAuthDraft={(providerId, value) =>
                setState((previous) => ({
                  ...previous,
                  authDrafts: {
                    ...previous.authDrafts,
                    [providerId]: value,
                  },
                }))
              }
              onAuthCode={(providerId, value) =>
                setState((previous) => ({
                  ...previous,
                  authCodeDrafts: {
                    ...previous.authCodeDrafts,
                    [providerId]: value,
                  },
                }))
              }
              onSaveKey={saveProviderKey}
              onStartLogin={startProviderLogin}
              onCompleteLogin={completeProviderLogin}
              onLogout={logoutProvider}
            />
          </Portal>
        )}
      </Show>
    </>
  );
}
