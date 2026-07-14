import { For, Match, Switch } from "solid-js";
import { t } from "../i18n";
import { classNames } from "../state/format";
import type { AppState } from "../state/global-store";

export function GatewayConnectionLoadingOverlay(props: {
  activeTab: AppState["activeTab"];
  settingsSection: AppState["settingsSection"];
  notice?: string;
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
                class={classNames("loading-bar", item === 0 && "medium", item !== 0 && "short")}
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
        <div class="gateway-loading-status" role="status">
          {props.notice || t("loading")}
        </div>
        <AppLoadingPlaceholder
          activeTab={props.activeTab}
          settingsSection={props.settingsSection}
        />
      </div>
    </section>
  );
}

export function AppLoadingPlaceholder(props: {
  activeTab: AppState["activeTab"];
  settingsSection: AppState["settingsSection"];
}) {
  return (
    <Switch fallback={<ConversationLoadingPlaceholder />}>
      <Match when={props.activeTab === "settings"}>
        <section class="settings-view layered-page layered-page-two" aria-label={t("loading")}>
          <header class="page-head page-layer-inner">
            <div class="page-title">
              <span>{t("settings")}</span>
              <h1>{settingsSectionTitle(props.settingsSection)}</h1>
            </div>
          </header>
          <main class="settings-canvas page-layer-middle">
            <section class="settings-stack page-layer-inner">
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
        <section class="product-workbench plan-workbench" aria-label={t("loading")}>
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
        <section class="files-view layered-page layered-page-two" aria-label={t("loading")}>
          <header class="page-head page-layer-inner">
            <div class="page-title">
              <span>{t("fileBrowser")}</span>
              <h1>
                <div class="loading-bar medium" />
              </h1>
            </div>
          </header>
          <main class="file-canvas page-layer-middle">
            <div class="file-canvas-inner page-layer-inner">
              <section class="surface-list-panel">
                <div class="surface-list-head file-list-head">
                  <span>{t("name")}</span>
                  <span>{t("git")}</span>
                  <span>{t("size")}</span>
                  <span>{t("modifiedAt")}</span>
                </div>
                <For each={[0, 1, 2, 3, 4, 5]}>
                  {() => (
                    <div class="surface-list-row file-list-row loading-list-row">
                      <div class="loading-bar wide" />
                      <div class="loading-bar short" />
                      <div class="loading-bar short" />
                      <div class="loading-bar medium" />
                    </div>
                  )}
                </For>
              </section>
            </div>
          </main>
        </section>
      </Match>
      <Match when={props.activeTab === "conversation"}>
        <ConversationLoadingPlaceholder />
      </Match>
    </Switch>
  );
}

export function ConversationLoadingPlaceholder() {
  return (
    <section class="conversation-view" aria-label={t("loading")}>
      <div class="conversation-grid">
        <div class="conversation-main">
          <div class="transcript">
            <div class="transcript-inner page-layer-inner">
              <TranscriptTextLoadingLines />
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

export function TranscriptTextLoadingLines() {
  return (
    <article class="message assistant transcript-loading-placeholder" aria-hidden="true">
      <div class="message-body">
        <div class="assistant-response">
          <div class="message-avatar-wrap" />
          <div class="assistant-stack assistant-text">
            <div class="assistant-text-block">
              <div class="part text-part">
                <div class="rich-text">
                  <span class="loading-bar text-loading-line" />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </article>
  );
}

function settingsSectionTitle(section: AppState["settingsSection"]): string {
  const labels: Record<AppState["settingsSection"], string> = {
    application: t("applicationSettings"),
    appearance: t("appearance"),
    providers: t("providers"),
    models: t("models"),
    agents: t("agentSettings"),
    personalization: t("personalization"),
    about: t("about"),
  };
  return labels[section];
}
