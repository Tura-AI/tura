import { Show } from "solid-js";
import { WorkspaceTree } from "../components/sidebar";
import { t } from "../i18n";
import { MainTabs, SettingsRail } from "../pages/settings/settings-view";
import { classNames } from "../state/format";
import { parentPath } from "../utils/app-format";
import type { AppShellViewModel } from "./app-shell-view-model";

export function AppRail(props: { view: AppShellViewModel; collapseAfterSelection: () => void }) {
  const {
    state,
    closeSettings,
    changeMainTab,
    expandedRailGroup,
    toggleRailGroup,
    openBlankSession,
    openSession,
    setState,
    updatePlanTicketStatus,
    sessionAttentionAcknowledged,
    openPlanSession,
    fileTree,
    fileLoadingPath,
    expandedFileTreePaths,
    expandedWorkspaces,
    loadFiles,
    openFile,
    toggleFileTreeDirectory,
    deleteSession,
    openSettings,
    openIssueConversation,
    toggleWorkspace,
  } = props.view;

  function openAppearanceSettings() {
    openSettings("appearance");
    props.collapseAfterSelection();
  }

  function selectWorkspace(project: Parameters<typeof toggleWorkspace>[0]) {
    void toggleWorkspace(project);
    props.collapseAfterSelection();
  }

  return (
    <aside class={classNames("rail", state().activeTab === "settings" && "settings-mode")}>
      <Show
        when={state().activeTab === "settings"}
        fallback={
          <>
            <MainTabs
              active={state().previousMainTab}
              conversationLabel={t("session")}
              onChange={(activeTab) => {
                void changeMainTab(activeTab);
                props.collapseAfterSelection();
              }}
            />
            <WorkspaceTree
              activeTab={state().activeTab}
              projects={state().projects}
              directory={state().directory}
              sessions={state().sessions}
              sessionsLoading={state().sessionsLoading}
              selectedSessionId={state().selectedSessionId}
              productIssues={state().productIssues}
              filePath={state().filePath}
              files={state().files}
              fileTree={fileTree()}
              fileLoadingPath={fileLoadingPath()}
              expandedFileTreePaths={expandedFileTreePaths()}
              selectedFile={state().selectedFile}
              expandedWorkspaces={expandedWorkspaces()}
              expandedGroup={expandedRailGroup()}
              attentionAcknowledged={sessionAttentionAcknowledged}
              onWorkspace={selectWorkspace}
              onBlankSession={(project) => {
                openBlankSession(project);
                props.collapseAfterSelection();
              }}
              onGroup={toggleRailGroup}
              onIssue={openIssueConversation}
              onStatus={updatePlanTicketStatus}
              onSession={(sessionId) => {
                const session = state().sessions.find((item) => item.id === sessionId);
                if (state().activeTab === "plan" && session) {
                  void openPlanSession(session);
                  props.collapseAfterSelection();
                  return;
                }
                void openSession(sessionId);
                props.collapseAfterSelection();
              }}
              onDeleteSession={deleteSession}
              onFile={(file) => {
                void openFile(file);
                props.collapseAfterSelection();
              }}
              onFileTreeDirectory={toggleFileTreeDirectory}
              onUp={() => loadFiles(parentPath(state().filePath))}
              onSettings={openAppearanceSettings}
            />
            <button class="settings-entry" type="button" onClick={openAppearanceSettings}>
              {t("settings")}
            </button>
          </>
        }
      >
        <SettingsRail
          active={state().settingsSection}
          onBack={closeSettings}
          onSection={(settingsSection) => {
            setState((previous) => ({ ...previous, settingsSection }));
            props.collapseAfterSelection();
          }}
        />
      </Show>
    </aside>
  );
}
