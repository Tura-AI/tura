import type { Session, TaskManagement } from "@tura/gateway-sdk";
import { Match, Show, Switch, createEffect, createSignal, onCleanup } from "solid-js";
import { cornerRadiusScale } from "../app-state-utils";
import { DEFAULT_CODE_FONT, DEFAULT_MAIN_FONT } from "../config/defaults";
import { sessionTasks, taskDisplayText, taskNonceId } from "../features/plan/tasks";
import { t } from "../i18n";
import { classNames } from "../state/format";
import type { AppState } from "../state/global-store";
import { AppRail } from "./app-rail";
import type { AppShellViewModel } from "./app-shell-view-model";
import { ConversationPageOutlet } from "./conversation-page-outlet";
import { FilesPageOutlet } from "./files-page-outlet";
import { AppLoadingPlaceholder, GatewayConnectionLoadingOverlay } from "./loading-placeholders";
import { PlanPageOutlet } from "./plan-page-outlet";
import { ProviderAuthPortal } from "./provider-auth-portal";
import { SettingsPageOutlet } from "./settings-page-outlet";
import { AppTitleBar, ErrorStrip, RailToggleButton } from "./shell-chrome";
import { useIdleScrollbars } from "./use-idle-scrollbars";
import { useRailLayout } from "./use-rail-layout";

export function AppShell(props: { view: AppShellViewModel }) {
  const {
    state,
    selectedSession,
    slashCommands,
    setState,
    submitPrompt,
    queuePrompt,
    runPlanTaskNow,
    updatePlanTicketTask,
    updateEditingTaskFromComposer,
    openSettings,
    saveRuntimeSettings,
    updateModelTier,
    refreshAgents,
    getAgent,
    saveAgent,
    deleteAgent,
    saveProviderKey,
    startProviderLogin,
    completeProviderLogin,
    logoutProvider,
  } = props.view;
  const openProviderSettings = (providerId?: string) =>
    setState((previous) => ({
      ...previous,
      activeTab: "settings",
      previousMainTab:
        previous.activeTab === "settings" ? previous.previousMainTab : previous.activeTab,
      settingsSection: "providers",
      selectedProviderId: providerId ?? previous.selectedProviderId,
      planNotice: undefined,
    }));
  const [conversationInspector, setConversationInspector] = createSignal({
    open: false,
    overlay: false,
    width: 0,
  });
  const [planPanelLayout, setPlanPanelLayout] = createSignal({
    open: false,
    overlay: false,
    width: 0,
  });
  const [conversationInspectorCloseToken, setConversationInspectorCloseToken] = createSignal(0);
  let settingsSaveTimer: number | undefined;
  function closeActiveRightSidebar() {
    if (state().activeTab === "conversation") {
      setConversationInspectorCloseToken((token) => token + 1);
      setConversationInspector({ open: false, overlay: false, width: 0 });
      return;
    }
    if (state().activeTab === "plan") {
      setState((previous) => ({
        ...previous,
        planPreviewSessionId: undefined,
        planDraftLane: undefined,
        planDraftSessionId: undefined,
        editingTask: undefined,
      }));
      setPlanPanelLayout({ open: false, overlay: false, width: 0 });
    }
  }
  const {
    railWidth,
    railCollapsed,
    railDragging,
    railFullscreen,
    openRail,
    collapseRailForMainWidth,
    collapseRailAfterCompactSelection,
    beginRailResize,
    beginRailMouseResize,
  } = useRailLayout({
    activeTab: () => state().activeTab,
    rightSidebarOpen: () =>
      state().activeTab === "conversation"
        ? conversationInspector().open || conversationInspector().overlay
        : state().activeTab === "plan"
          ? planPanelLayout().open || planPanelLayout().overlay
          : false,
    rightSidebarWidth: () =>
      state().activeTab === "conversation" && conversationInspector().open
        ? conversationInspector().width
        : state().activeTab === "plan" && planPanelLayout().open
          ? planPanelLayout().width
          : 0,
    closeRightSidebar: closeActiveRightSidebar,
  });

  useIdleScrollbars();

  createEffect(() => {
    document.documentElement.style.setProperty(
      "--corner-radius-scale",
      String(cornerRadiusScale(state().cornerRadius)),
    );
  });
  onCleanup(() => document.documentElement.style.removeProperty("--corner-radius-scale"));

  function showGatewayLoadingOverlay() {
    return !state().bootstrapped && state().connection !== "connected" && !state().error;
  }

  function applyRuntimeSetting(
    updater: (previous: AppState) => AppState,
    options: { debounce?: boolean } = {},
  ) {
    setState(updater);
    if (settingsSaveTimer) {
      window.clearTimeout(settingsSaveTimer);
      settingsSaveTimer = undefined;
    }
    if (options.debounce) {
      settingsSaveTimer = window.setTimeout(() => {
        settingsSaveTimer = undefined;
        void saveRuntimeSettings();
      }, 320);
      return;
    }
    void saveRuntimeSettings();
  }

  function editComposerTask(
    sessionId: string,
    taskNonceIdValue: string | undefined,
    composerText: string,
  ) {
    const editing = state().editingTask;
    const currentComposerText = state().composerText;
    if (editing && editing.sessionId === sessionId && editing.task_id === taskNonceIdValue) {
      setState((previous) => ({
        ...previous,
        composerText: "",
        editingTask: undefined,
      }));
      return;
    }
    if (editing) {
      persistEditedTaskText(editing, currentComposerText);
    }
    setState((previous) => ({
      ...previous,
      composerText,
      editingTask: {
        sessionId,
        task_id: taskNonceIdValue,
      },
    }));
  }

  function persistEditedTaskText(
    editing: { sessionId: string; task_id?: string },
    textValue: string,
  ) {
    const session = state().sessions.find((item) => item.id === editing.sessionId);
    const text = textValue.trim();
    if (!session || !text) {
      return;
    }
    const task = sessionTasks(session).find((item) => taskNonceId(item) === editing.task_id);
    if (task && taskDisplayText(task).trim() === text) {
      return;
    }
    const [summaryLine = "", ...deliverableLines] = text.split(/\r?\n/u);
    void updatePlanTicketTask(session, {
      task_id: editing.task_id,
      task_summary: summaryLine.trim(),
      deliverable: deliverableLines.join("\n").trim(),
    });
  }

  function selectedEditingTask() {
    const session = selectedSession();
    const editing = state().editingTask;
    if (!session || !editing || editing.sessionId !== session.id) {
      return undefined;
    }
    return sessionTasks(session).find((task) => taskNonceId(task) === editing.task_id);
  }

  function taskWithComposerText(task: TaskManagement, textValue: string): TaskManagement {
    const text = textValue.trim();
    if (!text) {
      return task;
    }
    const [summaryLine = "", ...deliverableLines] = text.split(/\r?\n/u);
    return {
      ...task,
      task_summary: summaryLine.trim(),
      deliverable: deliverableLines.join("\n").trim(),
    };
  }

  async function runEditingTaskNow(session: Session, task: TaskManagement) {
    const editing = state().editingTask;
    const editingThisTask =
      editing?.sessionId === session.id && editing.task_id === taskNonceId(task);
    if (!editingThisTask) {
      await runPlanTaskNow(session, task);
      return;
    }
    const nextTask = taskWithComposerText(task, state().composerText);
    if (taskDisplayText(nextTask).trim() !== taskDisplayText(task).trim()) {
      await updatePlanTicketTask(session, {
        task_id: taskNonceId(task),
        task_summary: nextTask.task_summary,
        deliverable: nextTask.deliverable,
      });
    }
    await runPlanTaskNow(session, nextTask);
    setState((previous) => ({
      ...previous,
      composerText: "",
      editingTask: undefined,
      planDraftStartCondition: "user_action",
    }));
  }

  async function submitCurrentComposer() {
    if (state().editingTask) {
      await updateEditingTaskFromComposer();
      return;
    }
    if (state().planDraftStartCondition === "session_idle") {
      await queuePrompt();
      return;
    }
    await submitPrompt();
  }

  async function queueCurrentComposer() {
    if (state().editingTask) {
      await updateEditingTaskFromComposer();
      return;
    }
    await queuePrompt();
  }

  return (
    <>
      <AppTitleBar />
      <main
        class={classNames(
          "workbench",
          railCollapsed() && "rail-collapsed",
          railFullscreen() && "rail-fullscreen",
          state().activeTab === "conversation" &&
            conversationInspector().overlay &&
            "inspector-overlay-open",
          ((state().activeTab === "conversation" && conversationInspector().overlay) ||
            (state().activeTab === "plan" && planPanelLayout().overlay)) &&
            "right-overlay-open",
          railDragging() && "rail-resizing",
          state().activeTab === "settings" && "settings-workbench",
        )}
        style={{
          "--rail": `${railWidth()}px`,
          "--app-font-family": state().mainFont || DEFAULT_MAIN_FONT,
          "--code-font-family": state().codeFont || DEFAULT_CODE_FONT,
          "--base-font-size": `${state().mainFontSize || 12}px`,
          "--code-font-size": `${state().codeFontSize || 12}px`,
          "--corner-radius-scale": String(cornerRadiusScale(state().cornerRadius)),
        }}
      >
        <AppRail view={props.view} collapseAfterSelection={collapseRailAfterCompactSelection} />
        <div
          class="rail-resize-handle"
          role="separator"
          aria-orientation="vertical"
          aria-label={t("sidebar")}
          onPointerDown={beginRailResize}
          onMouseDown={beginRailMouseResize}
        />

        <section class="main-column">
          <RailToggleButton
            collapsed={railCollapsed()}
            onToggle={() => (railCollapsed() ? openRail() : collapseRailForMainWidth())}
          />
          <ErrorStrip error={state().error} notice={state().settingsNotice} setState={setState} />
          <Show
            when={!state().loading}
            fallback={
              <Show when={!showGatewayLoadingOverlay()}>
                <AppLoadingPlaceholder
                  activeTab={state().activeTab}
                  settingsSection={state().settingsSection}
                />
              </Show>
            }
          >
            <Switch>
              <Match when={state().activeTab === "plan"}>
                <PlanPageOutlet
                  state={state()}
                  setState={setState}
                  previewSession={state().sessions.find(
                    (session) => session.id === state().planPreviewSessionId,
                  )}
                  previewMessages={
                    state().planPreviewSessionId
                      ? (state().messagesBySession[state().planPreviewSessionId!] ?? [])
                      : []
                  }
                  slashCommands={slashCommands()}
                  view={props.view}
                  onEditTask={(session, task, composerText) =>
                    editComposerTask(session.id, taskNonceId(task), composerText)
                  }
                  onRunTask={(session, task) => void runEditingTaskNow(session, task)}
                  onSubmit={() => void submitCurrentComposer()}
                  onQueueSubmit={() => void queueCurrentComposer()}
                  onOpenProviderSettings={openProviderSettings}
                  leftRailOpen={!railCollapsed()}
                  leftRailWidth={railFullscreen() ? 0 : railWidth()}
                  onRequestCollapseLeftRail={collapseRailForMainWidth}
                  onPanelLayout={setPlanPanelLayout}
                  onRuntimeSetting={applyRuntimeSetting}
                  onOpenSettings={openSettings}
                />
              </Match>
              <Match when={state().activeTab === "files"}>
                <FilesPageOutlet state={state()} setState={setState} view={props.view} />
              </Match>
              <Match when={state().activeTab === "conversation"}>
                <ConversationPageOutlet
                  state={props.view.state}
                  setState={props.view.setState}
                  selectedSession={props.view.selectedSession}
                  selectedMessages={props.view.selectedMessages}
                  loadEarlierMessages={props.view.loadEarlierMessages}
                  slashCommands={props.view.slashCommands}
                  selectedEditingTask={selectedEditingTask}
                  leftRailOpen={!railCollapsed()}
                  leftRailWidth={railFullscreen() ? 0 : railWidth()}
                  view={props.view}
                  onSubmit={() => void submitCurrentComposer()}
                  onQueueSubmit={() => void queueCurrentComposer()}
                  onInspectorLayout={setConversationInspector}
                  closeInspectorSignal={conversationInspectorCloseToken()}
                  onRequestCollapseLeftRail={collapseRailForMainWidth}
                  onOpenProviderSettings={openProviderSettings}
                  onRunTask={(session, task) => void runEditingTaskNow(session, task)}
                  onRuntimeSetting={applyRuntimeSetting}
                  onOpenSettings={openSettings}
                />
              </Match>
              <Match when={state().activeTab === "settings"}>
                <SettingsPageOutlet
                  state={state()}
                  setState={setState}
                  onModelTier={updateModelTier}
                  onRuntimeSetting={applyRuntimeSetting}
                  onRefreshAgents={refreshAgents}
                  onGetAgent={getAgent}
                  onSaveAgent={saveAgent}
                  onDeleteAgent={deleteAgent}
                />
              </Match>
            </Switch>
          </Show>
        </section>
        <Show when={showGatewayLoadingOverlay()}>
          <GatewayConnectionLoadingOverlay
            activeTab={state().activeTab}
            settingsSection={state().settingsSection}
            notice={state().gatewayStartupNotice}
          />
        </Show>
      </main>
      <Show when={state().providerAuthPanel}>
        {(panel) => (
          <ProviderAuthPortal
            state={state()}
            panel={panel()}
            setState={setState}
            onSaveKey={saveProviderKey}
            onStartLogin={startProviderLogin}
            onCompleteLogin={completeProviderLogin}
            onLogout={logoutProvider}
          />
        )}
      </Show>
    </>
  );
}
