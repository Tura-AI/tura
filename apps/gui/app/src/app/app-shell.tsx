import {
  For,
  Show,
  Switch,
  Match,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
  type Accessor,
  type Setter,
} from "solid-js";
import { Portal } from "solid-js/web";
import type { Session, TaskManagement } from "@tura/gateway-sdk";
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
  PlanConversationFeedbackNotice,
  PlanComposerTaskList,
} from "../pages/plan/plan-composer";
import type { AppState } from "../state/global-store";
import { parentPath } from "../utils/app-format";
import {
  defaultLocalStartAt,
  defaultPollInterval,
  hasVisibleSessionTasks,
  localDateTimeToUtcIso,
  sessionTasks,
  taskNonceId,
  taskPollInterval,
  taskStartCondition,
  taskDisplayText,
  timedTaskPatch,
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
const CONVERSATION_MAIN_MIN_WIDTH = 430;
const DEFAULT_MAIN_FONT =
  '"Microsoft YaHei", "PingFang SC", "PingFang TC", "Segoe UI", Arial, "Nirmala UI", "Segoe UI Arabic", "Noto Sans Bengali", "Yu Gothic UI", ui-sans-serif, system-ui, sans-serif';
const DEFAULT_CODE_FONT =
  'ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace';

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
    runPlanTaskNow,
    updatePlanTicketTask,
    updateEditingTaskFromComposer,
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
    updateModelTier,
    saveProviderKey,
    startProviderLogin,
    completeProviderLogin,
    logoutProvider,
    e2eFixture,
    providerAuthPanel,
  } = props.view;
  const openProviderSettings = (providerId?: string) =>
    setState((previous) => ({
      ...previous,
      activeTab: "settings",
      previousMainTab:
        previous.activeTab === "settings"
          ? previous.previousMainTab
          : previous.activeTab,
      settingsSection: "providers",
      selectedProviderId: providerId ?? previous.selectedProviderId,
      planNotice: undefined,
    }));
  const [railWidth, setRailWidth] = createSignal(RAIL_DEFAULT_WIDTH);
  const [lastRailWidth, setLastRailWidth] = createSignal(RAIL_DEFAULT_WIDTH);
  const [railCollapsed, setRailCollapsed] = createSignal(
    typeof window !== "undefined" &&
      window.matchMedia("(max-width: 760px)").matches,
  );
  const [railDragging, setRailDragging] = createSignal(false);
  const [viewportWidth, setViewportWidth] = createSignal(
    typeof window === "undefined" ? 0 : window.innerWidth,
  );
  const [conversationInspector, setConversationInspector] = createSignal({
    open: false,
    width: 0,
  });
  const [forceRailFullscreen, setForceRailFullscreen] = createSignal(false);
  let settingsSaveTimer: number | undefined;

  onMount(() => {
    const resize = () => setViewportWidth(window.innerWidth);
    const scrollbarTimers = new Map<HTMLElement, number>();
    const scrollbarPointerElements = new Set<HTMLElement>();
    const hideClass = "scrollbar-idle-hidden";
    const scrollOptions = { capture: true };
    const pointerOptions = { capture: true };

    function scrollingElementFromTarget(target: EventTarget | null) {
      if (target === document) {
        return document.scrollingElement as HTMLElement | null;
      }
      return target instanceof HTMLElement ? target : null;
    }

    function clearScrollbarTimer(element: HTMLElement) {
      const timer = scrollbarTimers.get(element);
      if (timer) {
        window.clearTimeout(timer);
        scrollbarTimers.delete(element);
      }
    }

    function canScrollVertically(element: HTMLElement) {
      return element.scrollHeight - element.clientHeight > 2;
    }

    function isAtScrollBottom(element: HTMLElement) {
      return element.scrollHeight - element.scrollTop - element.clientHeight <= 2;
    }

    function scheduleScrollbarHide(element: HTMLElement) {
      clearScrollbarTimer(element);
      element.classList.remove(hideClass);
      if (
        !canScrollVertically(element) ||
        !isAtScrollBottom(element) ||
        scrollbarPointerElements.has(element)
      ) {
        return;
      }
      const timer = window.setTimeout(() => {
        scrollbarTimers.delete(element);
        if (
          isAtScrollBottom(element) &&
          !scrollbarPointerElements.has(element)
        ) {
          element.classList.add(hideClass);
        }
      }, 5000);
      scrollbarTimers.set(element, timer);
    }

    function handleScrollableIdle(event: Event) {
      const element = scrollingElementFromTarget(event.target);
      if (!element) {
        return;
      }
      scheduleScrollbarHide(element);
    }

    function scrollableElementFromPoint(target: EventTarget | null) {
      let element = target instanceof HTMLElement ? target : null;
      while (element && element !== document.body) {
        if (canScrollVertically(element)) {
          return element;
        }
        element = element.parentElement;
      }
      return document.scrollingElement as HTMLElement | null;
    }

    function pointerInVerticalScrollbar(
      element: HTMLElement,
      event: PointerEvent,
    ) {
      if (!canScrollVertically(element)) {
        return false;
      }
      const rect = element.getBoundingClientRect();
      const scrollbarWidth = Math.max(12, element.offsetWidth - element.clientWidth);
      return (
        event.clientX >= rect.right - scrollbarWidth - 2 &&
        event.clientX <= rect.right + 2 &&
        event.clientY >= rect.top &&
        event.clientY <= rect.bottom
      );
    }

    function handleScrollbarPointerMove(event: PointerEvent) {
      const current = scrollableElementFromPoint(event.target);
      for (const element of Array.from(scrollbarPointerElements)) {
        if (element !== current || !pointerInVerticalScrollbar(element, event)) {
          scrollbarPointerElements.delete(element);
          scheduleScrollbarHide(element);
        }
      }
      if (!current) {
        return;
      }
      if (pointerInVerticalScrollbar(current, event)) {
        scrollbarPointerElements.add(current);
        clearScrollbarTimer(current);
        current.classList.remove(hideClass);
      }
    }

    window.addEventListener("resize", resize);
    document.addEventListener("scroll", handleScrollableIdle, scrollOptions);
    document.addEventListener(
      "pointermove",
      handleScrollbarPointerMove,
      pointerOptions,
    );
    onCleanup(() => {
      window.removeEventListener("resize", resize);
      document.removeEventListener(
        "scroll",
        handleScrollableIdle,
        scrollOptions,
      );
      document.removeEventListener(
        "pointermove",
        handleScrollbarPointerMove,
        pointerOptions,
      );
      for (const timer of scrollbarTimers.values()) {
        window.clearTimeout(timer);
      }
    });
  });

  const railFullscreen = createMemo(() => {
    if (railCollapsed() || state().activeTab !== "conversation") {
      return false;
    }
    return forceRailFullscreen();
  });

  createEffect(() => {
    if (railCollapsed() || state().activeTab !== "conversation") {
      setForceRailFullscreen(false);
    }
  });

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
    if (
      editing &&
      editing.sessionId === sessionId &&
      editing.nonce_id === taskNonceIdValue
    ) {
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
        nonce_id: taskNonceIdValue,
      },
    }));
  }

  function persistEditedTaskText(
    editing: { sessionId: string; nonce_id?: string },
    textValue: string,
  ) {
    const session = state().sessions.find(
      (item) => item.id === editing.sessionId,
    );
    const text = textValue.trim();
    if (!session || !text) {
      return;
    }
    const task = sessionTasks(session).find(
      (item) => taskNonceId(item) === editing.nonce_id,
    );
    if (task && taskDisplayText(task).trim() === text) {
      return;
    }
    const [summaryLine = "", ...deliveryLines] = text.split(/\r?\n/u);
    void updatePlanTicketTask(session, {
      nonce_id: editing.nonce_id,
      task_summary: summaryLine.trim(),
      delivery: deliveryLines.join("\n").trim(),
    });
  }

  function selectedEditingTask() {
    const session = selectedSession();
    const editing = state().editingTask;
    if (!session || !editing || editing.sessionId !== session.id) {
      return undefined;
    }
    return sessionTasks(session).find(
      (task) => taskNonceId(task) === editing.nonce_id,
    );
  }

  function taskWithComposerText(
    task: TaskManagement,
    textValue: string,
  ): TaskManagement {
    const text = textValue.trim();
    if (!text) {
      return task;
    }
    const [summaryLine = "", ...deliveryLines] = text.split(/\r?\n/u);
    return {
      ...task,
      task_summary: summaryLine.trim(),
      delivery: deliveryLines.join("\n").trim(),
    };
  }

  async function runEditingTaskNow(session: Session, task: TaskManagement) {
    const editing = state().editingTask;
    const editingThisTask =
      editing?.sessionId === session.id &&
      editing.nonce_id === taskNonceId(task);
    if (!editingThisTask) {
      await runPlanTaskNow(session, task);
      return;
    }
    const nextTask = taskWithComposerText(task, state().composerText);
    if (taskDisplayText(nextTask).trim() !== taskDisplayText(task).trim()) {
      await updatePlanTicketTask(session, {
        nonce_id: taskNonceId(task),
        task_summary: nextTask.task_summary,
        delivery: nextTask.delivery,
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
    await submitPrompt();
  }

  function openRail() {
    const preferredWidth = Math.min(
      RAIL_MAX_WIDTH,
      Math.max(RAIL_MIN_WIDTH, lastRailWidth()),
    );
    const maxWidth = maxRailWidth();
    const width = Math.min(preferredWidth, Math.max(RAIL_MIN_WIDTH, maxWidth));
    setRailWidth(width);
    setLastRailWidth(width);
    setForceRailFullscreen(maxWidth < RAIL_MIN_WIDTH);
    setRailCollapsed(false);
  }

  function collapseRailForMainWidth() {
    setRailCollapsed(true);
    setRailWidth(0);
    setForceRailFullscreen(false);
  }

  function rightConversationSidebarWidth() {
    return state().activeTab === "conversation" && conversationInspector().open
      ? conversationInspector().width
      : 0;
  }

  function maxRailWidth() {
    if (state().activeTab !== "conversation") {
      return RAIL_MAX_WIDTH;
    }
    return Math.min(
      RAIL_MAX_WIDTH,
      Math.max(
        0,
        viewportWidth() -
          rightConversationSidebarWidth() -
          CONVERSATION_MAIN_MIN_WIDTH,
      ),
    );
  }

  function collapseRailAfterCompactSelection() {
    if (
      railFullscreen() ||
      (typeof window !== "undefined" &&
        window.matchMedia("(max-width: 760px)").matches)
    ) {
      setRailCollapsed(true);
      setRailWidth(0);
      setForceRailFullscreen(false);
    }
  }

  function previewRailResize(clientX: number) {
    if (clientX <= RAIL_COLLAPSE_WIDTH) {
      setRailCollapsed(true);
      setRailWidth(0);
      setForceRailFullscreen(false);
      return;
    }
    setForceRailFullscreen(false);
    setRailCollapsed(false);
    const maxWidth = maxRailWidth();
    if (maxWidth < RAIL_MIN_WIDTH) {
      setRailCollapsed(true);
      setRailWidth(0);
      return;
    }
    setRailWidth(Math.min(maxWidth, Math.max(RAIL_MIN_WIDTH, clientX)));
  }

  function commitRailResize(clientX: number) {
    if (clientX <= RAIL_COLLAPSE_WIDTH) {
      setRailCollapsed(true);
      setRailWidth(0);
      setForceRailFullscreen(false);
      return;
    }
    const maxWidth = maxRailWidth();
    if (maxWidth < RAIL_MIN_WIDTH) {
      setRailCollapsed(true);
      setRailWidth(0);
      setForceRailFullscreen(false);
      return;
    }
    const nextWidth = Math.min(maxWidth, Math.max(RAIL_MIN_WIDTH, clientX));
    setRailWidth(nextWidth);
    setLastRailWidth(nextWidth);
    setRailCollapsed(false);
    setForceRailFullscreen(false);
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
          railFullscreen() && "rail-fullscreen",
          railDragging() && "rail-resizing",
          state().activeTab === "settings" && "settings-workbench",
        )}
        style={{
          "--rail": `${railWidth()}px`,
          "--app-font-family": state().mainFont || DEFAULT_MAIN_FONT,
          "--code-font-family": state().codeFont || DEFAULT_CODE_FONT,
          "--base-font-size": `${state().mainFontSize || 13}px`,
          "--code-font-size": `${state().codeFontSize || 12}px`,
        }}
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
                    openSettings("appearance");
                    collapseRailAfterCompactSelection();
                  }}
                />
                <button
                  class="settings-entry"
                  type="button"
                  onClick={() => {
                    openSettings("appearance");
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
              onSection={(settingsSection) => {
                setState((previous) => ({ ...previous, settingsSection }));
                collapseRailAfterCompactSelection();
              }}
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
              <div class="error-strip" role="alert">
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
          <Show
            when={!state().loading}
            fallback={
              <AppLoadingPlaceholder
                activeTab={state().activeTab}
                settingsSection={state().settingsSection}
              />
            }
          >
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
                  onSubmit={() => void submitCurrentComposer()}
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
                      planDraftSessionId: undefined,
                      planPreviewSessionId: undefined,
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
                    editComposerTask(
                      session.id,
                      taskNonceId(task),
                      composerText,
                    )
                  }
                  onDeleteTask={deletePlanTask}
                  onRunTask={(session, task) =>
                    void runEditingTaskNow(session, task)
                  }
                  onCreateSessionFromTask={createSessionFromPlanTask}
                  onOpenSession={openPlanSession}
                  onComposerText={(composerText) =>
                    setState((previous) => ({ ...previous, composerText }))
                  }
                  onComposerImages={(composerImages) =>
                    setState((previous) => ({ ...previous, composerImages }))
                  }
                  onSubmit={() => void submitCurrentComposer()}
                  onOpenProviderSettings={openProviderSettings}
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
                  onSubmit={() => void submitCurrentComposer()}
                  leftRailOpen={!railCollapsed()}
                  leftRailWidth={railFullscreen() ? 0 : railWidth()}
                  onRequestCollapseLeftRail={collapseRailForMainWidth}
                  onInspectorLayout={setConversationInspector}
                  conversationNotice={
                    state().planNotice ? (
                      <PlanConversationFeedbackNotice
                        message={state().planNotice?.message}
                        code={state().planNotice?.code}
                        providerId={state().planNotice?.providerId}
                        onOpenProviderSettings={openProviderSettings}
                      />
                    ) : undefined
                  }
                  composerToolbar={
                    selectedSession() && selectedEditingTask() ? (
                      <PlanComposerControls
                        startCondition={taskStartCondition(
                          selectedEditingTask()!,
                        )}
                        startAt={utcIsoToLocalDateTime(
                          selectedEditingTask()!.start_at,
                        )}
                        pollInterval={
                          selectedEditingTask()!.poll_interval ??
                          defaultPollInterval()
                        }
                        onStartCondition={(start_condition) => {
                          const task = selectedEditingTask()!;
                          if (start_condition === "user_action") {
                            void runEditingTaskNow(selectedSession()!, task);
                            return;
                          }
                          const startAt =
                            localDateTimeToUtcIso(
                              utcIsoToLocalDateTime(task.start_at) ||
                                defaultLocalStartAt(),
                            ) ?? localDateTimeToUtcIso(defaultLocalStartAt());
                          void updatePlanTicketTask(selectedSession()!, {
                            nonce_id: taskNonceId(task),
                            status: "todo",
                            ...timedTaskPatch(
                              start_condition,
                              startAt,
                              taskPollInterval(task),
                            ),
                          });
                        }}
                        onStartAt={(value) => {
                          const start_at = localDateTimeToUtcIso(value);
                          if (start_at) {
                            void updatePlanTicketTask(selectedSession()!, {
                              nonce_id: taskNonceId(selectedEditingTask()!),
                              start_at,
                            });
                          }
                        }}
                        onPollInterval={(poll_interval) =>
                          updatePlanTicketTask(selectedSession()!, {
                            nonce_id: taskNonceId(selectedEditingTask()!),
                            poll_interval,
                          })
                        }
                      />
                    ) : selectedSession() ? (
                      <PlanComposerControls
                        startCondition={state().planDraftStartCondition}
                        startAt={state().planDraftStartAt}
                        pollInterval={state().planDraftPollInterval}
                        onStartCondition={(planDraftStartCondition) =>
                          setState((previous) => ({
                            ...previous,
                            planDraftStartCondition,
                          }))
                        }
                        onStartAt={(planDraftStartAt) =>
                          setState((previous) => ({
                            ...previous,
                            planDraftStartAt,
                          }))
                        }
                        onPollInterval={(planDraftPollInterval) =>
                          setState((previous) => ({
                            ...previous,
                            planDraftPollInterval,
                          }))
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
                        pulseNonceId={
                          state().taskPulse?.sessionId === selectedSession()!.id
                            ? state().taskPulse?.nonce_id
                            : undefined
                        }
                        pulseToken={
                          state().taskPulse?.sessionId === selectedSession()!.id
                            ? state().taskPulse?.token
                            : undefined
                        }
                        onEdit={(task, composerText) =>
                          editComposerTask(
                            selectedSession()!.id,
                            taskNonceId(task),
                            composerText,
                          )
                        }
                        onDelete={(task) =>
                          deletePlanTask(selectedSession()!, task)
                        }
                        onRun={(task) =>
                          void runEditingTaskNow(selectedSession()!, task)
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
                  onModelTier={updateModelTier}
                  onConfigureProviders={() =>
                    setState((previous) => ({
                      ...previous,
                      settingsSection: "providers",
                    }))
                  }
                  onTheme={(themeMode) =>
                    applyRuntimeSetting((previous) => ({
                      ...previous,
                      themeMode,
                      configDraft: {
                        ...previous.configDraft,
                        theme: themeMode,
                      },
                    }))
                  }
                  onMainFont={(mainFont) =>
                    applyRuntimeSetting((previous) => ({
                      ...previous,
                      mainFont,
                      configDraft: {
                        ...previous.configDraft,
                        main_font: mainFont,
                      },
                    }))
                  }
                  onCodeFont={(codeFont) =>
                    applyRuntimeSetting((previous) => ({
                      ...previous,
                      codeFont,
                      configDraft: {
                        ...previous.configDraft,
                        code_font: codeFont,
                      },
                    }))
                  }
                  onMainFontSize={(mainFontSize) =>
                    applyRuntimeSetting((previous) => ({
                      ...previous,
                      mainFontSize,
                      configDraft: {
                        ...previous.configDraft,
                        main_font_size: String(mainFontSize),
                      },
                    }))
                  }
                  onCodeFontSize={(codeFontSize) =>
                    applyRuntimeSetting((previous) => ({
                      ...previous,
                      codeFontSize,
                      configDraft: {
                        ...previous.configDraft,
                        code_font_size: String(codeFontSize),
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
                />
              </Match>
            </Switch>
          </Show>
        </section>
        <Show when={state().connection !== "connected" && !state().error}>
          <GatewayConnectionLoadingOverlay
            activeTab={state().activeTab}
            settingsSection={state().settingsSection}
          />
        </Show>
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

function GatewayConnectionLoadingOverlay(props: {
  activeTab: AppState["activeTab"];
  settingsSection: AppState["settingsSection"];
}) {
  return (
    <section class="gateway-loading-overlay" aria-label={t("loading")}>
      <aside class="gateway-loading-rail">
        <div class="gateway-loading-brand">
          <div class="loading-bar short" />
        </div>
        <nav class="gateway-loading-nav">
          <For each={[0, 1, 2]}>
            {(item) => (
              <div
                class={classNames(
                  "loading-bar",
                  item === 0 && "medium",
                  item !== 0 && "short",
                )}
              />
            )}
          </For>
        </nav>
        <div class="gateway-loading-tree">
          <For each={[0, 1, 2, 3, 4, 5]}>
            {(item) => (
              <div
                class={classNames(
                  "loading-bar",
                  item % 2 === 0 && "wide",
                  item % 2 === 1 && "medium",
                )}
              />
            )}
          </For>
        </div>
        <div class="loading-bar medium" />
      </aside>
      <div class="gateway-loading-main">
        <AppLoadingPlaceholder
          activeTab={props.activeTab}
          settingsSection={props.settingsSection}
        />
      </div>
    </section>
  );
}

function AppLoadingPlaceholder(props: {
  activeTab: AppState["activeTab"];
  settingsSection: AppState["settingsSection"];
}) {
  return (
    <Switch
      fallback={<ConversationLoadingPlaceholder title={t("conversation")} />}
    >
      <Match when={props.activeTab === "new"}>
        <section class="new-session-view" aria-label={t("loading")}>
          <div class="new-session-center">
            <div class="loading-bar loading-title" />
            <div class="bottom-composer loading-composer">
              <div class="loading-bar wide" />
              <div class="loading-bar medium" />
              <div class="loading-composer-actions">
                <div class="loading-bar short" />
                <div class="loading-bar short" />
              </div>
            </div>
          </div>
        </section>
      </Match>
      <Match when={props.activeTab === "settings"}>
        <section class="settings-view" aria-label={t("loading")}>
          <header class="page-head">
            <div class="page-title">
              <span>{t("settings")}</span>
              <h1>{settingsSectionTitle(props.settingsSection)}</h1>
            </div>
          </header>
          <main class="settings-canvas">
            <section class="settings-stack">
              <section class="settings-panel">
                <header>
                  <span>{settingsSectionTitle(props.settingsSection)}</span>
                </header>
                <div class="settings-fields">
                  <For each={[0, 1, 2, 3, 4]}>
                    {(item) => (
                      <div class="field-row">
                        <div class="loading-bar short" />
                        <div
                          class={classNames(
                            "loading-bar",
                            item % 2 === 0 && "wide",
                            item % 2 === 1 && "medium",
                          )}
                        />
                      </div>
                    )}
                  </For>
                </div>
              </section>
            </section>
          </main>
        </section>
      </Match>
      <Match when={props.activeTab === "plan"}>
        <section
          class="product-workbench plan-workbench"
          aria-label={t("loading")}
        >
          <div class="plan-main">
            <header class="page-head plan-head">
              <div class="page-title">
                <span>{t("plan")}</span>
                <h1>
                  <div class="loading-bar medium" />
                </h1>
              </div>
              <div class="page-actions">
                <label class="search-box">
                  <div class="loading-bar wide" />
                </label>
              </div>
            </header>
            <main class="plan-board">
              <div class="board-shell">
                <div class="board-grid">
                  <For each={[0, 1, 2, 3]}>
                    {() => (
                      <section class="board-column">
                        <header>
                          <div class="loading-bar medium" />
                        </header>
                        <div class="loading-board-list">
                          <div class="loading-panel">
                            <div class="loading-bar wide" />
                            <div class="loading-bar medium" />
                          </div>
                          <div class="loading-panel">
                            <div class="loading-bar" />
                            <div class="loading-bar short" />
                          </div>
                        </div>
                      </section>
                    )}
                  </For>
                </div>
              </div>
            </main>
          </div>
        </section>
      </Match>
      <Match when={props.activeTab === "files"}>
        <section class="files-view" aria-label={t("loading")}>
          <header class="page-head">
            <div class="page-title">
              <span>{t("fileBrowser")}</span>
              <h1>
                <div class="loading-bar medium" />
              </h1>
            </div>
          </header>
          <main class="file-canvas">
            <section class="surface-list-panel">
              <div class="surface-list-head file-list-head">
                <span>{t("name")}</span>
                <span>{t("git")}</span>
                <span>{t("size")}</span>
                <span>{t("modifiedAt")}</span>
              </div>
              <For each={[0, 1, 2, 3, 4, 5]}>
                {(item) => (
                  <div class="surface-list-row file-list-row loading-list-row">
                    <div class="loading-bar wide" />
                    <div class="loading-bar short" />
                    <div class="loading-bar short" />
                    <div class="loading-bar medium" />
                  </div>
                )}
              </For>
            </section>
          </main>
        </section>
      </Match>
      <Match when={props.activeTab === "conversation"}>
        <ConversationLoadingPlaceholder title={t("conversation")} />
      </Match>
    </Switch>
  );
}

function ConversationLoadingPlaceholder(props: { title: string }) {
  return (
    <section class="conversation-view" aria-label={t("loading")}>
      <header class="page-head">
        <div class="page-title">
          <span>{props.title}</span>
          <h1>
            <div class="loading-bar medium" />
          </h1>
        </div>
      </header>
      <div class="conversation-grid">
        <div class="conversation-main">
          <div class="transcript-loading-placeholder">
            <div class="loading-bar wide" />
            <div class="loading-bar medium" />
            <div class="loading-bar" />
          </div>
          <div class="bottom-composer loading-composer">
            <div class="loading-bar wide" />
            <div class="loading-bar medium" />
          </div>
        </div>
      </div>
    </section>
  );
}

function settingsSectionTitle(section: AppState["settingsSection"]): string {
  const labels: Record<AppState["settingsSection"], string> = {
    appearance: t("appearance"),
    providers: t("providers"),
    models: t("models"),
  };
  return labels[section];
}
