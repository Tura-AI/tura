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
import FolderOpen from "lucide-solid/icons/folder-open";
import FolderSearch from "lucide-solid/icons/folder-search";
import KeyRound from "lucide-solid/icons/key-round";
import MessageSquare from "lucide-solid/icons/message-square";
import MoreHorizontal from "lucide-solid/icons/more-horizontal";
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
} from "../../conversation/conversation-view";
import { applyGatewayEvent } from "../../state/event-reducer";
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
} from "../../state/global-store";
import { classNames, truncate } from "../../state/format";
import { t, type TextKey } from "../../i18n";

import {
  ProviderConfigGroup,
  ProviderSelectMenu,
  ProviderAuthDialog,
} from "./provider-settings";
import {
  authStatusText,
  configFieldRows,
  modelRef,
  providerConfigured,
  providerIdFromModel,
  providerSourceLabel,
  providerStateLabel,
  settingsSections,
} from "../../utils/settings";
import { formatModelLimit, shortWorkspaceLabel } from "../../utils/app-format";
export function MainTabs(props: {
  active: Exclude<MainTab, "settings">;
  onChange: (tab: Exclude<MainTab, "settings">) => void;
}) {
  const tabs: Array<{
    id: Exclude<MainTab, "settings">;
    label: string;
    icon?: JSX.Element;
  }> = [
    { id: "new", label: t("session"), icon: <MessageSquare size={15} /> },
    { id: "plan", label: t("plan"), icon: <LayoutList size={15} /> },
    { id: "files", label: t("fileBrowser"), icon: <FolderSearch size={15} /> },
  ];
  return (
    <nav class="main-tabs">
      <For each={tabs}>
        {(item) => (
          <button
            class={classNames(props.active === item.id && "selected")}
            onClick={() => props.onChange(item.id)}
          >
            <Show when={item.icon}>{(icon) => icon()}</Show>
            <span>{item.label}</span>
          </button>
        )}
      </For>
    </nav>
  );
}

export function SettingsRail(props: {
  active: SettingsSection;
  onBack: () => void;
  onSection: (section: SettingsSection) => void;
}) {
  return (
    <nav class="settings-rail">
      <button class="settings-back" type="button" onClick={props.onBack}>
        <ArrowLeft size={15} strokeWidth={1.8} aria-hidden="true" />
        {t("backToApp")}
      </button>
      <div class="section-title">{t("settings")}</div>
      <div class="settings-section-list">
        <For each={settingsSections()}>
          {(item) => (
            <button
              class={classNames(props.active === item.id && "selected")}
              type="button"
              onClick={() => props.onSection(item.id)}
            >
              {item.label}
            </button>
          )}
        </For>
      </div>
    </nav>
  );
}

export function SettingsView(props: {
  state: AppState;
  section: SettingsSection;
  onProvider: (providerId: string) => void;
  onModel: (model: string) => void;
  onAgent: (agent?: string) => void;
  onVariant: (variant: string) => void;
  onAcceleration: (enabled: boolean) => void;
  onTheme: (theme: ThemeMode) => void;
  onConfigDraft: (key: string, value: string) => void;
  onWorkspaceConfigDraft: (key: string, value: string) => void;
  onProviderSearch: (value: string) => void;
  onOpenProviderAuth: (providerId: string) => void;
  onAuthDraft: (providerId: string, value: string) => void;
  onAuthCode: (providerId: string, value: string) => void;
  onSaveSettings: () => void;
  onValidateModel: () => void;
  onSaveKey: (providerId: string, method: ProviderAuthMethod) => void;
  onStartLogin: (providerId: string, methodIndex: number) => void;
  onCompleteLogin: (
    providerId: string,
    code?: string,
    methodIndex?: number,
  ) => void;
  onLogout: (providerId: string) => void;
}) {
  const providers = createMemo(() => props.state.providers?.all ?? []);
  const selectedProvider = createMemo(
    () =>
      providers().find(
        (provider) => provider.id === props.state.selectedProviderId,
      ) ?? providers()[0],
  );
  const selectedProviderStatus = createMemo(() => {
    const provider = selectedProvider();
    return provider ? props.state.providerAuthStatus[provider.id] : undefined;
  });
  const selectedMethods = createMemo(() => {
    const provider = selectedProvider();
    return provider ? (props.state.providerAuthMethods[provider.id] ?? []) : [];
  });
  const selectedModels = createMemo(() =>
    Object.values(selectedProvider()?.models ?? {}).sort((left, right) =>
      left.name.localeCompare(right.name),
    ),
  );
  const title = createMemo(
    () =>
      settingsSections().find((item) => item.id === props.section)?.label ??
      t("settings"),
  );
  const configRows = createMemo(() => configFieldRows(props.state));
  const workspaceRows = createMemo(() =>
    Object.entries(props.state.workspaceConfigDraft).sort(([left], [right]) =>
      left.localeCompare(right),
    ),
  );
  const filteredProviders = createMemo(() => {
    const query = props.state.providerSearch.trim().toLowerCase();
    if (!query) {
      return providers();
    }
    return providers().filter((provider) =>
      [provider.name, provider.id, provider.source, ...provider.env]
        .join(" ")
        .toLowerCase()
        .includes(query),
    );
  });
  const configuredProviders = createMemo(() =>
    filteredProviders().filter((provider) =>
      providerConfigured(props.state, provider.id),
    ),
  );
  const unconfiguredProviders = createMemo(() =>
    filteredProviders().filter(
      (provider) => !providerConfigured(props.state, provider.id),
    ),
  );

  function chooseProvider(provider: SdkProvider) {
    props.onProvider(provider.id);
  }

  function chooseModelProvider(provider: SdkProvider) {
    props.onProvider(provider.id);
    if (providerIdFromModel(props.state.selectedModel) !== provider.id) {
      const modelId =
        props.state.providers?.default[provider.id] ??
        Object.keys(provider.models)[0];
      if (modelId) {
        props.onModel(modelRef(provider.id, modelId));
      }
    }
  }

  return (
    <section class="settings-view">
      <header class="page-head">
        <div class="page-title">
          <span>{t("settings")}</span>
          <h1>{title()}</h1>
        </div>
        <div class="page-actions">
          <Show when={props.section === "models"}>
            <button
              class="secondary"
              disabled={
                props.state.settingsSaving || !props.state.selectedModel
              }
              onClick={props.onValidateModel}
            >
              {t("validate")}
            </button>
          </Show>
          <Show
            when={props.section !== "providers" && props.section !== "auth"}
          >
            <button
              class="primary"
              disabled={props.state.settingsSaving}
              onClick={props.onSaveSettings}
            >
              {t("save")}
            </button>
          </Show>
        </div>
      </header>

      <main class="settings-canvas">
        <section class="settings-stack">
          <Switch>
            <Match when={props.section === "general"}>
              <section class="settings-panel">
                <header>
                  <span>{t("overview")}</span>
                  <small>{props.state.connection}</small>
                </header>
                <div class="provider-detail">
                  <div class="provider-metrics">
                    <MetricCell
                      label={t("workspace")}
                      value={shortWorkspaceLabel(props.state.directory)}
                    />
                    <MetricCell
                      label={t("gateway")}
                      value={props.state.gatewayUrl}
                    />
                    <MetricCell
                      label={t("version")}
                      value={props.state.health?.version ?? "--"}
                    />
                    <MetricCell
                      label={t("user")}
                      value={props.state.me?.email ?? "--"}
                    />
                  </div>
                </div>
              </section>
              <section class="settings-panel">
                <header>
                  <span>{t("currentRuntime")}</span>
                  <small>{props.state.selectedModel ?? "--"}</small>
                </header>
                <div class="settings-fields">
                  <ReadonlyRow
                    label={t("provider")}
                    value={selectedProvider()?.name ?? "--"}
                  />
                  <ReadonlyRow
                    label={t("model")}
                    value={props.state.selectedModel ?? "--"}
                  />
                  <ReadonlyRow
                    label={t("agent")}
                    value={props.state.selectedAgent ?? "--"}
                  />
                </div>
              </section>
            </Match>

            <Match when={props.section === "appearance"}>
              <section class="settings-panel">
                <header>
                  <span>{t("theme")}</span>
                  <small>{props.state.themeMode}</small>
                </header>
                <div class="settings-fields">
                  <div class="field-row">
                    <span>{t("mode")}</span>
                    <div class="segmented two">
                      <For each={["light", "dark"] as ThemeMode[]}>
                        {(mode) => (
                          <button
                            class={classNames(
                              props.state.themeMode === mode && "selected",
                            )}
                            onClick={() => props.onTheme(mode)}
                          >
                            {mode === "dark" ? t("dark") : t("light")}
                          </button>
                        )}
                      </For>
                    </div>
                  </div>
                  <ReadonlyRow
                    label={t("surface")}
                    value="paper / line / ink"
                  />
                  <ReadonlyRow label={t("radius")} value="8 / 6" />
                </div>
              </section>
            </Match>

            <Match when={props.section === "providers"}>
              <section class="settings-panel">
                <header>
                  <span>{t("providers")}</span>
                  <small>{providers().length}</small>
                </header>
                <label class="workspace-search-row provider-search-row">
                  <Search size={14} strokeWidth={1.7} />
                  <input
                    class="workspace-search"
                    value={props.state.providerSearch}
                    placeholder={`${t("search")}...`}
                    onInput={(event) =>
                      props.onProviderSearch(event.currentTarget.value)
                    }
                  />
                </label>
                <div class="settings-list provider-config-list">
                  <ProviderConfigGroup
                    label={t("configuredProviders")}
                    providers={configuredProviders()}
                    state={props.state}
                    selectedProviderId={selectedProvider()?.id}
                    onProvider={(provider) => {
                      chooseProvider(provider);
                      props.onOpenProviderAuth(provider.id);
                    }}
                  />
                  <ProviderConfigGroup
                    label={t("unconfiguredProviders")}
                    providers={unconfiguredProviders()}
                    state={props.state}
                    selectedProviderId={selectedProvider()?.id}
                    onProvider={(provider) => {
                      chooseProvider(provider);
                      props.onOpenProviderAuth(provider.id);
                    }}
                  />
                </div>
              </section>
              <section class="settings-panel">
                <header>
                  <span>{selectedProvider()?.name ?? t("provider")}</span>
                  <small>{authStatusText(selectedProviderStatus())}</small>
                </header>
                <Show when={selectedProvider()}>
                  {(provider) => (
                    <div class="provider-detail">
                      <div class="provider-metrics">
                        <MetricCell
                          label={t("state")}
                          value={authStatusText(selectedProviderStatus())}
                        />
                        <MetricCell
                          label={t("source")}
                          value={providerSourceLabel(provider().source)}
                        />
                        <MetricCell
                          label={t("env")}
                          value={provider().env.join(", ") || "--"}
                        />
                        <MetricCell
                          label={t("models")}
                          value={String(Object.keys(provider().models).length)}
                        />
                      </div>
                    </div>
                  )}
                </Show>
              </section>
            </Match>

            <Match when={props.section === "models"}>
              <section class="settings-panel">
                <header>
                  <span>{t("modelRuntime")}</span>
                  <small>{props.state.selectedModel ?? "--"}</small>
                </header>
                <div class="settings-fields">
                  <label class="field-row">
                    <span>{t("provider")}</span>
                    <ProviderSelectMenu
                      providers={providers()}
                      selectedProviderId={selectedProvider()?.id}
                      state={props.state}
                      onProvider={chooseModelProvider}
                    />
                  </label>
                  <label class="field-row">
                    <span>{t("model")}</span>
                    <select
                      value={props.state.selectedModel ?? ""}
                      onChange={(event) =>
                        props.onModel(event.currentTarget.value)
                      }
                    >
                      <For each={selectedModels()}>
                        {(model) => (
                          <option
                            value={modelRef(selectedProvider()?.id, model.id)}
                          >
                            {model.name}
                          </option>
                        )}
                      </For>
                    </select>
                  </label>
                </div>
              </section>

              <section class="settings-panel">
                <header>
                  <span>{t("models")}</span>
                  <small>{selectedProvider()?.name ?? "--"}</small>
                </header>
                <Show
                  when={selectedProvider()}
                  fallback={<div class="surface-list-empty">{t("empty")}</div>}
                >
                  {(provider) => (
                    <div class="provider-detail">
                      <div class="model-list">
                        <For each={selectedModels()}>
                          {(model) => (
                            <button
                              class={classNames(
                                props.state.selectedModel ===
                                  modelRef(provider().id, model.id) &&
                                  "selected",
                              )}
                              onClick={() =>
                                props.onModel(modelRef(provider().id, model.id))
                              }
                            >
                              <span>{model.name}</span>
                              <small>
                                {formatModelLimit(model.limit.context)}
                              </small>
                            </button>
                          )}
                        </For>
                      </div>
                    </div>
                  )}
                </Show>
              </section>
            </Match>

            <Match when={props.section === "auth"}>
              <section class="settings-panel">
                <header>
                  <span>{t("login")}</span>
                  <small>{authStatusText(selectedProviderStatus())}</small>
                </header>
                <Show
                  when={selectedProvider()}
                  fallback={<div class="surface-list-empty">{t("empty")}</div>}
                >
                  {(provider) => (
                    <div class="settings-fields login-fields">
                      <For
                        each={selectedMethods()}
                        fallback={
                          <div class="surface-list-empty">{t("empty")}</div>
                        }
                      >
                        {(method, index) => (
                          <div
                            class={classNames(
                              "login-method",
                              method.type === "oauth" && "oauth",
                            )}
                          >
                            <div class="login-method-copy">
                              <span>{method.label}</span>
                              <small>
                                {method.token_env ??
                                  method.login_env ??
                                  method.kind}
                              </small>
                            </div>
                            <Show when={method.type === "api"}>
                              <div class="login-method-controls">
                                <input
                                  type="password"
                                  value={
                                    props.state.authDrafts[provider().id] ?? ""
                                  }
                                  placeholder={method.token_env ?? t("apiKey")}
                                  onInput={(event) =>
                                    props.onAuthDraft(
                                      provider().id,
                                      event.currentTarget.value,
                                    )
                                  }
                                />
                                <button
                                  class="secondary"
                                  disabled={
                                    props.state.settingsSaving ||
                                    !props.state.authDrafts[
                                      provider().id
                                    ]?.trim()
                                  }
                                  onClick={() =>
                                    props.onSaveKey(provider().id, method)
                                  }
                                >
                                  {t("save")}
                                </button>
                              </div>
                            </Show>
                            <Show when={method.type === "oauth"}>
                              <div class="login-method-controls oauth-controls">
                                <button
                                  class="secondary"
                                  disabled={props.state.settingsSaving}
                                  onClick={() =>
                                    props.onStartLogin(provider().id, index())
                                  }
                                >
                                  {t("openLogin")}
                                </button>
                                <input
                                  value={
                                    props.state.authCodeDrafts[provider().id] ??
                                    ""
                                  }
                                  placeholder={t("codeOrToken")}
                                  onInput={(event) =>
                                    props.onAuthCode(
                                      provider().id,
                                      event.currentTarget.value,
                                    )
                                  }
                                />
                                <button
                                  class="secondary"
                                  disabled={props.state.settingsSaving}
                                  onClick={() =>
                                    props.onCompleteLogin(
                                      provider().id,
                                      props.state.authCodeDrafts[provider().id],
                                      index(),
                                    )
                                  }
                                >
                                  {t("complete")}
                                </button>
                              </div>
                            </Show>
                          </div>
                        )}
                      </For>
                      <div class="settings-actions-row">
                        <button
                          class="text-button"
                          disabled={
                            props.state.settingsSaving ||
                            !selectedProviderStatus()?.configured
                          }
                          onClick={() => props.onLogout(provider().id)}
                        >
                          {t("logout")}
                        </button>
                      </div>
                    </div>
                  )}
                </Show>
              </section>
            </Match>

            <Match when={props.section === "runtime"}>
              <section class="settings-panel">
                <header>
                  <span>{t("runtime")}</span>
                  <small>{props.state.selectedAgent ?? "--"}</small>
                </header>
                <div class="settings-fields">
                  <label class="field-row">
                    <span>{t("agent")}</span>
                    <select
                      value={props.state.selectedAgent ?? ""}
                      onChange={(event) =>
                        props.onAgent(event.currentTarget.value || undefined)
                      }
                    >
                      <For
                        each={props.state.agents.filter(
                          (agent) => !agent.hidden,
                        )}
                      >
                        {(agent) => (
                          <option value={agent.name}>{agent.name}</option>
                        )}
                      </For>
                    </select>
                  </label>
                  <div class="field-row">
                    <span>{t("variant")}</span>
                    <div class="segmented">
                      <For each={["low", "medium", "high"]}>
                        {(variant) => (
                          <button
                            class={classNames(
                              props.state.modelVariant === variant &&
                                "selected",
                            )}
                            onClick={() => props.onVariant(variant)}
                          >
                            {variant}
                          </button>
                        )}
                      </For>
                    </div>
                  </div>
                  <label class="field-row compact-field">
                    <span>{t("acceleration")}</span>
                    <input
                      type="checkbox"
                      checked={props.state.accelerationEnabled}
                      onChange={(event) =>
                        props.onAcceleration(event.currentTarget.checked)
                      }
                    />
                  </label>
                  <ReadonlyRow
                    label={t("model")}
                    value={props.state.selectedModel ?? "--"}
                  />
                </div>
              </section>
            </Match>

            <Match when={props.section === "config"}>
              <section class="settings-panel">
                <header>
                  <span>{t("turaConfig")}</span>
                  <small>{t("global")}</small>
                </header>
                <div class="settings-fields">
                  <For each={configRows()}>
                    {(row) => (
                      <label class="field-row">
                        <span>{row.label}</span>
                        <input
                          value={props.state.configDraft[row.key] ?? ""}
                          onInput={(event) =>
                            props.onConfigDraft(
                              row.key,
                              event.currentTarget.value,
                            )
                          }
                        />
                      </label>
                    )}
                  </For>
                </div>
              </section>
            </Match>

            <Match when={props.section === "workspace"}>
              <section class="settings-panel">
                <header>
                  <span>{t("workspaceConfig")}</span>
                  <small>{workspaceRows().length}</small>
                </header>
                <div class="settings-fields">
                  <For
                    each={workspaceRows()}
                    fallback={
                      <div class="surface-list-empty">{t("empty")}</div>
                    }
                  >
                    {([key, value]) => (
                      <label class="field-row">
                        <span>{key}</span>
                        <input
                          value={value}
                          onInput={(event) =>
                            props.onWorkspaceConfigDraft(
                              key,
                              event.currentTarget.value,
                            )
                          }
                        />
                      </label>
                    )}
                  </For>
                </div>
              </section>
            </Match>

            <Match when={props.section === "environment"}>
              <section class="settings-panel">
                <header>
                  <span>{t("paths")}</span>
                  <small>{props.state.connection}</small>
                </header>
                <div class="settings-fields">
                  <ReadonlyRow
                    label={t("home")}
                    value={props.state.paths?.home ?? "--"}
                  />
                  <ReadonlyRow
                    label={t("state")}
                    value={props.state.paths?.state ?? "--"}
                  />
                  <ReadonlyRow
                    label={t("config")}
                    value={props.state.paths?.config ?? "--"}
                  />
                  <ReadonlyRow
                    label={t("worktree")}
                    value={props.state.paths?.worktree ?? "--"}
                  />
                </div>
              </section>
              <section class="settings-panel">
                <header>
                  <span>{t("env")}</span>
                  <small>{selectedProvider()?.name ?? "--"}</small>
                </header>
                <div class="settings-fields">
                  <For each={providers().slice(0, 10)}>
                    {(provider) => (
                      <ReadonlyRow
                        label={provider.name}
                        value={provider.env.join(", ") || "--"}
                      />
                    )}
                  </For>
                </div>
              </section>
            </Match>
          </Switch>

          <Show
            when={props.state.settingsNotice || props.state.modelValidation}
          >
            <div class="settings-note">
              {props.state.settingsNotice ?? props.state.modelValidation}
            </div>
          </Show>
        </section>
      </main>
    </section>
  );
}

export function ReadonlyRow(props: { label: string; value: string }) {
  return (
    <div class="field-row readonly-row">
      <span>{props.label}</span>
      <code>{props.value}</code>
    </div>
  );
}

export function MetricCell(props: { label: string; value: string }) {
  return (
    <div class="metric-cell">
      <span>{props.value}</span>
      <small>{props.label}</small>
    </div>
  );
}
