import {
  type AgentAvatarConfig,
  type AgentUpsertRequest,
  type PersonaMediaConfig,
  type SdkProvider,
  type StoredAgent,
  type StoredPersona,
  type TuraConfigModelPair,
} from "@tura/gateway-sdk";
import ArrowLeft from "lucide-solid/icons/arrow-left";
import FolderSearch from "lucide-solid/icons/folder-search";
import LayoutList from "lucide-solid/icons/layout-list";
import MessageSquare from "lucide-solid/icons/message-square";
import Search from "lucide-solid/icons/search";
import {
  createEffect,
  createMemo,
  createSignal,
  For,
  Match,
  Show,
  Switch,
  type JSX,
} from "solid-js";
import { t } from "../../i18n";
import {
  AgentAvatarCanvas,
  AVATAR_WORKSPACE_CONFIG_KEY,
  AVATAR_SETTING_LIMITS,
  DEFAULT_AVATAR_SETTINGS,
  agentAvatarMedia,
  avatarSettingsFromConfigValue,
  normalizeAvatarSettings,
  type AvatarDisplayMode,
  type AvatarRenderSettings,
} from "../../components/avatar/agent-avatar-canvas";
import { DEFAULT_CODE_FONT } from "../../config/defaults";
import { classNames } from "../../state/format";
import {
  systemThemeMode,
  type AppState,
  type MainTab,
  type SettingsSection,
  type ThemeMode,
} from "../../state/global-store";

import { providerConfigured } from "../../utils/settings";
import { AppearanceSelect, CONFIGURE_PROVIDER_OPTION } from "./appearance-select";
import { providerDomains } from "./provider-domain";
import { ProviderConfigGroup } from "./provider-settings";
import { AgentSettingsPanel } from "./agent-settings-panel";
import { settingsRoutes, settingsRouteTitle } from "./settings-router";
import {
  DEFAULT_PROVIDER_DOMAIN,
  LANGUAGE_OPTIONS,
  MODEL_SETTINGS_TIERS,
  THEME_OPTIONS,
  codeFontOptions,
  compareProviderDomains,
  languageLabel,
  mainFontOptions,
  modelOptionValue,
  modelTierLabel,
  modelTierOptions,
  providerDomainLabel,
  sizeOptions,
} from "./settings-options";
export function MainTabs(props: {
  active: Exclude<MainTab, "settings">;
  conversationLabel?: string;
  onChange: (tab: Exclude<MainTab, "settings">) => void;
}) {
  const tabs: Array<{
    id: Exclude<MainTab, "settings">;
    label: string;
    icon?: JSX.Element;
  }> = [
    {
      id: "conversation",
      label: props.conversationLabel ?? t("session"),
      icon: <MessageSquare size={15} />,
    },
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
            <Show when={item.id === "conversation"}>
              <span class="main-tab-hidden-alias">
                {t("sessionHistory")} {t("newSession")}
              </span>
            </Show>
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
  function selectSection(event: MouseEvent & { currentTarget: HTMLButtonElement }) {
    const section = event.currentTarget.dataset.section as SettingsSection | undefined;
    if (section) {
      props.onSection(section);
    }
  }

  return (
    <nav class="settings-rail">
      <button class="settings-back" type="button" onClick={props.onBack}>
        <ArrowLeft size={15} strokeWidth={1.8} aria-hidden="true" />
        {t("backToApp")}
      </button>
      <div class="section-title">{t("settings")}</div>
      <div class="settings-section-list">
        <For each={settingsRoutes()}>
          {(item) => (
            <button
              class={classNames(props.active === item.id && "selected")}
              data-section={item.id}
              aria-current={props.active === item.id ? "page" : undefined}
              type="button"
              onClick={selectSection}
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
  onModelTier: (tier: string, option: TuraConfigModelPair) => void;
  onConfigureProviders: () => void;
  onTheme: (theme: ThemeMode) => void;
  onMainFont: (font: string) => void;
  onCodeFont: (font: string) => void;
  onMainFontSize: (size: number) => void;
  onCodeFontSize: (size: number) => void;
  onProviderSearch: (value: string) => void;
  onOpenProviderAuth: (providerId: string) => void;
  onRefreshAgents: () => Promise<void>;
  onGetAgent: (agentId: string) => Promise<StoredAgent | undefined>;
  onSaveAgent: (agentId: string | undefined, payload: AgentUpsertRequest) => Promise<void>;
  onDeleteAgent: (agentId: string) => Promise<void>;
  onSavePersonalization: (avatar: AvatarRenderSettings) => void;
  onLanguage: (language: string) => void;
}) {
  const providers = createMemo(() => props.state.providers?.all ?? []);
  const [providerDomainFilter, setProviderDomainFilter] = createSignal(DEFAULT_PROVIDER_DOMAIN);
  const title = createMemo(() => settingsRouteTitle(props.section));
  const providerDomainOptions = createMemo(() => {
    return [...(props.state.providers?.enums.domains ?? [])].sort(compareProviderDomains);
  });
  createEffect(() => {
    const options = providerDomainOptions();
    if (options.length === 0 || options.includes(providerDomainFilter())) {
      return;
    }
    setProviderDomainFilter(options[0]);
  });
  const filteredProviders = createMemo(() => {
    const query = props.state.providerSearch.trim().toLowerCase();
    const domain = providerDomainFilter();
    const domainProviders = providers().filter((provider) =>
      providerDomains(provider).includes(domain),
    );
    if (!query) {
      return domainProviders;
    }
    return domainProviders.filter((provider) =>
      [provider.name, provider.id, provider.source, ...provider.env]
        .join(" ")
        .toLowerCase()
        .includes(query),
    );
  });
  const configuredProviders = createMemo(() =>
    filteredProviders().filter((provider) => providerConfigured(props.state, provider.id)),
  );
  const unconfiguredProviders = createMemo(() =>
    filteredProviders().filter((provider) => !providerConfigured(props.state, provider.id)),
  );

  function chooseProvider(provider: SdkProvider) {
    props.onProvider(provider.id);
  }

  return (
    <section class="settings-view layered-page layered-page-two">
      <header class="page-head page-layer-inner">
        <div class="page-title">
          <span>{t("settings")}</span>
          <h1>{title()}</h1>
        </div>
        <div class="page-actions" />
      </header>

      <main class="settings-canvas page-layer-middle">
        <section class="settings-stack page-layer-inner">
          <Switch>
            <Match when={props.section === "application"}>
              <section class="settings-panel">
                <header>
                  <span>{t("applicationSettings")}</span>
                  <small>{languageLabel(props.state.configDraft.language)}</small>
                </header>
                <div class="settings-fields">
                  <div class="field-row">
                    <span>{t("language")}</span>
                    <AppearanceSelect
                      value={props.state.configDraft.language || "zh-CN"}
                      options={LANGUAGE_OPTIONS.map((option) => ({
                        id: option.id,
                        label: option.label,
                        value: option.id,
                        preview: "inherit",
                      }))}
                      onSelect={(option) => props.onLanguage(option.value)}
                    />
                  </div>
                </div>
              </section>
            </Match>

            <Match when={props.section === "appearance"}>
              <section class="settings-panel appearance-panel">
                <header>
                  <span>{t("themeSettings")}</span>
                  <small>{props.state.themeMode}</small>
                </header>
                <div class="settings-fields">
                  <div class="field-row">
                    <span>{t("themeColor")}</span>
                    <div class="segmented settings-filter-segmented">
                      <For each={THEME_OPTIONS}>
                        {(option) => (
                          <button
                            class={classNames(
                              "theme-choice",
                              props.state.themeMode === option.id && "selected",
                            )}
                            onClick={() => props.onTheme(option.id)}
                          >
                            <span
                              class="theme-choice-swatch"
                              style={{
                                "--theme-paper": option.preview.paper,
                                "--theme-wash": option.preview.wash,
                                "--theme-accent": option.preview.accent,
                              }}
                              aria-hidden="true"
                            />
                            <span class="theme-choice-label">
                              {option.label}
                              <Show when={option.id === systemThemeMode()}> ({t("default")})</Show>
                            </span>
                          </button>
                        )}
                      </For>
                    </div>
                  </div>
                  <div class="field-row">
                    <span>{t("mainFont")}</span>
                    <AppearanceSelect
                      value={props.state.mainFont || mainFontOptions()[0].value}
                      options={mainFontOptions()}
                      onSelect={(option) =>
                        props.onMainFont(option.id === "system" ? "" : option.value)
                      }
                    />
                  </div>
                  <div class="field-row">
                    <span>{t("codeFont")}</span>
                    <AppearanceSelect
                      value={props.state.codeFont || DEFAULT_CODE_FONT}
                      options={codeFontOptions()}
                      onSelect={(option) =>
                        props.onCodeFont(option.value === DEFAULT_CODE_FONT ? "" : option.value)
                      }
                    />
                  </div>
                  <div class="field-row">
                    <span>{t("mainFontSize")}</span>
                    <AppearanceSelect
                      value={String(props.state.mainFontSize)}
                      options={sizeOptions(11, 15, 12)}
                      onSelect={(option) => props.onMainFontSize(Number(option.value))}
                    />
                  </div>
                  <div class="field-row">
                    <span>{t("codeFontSize")}</span>
                    <AppearanceSelect
                      value={String(props.state.codeFontSize)}
                      options={sizeOptions(9, 15, 11)}
                      onSelect={(option) => props.onCodeFontSize(Number(option.value))}
                    />
                  </div>
                </div>
              </section>
            </Match>

            <Match when={props.section === "providers"}>
              <section class="settings-panel">
                <header>
                  <span>{t("providerSettings")}</span>
                  <small>{providers().length}</small>
                </header>
                <div class="provider-domain-filter-row">
                  <span>{t("providerType")}</span>
                  <div class="segmented settings-filter-segmented">
                    <For each={providerDomainOptions()}>
                      {(domain) => (
                        <button
                          class={classNames(providerDomainFilter() === domain && "selected")}
                          onClick={() => setProviderDomainFilter(domain)}
                        >
                          {providerDomainLabel(domain)}
                        </button>
                      )}
                    </For>
                  </div>
                </div>
                <label class="workspace-search-row provider-search-row">
                  <Search size={14} strokeWidth={1.7} />
                  <input
                    class="workspace-search"
                    value={props.state.providerSearch}
                    placeholder={`${t("search")}...`}
                    onInput={(event) => props.onProviderSearch(event.currentTarget.value)}
                  />
                </label>
                <div class="settings-list provider-config-list">
                  <ProviderConfigGroup
                    label={t("configuredProviders")}
                    providers={configuredProviders()}
                    state={props.state}
                    onProvider={(provider) => {
                      chooseProvider(provider);
                      props.onOpenProviderAuth(provider.id);
                    }}
                  />
                  <ProviderConfigGroup
                    label={t("unconfiguredProviders")}
                    providers={unconfiguredProviders()}
                    state={props.state}
                    onProvider={(provider) => {
                      chooseProvider(provider);
                      props.onOpenProviderAuth(provider.id);
                    }}
                  />
                </div>
              </section>
            </Match>

            <Match when={props.section === "models"}>
              <section class="settings-panel model-config-panel">
                <header>
                  <span>{t("modelRuntime")}</span>
                  <small>{props.state.modelConfig?.path ?? "--"}</small>
                </header>
                <Show
                  when={(props.state.modelConfig?.tiers ?? []).length > 0}
                  fallback={<div class="surface-list-empty">{t("empty")}</div>}
                >
                  <div class="settings-fields">
                    <For
                      each={(props.state.modelConfig?.tiers ?? []).filter((tier) =>
                        MODEL_SETTINGS_TIERS.includes(tier.tier),
                      )}
                    >
                      {(tier) => (
                        <div class="field-row">
                          <span class="model-tier-label">
                            <span>{modelTierLabel(tier.tier)}</span>
                          </span>
                          <Show
                            when={modelTierOptions(tier).length > 0}
                            fallback={
                              <button
                                type="button"
                                class="appearance-select-button model-configure-button"
                                onClick={props.onConfigureProviders}
                              >
                                <span>{t("configureProvider")}</span>
                              </button>
                            }
                          >
                            <AppearanceSelect
                              value={modelOptionValue(tier.current)}
                              options={modelTierOptions(tier)}
                              footer={{
                                label: t("configureProvider"),
                                onSelect: props.onConfigureProviders,
                              }}
                              onSelect={(option) => {
                                if (option.value === CONFIGURE_PROVIDER_OPTION) {
                                  props.onConfigureProviders();
                                  return;
                                }
                                const modelOption = tier.options.find(
                                  (item) => modelOptionValue(item) === option.value,
                                );
                                if (modelOption) {
                                  props.onModelTier(tier.tier, modelOption);
                                }
                              }}
                            />
                          </Show>
                        </div>
                      )}
                    </For>
                  </div>
                </Show>
              </section>
            </Match>

            <Match when={props.section === "agents"}>
              <AgentSettingsPanel
                agents={props.state.agents}
                saving={props.state.settingsSaving}
                modelConfig={props.state.modelConfig}
                onRefresh={props.onRefreshAgents}
                onGetAgent={props.onGetAgent}
                onSaveAgent={props.onSaveAgent}
              />
            </Match>
            <Match when={props.section === "personalization"}>
              <PersonalizationSettingsPanel
                personas={props.state.personas}
                savedAvatar={personalizationAvatarFromState(props.state)}
                saving={props.state.settingsSaving}
                onSave={props.onSavePersonalization}
              />
            </Match>
          </Switch>
        </section>
      </main>
    </section>
  );
}

function PersonalizationSettingsPanel(props: {
  personas: StoredPersona[];
  savedAvatar: AvatarRenderSettings;
  saving: boolean;
  onSave: (avatar: AvatarRenderSettings) => void;
}) {
  const personas = createMemo(() => avatarPersonaOptions(props.personas));
  const [selectedPersonaId, setSelectedPersonaId] = createSignal(
    props.savedAvatar.persona_id ?? props.savedAvatar.role,
  );
  const [avatar, setAvatar] = createSignal<AvatarRenderSettings>(props.savedAvatar);
  const selectedMedia = createMemo(() =>
    agentAvatarMedia(
      personaMediaForAvatar(props.personas, selectedPersonaId()),
      selectedPersonaId(),
    ),
  );

  createEffect(() => {
    if (!personas().some((persona) => persona.id === selectedPersonaId())) {
      setSelectedPersonaId(personas()[0]?.id ?? "tura");
    }
  });

  function selectPersona(personaId: string) {
    setSelectedPersonaId(personaId);
    setAvatar((current) =>
      normalizeAvatarSettings({
        ...current,
        persona_id: personaId,
        role: personaId,
      }),
    );
  }

  function savePersonalization() {
    props.onSave(
      normalizeAvatarSettings({
        ...avatar(),
        persona_id: selectedPersonaId(),
        role: selectedPersonaId(),
      }),
    );
  }

  return (
    <section class="settings-panel agent-settings-panel personalization-panel">
      <header>
        <span>{t("personalization")}</span>
        <small>{personas().length}</small>
      </header>
      <div class="agent-settings-layout">
        <div class="settings-list agent-list">
          <div class="settings-list provider-config-list agent-config-list">
            <div class="provider-config-group agent-configured-group">
              <div class="provider-config-group-title">
                <span>可配置 Persona</span>
                <small>{personas().length}</small>
              </div>
              <div class="workspace-picker-list agent-list-scroll">
                <For each={personas()}>
                  {(persona) => (
                    <button
                      type="button"
                      class={classNames(
                        "workspace-pick-row",
                        "agent-pick-row",
                        "persona-pick-row",
                        selectedPersonaId() === persona.id && "selected",
                      )}
                      onClick={() => selectPersona(persona.id)}
                    >
                      <span>{persona.label}</span>
                      <small>{persona.description}</small>
                    </button>
                  )}
                </For>
              </div>
            </div>
          </div>
        </div>
        <div class="settings-fields agent-editor">
          <AgentAvatarSettings media={selectedMedia()} value={avatar()} onChange={setAvatar} />
          <div class="settings-actions-row agent-actions-row">
            <button
              type="button"
              class="primary"
              disabled={props.saving}
              aria-busy={props.saving}
              onClick={savePersonalization}
            >
              <Show
                when={!props.saving}
                fallback={<span class="button-loading-bar loading-bar short" />}
              >
                {t("save")}
              </Show>
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}

function personalizationAvatarFromState(state: AppState): AvatarRenderSettings {
  return avatarSettingsFromConfigValue(
    state.workspaceConfigDraft[AVATAR_WORKSPACE_CONFIG_KEY] ??
      state.workspaceConfig[AVATAR_WORKSPACE_CONFIG_KEY],
  );
}

function AgentAvatarSettings(props: {
  media: PersonaMediaConfig;
  value: AvatarRenderSettings;
  onChange: (value: AvatarRenderSettings) => void;
}) {
  function updateAvatar(patch: Partial<AgentAvatarConfig>) {
    props.onChange(normalizeAvatarSettings({ ...props.value, ...patch }));
  }
  const displayModeOptions: Array<{ value: AvatarDisplayMode; label: string }> = [
    { value: "hidden", label: "隐藏头像" },
    { value: "static", label: "静态头像" },
    { value: "dynamic", label: "动态头像" },
  ];

  return (
    <section class="agent-avatar-settings">
      <div class="agent-avatar-controls">
        <div class="agent-avatar-control-row">
          <span>头像显示</span>
          <div class="segmented agent-avatar-mode-segmented">
            <For each={displayModeOptions}>
              {(option) => (
                <button
                  type="button"
                  class={classNames(props.value.display_mode === option.value && "selected")}
                  onClick={() => updateAvatar({ display_mode: option.value })}
                >
                  {option.label}
                </button>
              )}
            </For>
          </div>
        </div>
        <AvatarRange
          id="agent-avatar-pixel"
          label="像素画"
          min={AVATAR_SETTING_LIMITS.pixelSize.min}
          max={AVATAR_SETTING_LIMITS.pixelSize.max}
          value={props.value.pixel_size}
          onInput={(pixel_size) => updateAvatar({ pixel_size })}
        />
        <AvatarRange
          id="agent-avatar-threshold"
          label="阈值"
          min={AVATAR_SETTING_LIMITS.threshold.min}
          max={AVATAR_SETTING_LIMITS.threshold.max}
          value={props.value.threshold}
          onInput={(threshold) => updateAvatar({ threshold })}
        />
        <AvatarRange
          id="agent-avatar-scale"
          label="头像缩放"
          min={AVATAR_SETTING_LIMITS.scale.min}
          max={AVATAR_SETTING_LIMITS.scale.max}
          value={props.value.scale}
          onInput={(scale) => updateAvatar({ scale })}
        />
      </div>
      <div class="agent-avatar-preview" aria-label="头像预览">
        <span>头像预览</span>
        <div class="agent-avatar-preview-shell">
          <Show
            when={props.value.display_mode !== "hidden"}
            fallback={<span class="settings-note">头像已隐藏</span>}
          >
            <AgentAvatarCanvas
              media={props.media}
              settings={{
                ...props.value,
                scale: DEFAULT_AVATAR_SETTINGS.scale,
              }}
              expressionId="vigilant"
              interactive={props.value.display_mode === "dynamic"}
              label="头像预览"
            />
          </Show>
        </div>
      </div>
    </section>
  );
}

function AvatarRange(props: {
  id: string;
  label: string;
  min: number;
  max: number;
  value: number;
  onInput: (value: number) => void;
}) {
  const progress = createMemo(() =>
    Math.round(((props.value - props.min) / (props.max - props.min)) * 100),
  );
  return (
    <div class="agent-avatar-control-row">
      <label for={props.id}>{props.label}</label>
      <div class="range-control" style={{ "--range-progress": `${progress()}%` }}>
        <input
          id={props.id}
          type="range"
          min={props.min}
          max={props.max}
          value={props.value}
          onInput={(event) => props.onInput(Number(event.currentTarget.value))}
        />
        <output for={props.id}>{props.value}</output>
      </div>
    </div>
  );
}

function avatarPersonaOptions(personas: StoredPersona[]) {
  const fromGateway = personas
    .filter((persona) => persona.summary.media || persona.config.media)
    .map((persona) => ({
      id: persona.summary.id,
      label: persona.summary.display_name || persona.summary.id,
      description:
        persona.summary.short_description ||
        persona.config.short_description ||
        persona.summary.description ||
        persona.config.description ||
        "",
    }));
  const fallback = ["tura", "wonderful", "pidan"].map((id) => ({
    id,
    label: id,
    description:
      id === "tura"
        ? "Sharp supervisor"
        : id === "wonderful"
          ? "Loyal companion"
          : "Sleepy strategist",
  }));
  const seen = new Set<string>();
  return [...fromGateway, ...fallback].filter((item) => {
    if (seen.has(item.id)) {
      return false;
    }
    seen.add(item.id);
    return true;
  });
}

function personaMediaForAvatar(
  personas: StoredPersona[],
  personaId: string | undefined,
): PersonaMediaConfig | undefined {
  return (
    personas.find((persona) => persona.summary.id === personaId)?.summary.media ??
    personas.find((persona) => persona.summary.id === personaId)?.config.media ??
    undefined
  );
}
