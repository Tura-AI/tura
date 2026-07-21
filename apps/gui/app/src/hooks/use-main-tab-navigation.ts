import type { Accessor, Setter } from "solid-js";
import { clearLastSessionOpened, readLastSessionOpened } from "../app-state-utils";
import type { AppState, MainTab, SettingsSection } from "../state/global-store";

export function useMainTabNavigation(options: {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  refreshProviderSurface: () => Promise<void>;
  openBlankSession: () => void;
  openSession: (sessionId: string) => Promise<void>;
  loadFiles: (path?: string) => Promise<void>;
  e2eFixture?: string;
}) {
  const {
    state,
    setState,
    refreshProviderSurface,
    openBlankSession,
    openSession,
    loadFiles,
    e2eFixture,
  } = options;

  function openSettings(section: SettingsSection = state().settingsSection) {
    setState((previous) => ({
      ...previous,
      previousMainTab:
        previous.activeTab === "settings" ? previous.previousMainTab : previous.activeTab,
      activeTab: "settings",
      settingsSection: section,
      settingsNotice: undefined,
    }));
    if (!e2eFixture) {
      void refreshProviderSurface();
    }
  }

  function closeSettings() {
    setState((previous) => ({
      ...previous,
      activeTab: previous.previousMainTab,
      settingsNotice: undefined,
    }));
  }

  async function changeMainTab(activeTab: Exclude<MainTab, "settings">) {
    const lastSessionId =
      state().lastSessionOpenedId ??
      readLastSessionOpened() ??
      (state().activeTab === "conversation"
        ? undefined
        : (state().selectedSessionId ?? state().planPreviewSessionId));
    const lastSession = lastSessionId
      ? state().sessions.find((session) => session.id === lastSessionId)
      : undefined;

    if (activeTab === "conversation") {
      if (state().activeTab === "conversation") {
        openBlankSession();
        return;
      }
      if (!lastSessionId || !lastSession) {
        if (lastSessionId) {
          clearLastSessionOpened();
        }
        setState((previous) => ({
          ...previous,
          lastSessionOpenedId: undefined,
        }));
        openBlankSession();
        return;
      }
      setState((previous) => ({
        ...previous,
        activeTab: "conversation",
        previousMainTab: "conversation",
        selectedSessionId: lastSessionId,
        error: undefined,
      }));
      await openSession(lastSessionId);
      return;
    }

    if (activeTab === "plan") {
      if (lastSessionId && !lastSession) {
        clearLastSessionOpened();
        setState((previous) => ({
          ...previous,
          lastSessionOpenedId: undefined,
        }));
      }
      setState((previous) => ({
        ...previous,
        activeTab: "plan",
        previousMainTab: "plan",
        selectedSessionId: lastSession?.id ?? previous.selectedSessionId,
        planPreviewSessionId: lastSession?.id ?? previous.planPreviewSessionId,
        error: undefined,
      }));
      if (lastSession) {
        await openSession(lastSession.id);
      }
      return;
    }

    const selectedSessionId = state().selectedSessionId;
    setState((previous) => ({
      ...previous,
      activeTab,
      previousMainTab: activeTab,
      selectedSessionId,
    }));
    if (activeTab === "files" && state().files.length === 0) {
      void loadFiles("");
    }
  }

  return {
    openSettings,
    closeSettings,
    changeMainTab,
  };
}
