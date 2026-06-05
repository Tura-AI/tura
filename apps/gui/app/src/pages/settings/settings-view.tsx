import {
  type Agent,
  type AgentAvatarConfig,
  type AgentConfig,
  type AgentUpsertRequest,
  type PersonaMediaConfig,
  type SdkProvider,
  type StoredAgent,
  type StoredPersona,
  type TuraConfigModelPair,
  type TuraConfigResponse,
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
import { activeLanguage, t, type TextKey } from "../../i18n";
import { AgentIcon } from "../../components/agent-icon";
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
import {
  agentDisplayName,
  visibleConfigurableAgents,
} from "../../utils/agent-display";
import {
  AppearanceSelect,
  CONFIGURE_PROVIDER_OPTION,
  type AppearanceOption,
} from "./appearance-select";
import { providerDomains } from "./provider-domain";
import { ProviderConfigGroup } from "./provider-settings";
import { settingsRoutes, settingsRouteTitle } from "./settings-router";
const THEME_OPTIONS: Array<{ id: ThemeMode; label: string }> = [
  { id: "light", label: t("light") },
  { id: "dark", label: t("dark") },
  { id: "caral", label: "Caral" },
  { id: "uruk", label: "Uruk" },
  { id: "liangzhu", label: "Liangzhu" },
];
const DEFAULT_PROVIDER_DOMAIN = "llm";
const PROVIDER_DOMAIN_LABELS: Record<string, TextKey> = {
  communication: "domainCommunication",
  infrastructure: "domainInfrastructure",
  llm: "domainLlm",
  other: "domainOther",
  productivity: "domainProductivity",
  search: "domainSearch",
};
const AGENT_MODEL_TIERS = [
  "flagship_thinking",
  "thinking",
  "fast",
  "instant",
] as const;
const AGENT_REASONING_EFFORTS = ["low", "medium", "high", "xhigh"] as const;
const MODEL_SETTINGS_TIERS = [
  "flagship_thinking",
  "thinking",
  "fast",
  "instant",
];
const LANGUAGE_OPTIONS = [
  { id: "zh-CN", label: "简体中文" },
  { id: "en", label: "English" },
];

type FontLocale =
  | "en"
  | "zhHans"
  | "zhHant"
  | "es"
  | "hi"
  | "ar"
  | "pt"
  | "bn"
  | "ru"
  | "ja";
const FONT_LOCALE_ORDER: FontLocale[] = [
  "zhHans",
  "zhHant",
  "en",
  "es",
  "hi",
  "ar",
  "pt",
  "bn",
  "ru",
  "ja",
];

const MAIN_FONT_MAP = [
  {
    id: "system",
    names: {
      en: "Segoe UI",
      zhHans: "微软雅黑",
      zhHant: "蘋方-繁",
      es: "Segoe UI",
      hi: "Nirmala UI",
      ar: "Segoe UI Arabic",
      pt: "Segoe UI",
      bn: "Nirmala UI",
      ru: "Segoe UI",
      ja: "Yu Gothic UI",
    },
    families: {
      en: '"Segoe UI"',
      zhHans: '"Microsoft YaHei"',
      zhHant: '"PingFang TC", "Microsoft JhengHei"',
      es: '"Segoe UI"',
      hi: '"Nirmala UI"',
      ar: '"Segoe UI Arabic"',
      pt: '"Segoe UI"',
      bn: '"Nirmala UI"',
      ru: '"Segoe UI"',
      ja: '"Yu Gothic UI", "Yu Gothic"',
    },
  },
  {
    id: "arial",
    names: {
      en: "Arial",
      zhHans: "黑体",
      zhHant: "微軟正黑體",
      es: "Arial",
      hi: "Nirmala UI",
      ar: "Arial",
      pt: "Arial",
      bn: "Nirmala UI",
      ru: "Arial",
      ja: "Meiryo",
    },
    families: {
      en: "Arial",
      zhHans: "SimHei",
      zhHant: '"Microsoft JhengHei"',
      es: "Arial",
      hi: '"Nirmala UI"',
      ar: "Arial",
      pt: "Arial",
      bn: '"Nirmala UI"',
      ru: "Arial",
      ja: "Meiryo",
    },
  },
  {
    id: "noto-sans",
    names: {
      en: "Noto Sans",
      zhHans: "思源黑体",
      zhHant: "思源黑體",
      es: "Noto Sans",
      hi: "Noto Sans Devanagari",
      ar: "Noto Sans Arabic",
      pt: "Noto Sans",
      bn: "Noto Sans Bengali",
      ru: "Noto Sans",
      ja: "Noto Sans JP",
    },
    families: {
      en: '"Noto Sans"',
      zhHans: '"Noto Sans SC", "Source Han Sans SC"',
      zhHant: '"Noto Sans TC", "Source Han Sans TC"',
      es: '"Noto Sans"',
      hi: '"Noto Sans Devanagari"',
      ar: '"Noto Sans Arabic"',
      pt: '"Noto Sans"',
      bn: '"Noto Sans Bengali"',
      ru: '"Noto Sans"',
      ja: '"Noto Sans JP"',
    },
  },
  {
    id: "humanist",
    names: {
      en: "Aptos",
      zhHans: "等线",
      zhHant: "蘋方-繁",
      es: "Aptos",
      hi: "Nirmala UI",
      ar: "Dubai",
      pt: "Aptos",
      bn: "Nirmala UI",
      ru: "Aptos",
      ja: "Yu Gothic",
    },
    families: {
      en: "Aptos",
      zhHans: "DengXian",
      zhHant: '"PingFang TC"',
      es: "Aptos",
      hi: '"Nirmala UI"',
      ar: "Dubai",
      pt: "Aptos",
      bn: '"Nirmala UI"',
      ru: "Aptos",
      ja: '"Yu Gothic"',
    },
  },
  {
    id: "serif",
    names: {
      en: "Georgia",
      zhHans: "宋体",
      zhHant: "新細明體",
      es: "Georgia",
      hi: "Noto Serif Devanagari",
      ar: "Noto Naskh Arabic",
      pt: "Georgia",
      bn: "Noto Serif Bengali",
      ru: "Georgia",
      ja: "Yu Mincho",
    },
    families: {
      en: "Georgia",
      zhHans: "SimSun",
      zhHant: "PMingLiU",
      es: "Georgia",
      hi: '"Noto Serif Devanagari"',
      ar: '"Noto Naskh Arabic"',
      pt: "Georgia",
      bn: '"Noto Serif Bengali"',
      ru: "Georgia",
      ja: '"Yu Mincho"',
    },
  },
] as const;

const CODE_FONT_OPTIONS = [
  { label: "System Mono (Default)", value: DEFAULT_CODE_FONT },
  { label: "Cascadia Code", value: '"Cascadia Code", Consolas, monospace' },
  { label: "JetBrains Mono", value: '"JetBrains Mono", Consolas, monospace' },
  { label: "Fira Code", value: '"Fira Code", Consolas, monospace' },
  { label: "Consolas", value: "Consolas, monospace" },
] as const;

function displayFontLocale(): FontLocale {
  return activeLanguage === "zh-CN" ? "zhHans" : "en";
}

function fontFamilyValue(
  fonts: Record<FontLocale, string>,
  preferred: FontLocale,
): string {
  const ordered = [
    preferred,
    ...FONT_LOCALE_ORDER.filter((locale) => locale !== preferred),
  ];
  return [
    ...new Set(ordered.map((locale) => fonts[locale])),
    "ui-sans-serif",
    "system-ui",
    "sans-serif",
  ].join(", ");
}

function mainFontOptions(): AppearanceOption[] {
  const locale = displayFontLocale();
  return MAIN_FONT_MAP.map((font) => {
    const value = fontFamilyValue(font.families, locale);
    const localizedName = font.names[locale];
    const englishName = font.names.en;
    return {
      id: font.id,
      label:
        font.id === "system"
          ? locale === "en" || localizedName === englishName
            ? `${localizedName} (${t("default")})`
            : `${localizedName} / ${englishName} (${t("default")})`
          : locale === "en" || localizedName === englishName
            ? localizedName
            : `${localizedName} / ${englishName}`,
      value,
      preview: font.families[locale],
    };
  });
}

function codeFontOptions(): AppearanceOption[] {
  return CODE_FONT_OPTIONS.map((font) => ({
    id: font.label,
    label: font.label,
    value: font.value,
    preview: font.value,
  }));
}

function providerDomainLabel(domain: string): string {
  const label = PROVIDER_DOMAIN_LABELS[domain];
  return label ? t(label) : domain;
}

function compareProviderDomains(left: string, right: string): number {
  if (left === DEFAULT_PROVIDER_DOMAIN) {
    return right === DEFAULT_PROVIDER_DOMAIN ? 0 : -1;
  }
  if (right === DEFAULT_PROVIDER_DOMAIN) {
    return 1;
  }
  if (left === "other") {
    return right === "other" ? 0 : 1;
  }
  if (right === "other") {
    return -1;
  }
  return providerDomainLabel(left).localeCompare(providerDomainLabel(right));
}

function sizeOptions(
  min: number,
  max: number,
  defaultSize: number,
): AppearanceOption[] {
  return Array.from({ length: max - min + 1 }, (_, index) => {
    const size = min + index;
    return {
      id: String(size),
      label: size === defaultSize ? `${size} (${t("default")})` : String(size),
      value: String(size),
      preview: "inherit",
      size,
    };
  });
}

function modelOptionValue(
  option?: Pick<TuraConfigModelPair, "provider" | "model"> | null,
): string {
  return option ? `${option.provider}/${option.model}` : "";
}

function languageLabel(value: string | undefined): string {
  return (
    LANGUAGE_OPTIONS.find((option) => option.id === value)?.label ??
    LANGUAGE_OPTIONS[0].label
  );
}

function modelConfigOption(option: TuraConfigModelPair): AppearanceOption {
  const provider = option.provider_name || option.provider;
  const model = option.model_name || option.model;
  return {
    id: modelOptionValue(option),
    label: `${provider}/${model}`,
    value: modelOptionValue(option),
    preview: "inherit",
  };
}

function modelTierOptions(
  tier: TuraConfigResponse["tiers"][number],
): AppearanceOption[] {
  const options = tier.options.map(modelConfigOption);
  const currentValue = modelOptionValue(tier.current);
  if (
    currentValue &&
    !options.some((option) => option.value === currentValue) &&
    tier.current
  ) {
    return [
      {
        id: currentValue,
        label: currentValue,
        value: currentValue,
        preview: "inherit",
      },
      ...options,
    ];
  }
  return options;
}

function modelTierLabel(tier: string): string {
  const labels: Record<string, TextKey> = {
    embedding_high: "modelTierEmbeddingHigh",
    embedding_low: "modelTierEmbeddingLow",
    fast: "modelTierFast",
    flagship_thinking: "modelTierFlagshipThinking",
    instant: "modelTierInstant",
    thinking: "modelTierThinking",
  };
  return labels[tier] ? t(labels[tier]) : tier;
}

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
  function selectSection(
    event: MouseEvent & { currentTarget: HTMLButtonElement },
  ) {
    const section = event.currentTarget.dataset.section as
      | SettingsSection
      | undefined;
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
  onSaveAgent: (
    agentId: string | undefined,
    payload: AgentUpsertRequest,
  ) => Promise<void>;
  onDeleteAgent: (agentId: string) => Promise<void>;
  onSavePersonalization: (avatar: AvatarRenderSettings) => void;
  onLanguage: (language: string) => void;
}) {
  const providers = createMemo(() => props.state.providers?.all ?? []);
  const [providerDomainFilter, setProviderDomainFilter] = createSignal(
    DEFAULT_PROVIDER_DOMAIN,
  );
  const title = createMemo(() => settingsRouteTitle(props.section));
  const providerDomainOptions = createMemo(() => {
    return [...(props.state.providers?.enums.domains ?? [])].sort(
      compareProviderDomains,
    );
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
                  <small>
                    {languageLabel(props.state.configDraft.language)}
                  </small>
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
                              props.state.themeMode === option.id && "selected",
                            )}
                            onClick={() => props.onTheme(option.id)}
                          >
                            {option.label}
                            <Show when={option.id === systemThemeMode()}>
                              {" "}
                              ({t("default")})
                            </Show>
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
                        props.onMainFont(
                          option.id === "system" ? "" : option.value,
                        )
                      }
                    />
                  </div>
                  <div class="field-row">
                    <span>{t("codeFont")}</span>
                    <AppearanceSelect
                      value={props.state.codeFont || DEFAULT_CODE_FONT}
                      options={codeFontOptions()}
                      onSelect={(option) =>
                        props.onCodeFont(
                          option.value === DEFAULT_CODE_FONT
                            ? ""
                            : option.value,
                        )
                      }
                    />
                  </div>
                  <div class="field-row">
                    <span>{t("mainFontSize")}</span>
                    <AppearanceSelect
                      value={String(props.state.mainFontSize)}
                      options={sizeOptions(11, 15, 12)}
                      onSelect={(option) =>
                        props.onMainFontSize(Number(option.value))
                      }
                    />
                  </div>
                  <div class="field-row">
                    <span>{t("codeFontSize")}</span>
                    <AppearanceSelect
                      value={String(props.state.codeFontSize)}
                      options={sizeOptions(9, 15, 11)}
                      onSelect={(option) =>
                        props.onCodeFontSize(Number(option.value))
                      }
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
                          class={classNames(
                            providerDomainFilter() === domain && "selected",
                          )}
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
                      each={(props.state.modelConfig?.tiers ?? []).filter(
                        (tier) => MODEL_SETTINGS_TIERS.includes(tier.tier),
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
                                if (
                                  option.value === CONFIGURE_PROVIDER_OPTION
                                ) {
                                  props.onConfigureProviders();
                                  return;
                                }
                                const modelOption = tier.options.find(
                                  (item) =>
                                    modelOptionValue(item) === option.value,
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

function AgentSettingsPanel(props: {
  agents: Agent[];
  saving: boolean;
  modelConfig?: TuraConfigResponse;
  onRefresh: () => Promise<void>;
  onGetAgent: (agentId: string) => Promise<StoredAgent | undefined>;
  onSaveAgent: (
    agentId: string | undefined,
    payload: AgentUpsertRequest,
  ) => Promise<void>;
}) {
  const [selectedAgentId, setSelectedAgentId] = createSignal<string>();
  const [storedAgent, setStoredAgent] = createSignal<StoredAgent>();
  const [selectedTier, setSelectedTier] =
    createSignal<(typeof AGENT_MODEL_TIERS)[number]>("thinking");
  const [selectedReasoningEffort, setSelectedReasoningEffort] =
    createSignal<(typeof AGENT_REASONING_EFFORTS)[number]>("medium");
  const [priorityEnabled, setPriorityEnabled] = createSignal(false);
  const [loadingAgent, setLoadingAgent] = createSignal(false);
  const [agentQuery, setAgentQuery] = createSignal("");
  const visibleAgents = createMemo(() =>
    visibleConfigurableAgents(props.agents),
  );
  const selectedAgent = createMemo(() =>
    visibleAgents().find((agent) => agent.name === selectedAgentId()),
  );
  const filteredAgents = createMemo(() => {
    const query = agentQuery().trim().toLowerCase();
    if (!query) {
      return visibleAgents();
    }
    return visibleAgents().filter((agent) =>
      `${agentDisplayName(agent)} ${agent.description} ${agent.mode}`
        .toLowerCase()
        .includes(query),
    );
  });
  const configuredAgentCount = createMemo(() => filteredAgents().length);
  const selectedCapabilities = createMemo(() =>
    capabilitiesForAgent(selectedAgent(), storedAgent()),
  );
  const modelTierOptions = createMemo(() =>
    AGENT_MODEL_TIERS.map((tier) => ({
      id: tier,
      label: modelTierLabel(tier),
      value: tier,
      detail: modelForTier(props.modelConfig, tier),
      preview: "inherit",
    })),
  );
  const reasoningEffortOptions = createMemo(() =>
    AGENT_REASONING_EFFORTS.map((effort) => ({
      id: effort,
      label: reasoningEffortLabel(effort),
      value: effort,
      preview: "inherit",
    })),
  );

  createEffect(() => {
    if (!selectedAgentId() && filteredAgents().length > 0) {
      void selectAgent(filteredAgents()[0]!);
    }
  });

  async function selectAgent(agent: Agent) {
    setSelectedAgentId(agent.name);
    setLoadingAgent(true);
    const stored = await props.onGetAgent(agent.name);
    setStoredAgent(stored);
    setSelectedTier(normalizeAgentModelTier(agentModelTier(agent, stored)));
    setSelectedReasoningEffort(
      normalizeReasoningEffort(agentReasoningEffort(agent, stored)),
    );
    setPriorityEnabled(agentPriorityEnabled(agent, stored));
    setLoadingAgent(false);
  }

  async function saveAgentSettings() {
    const agent = selectedAgent();
    const stored = storedAgent();
    if (!agent || !stored) {
      return;
    }
    const payload: AgentUpsertRequest = {
      config: agentConfigWithProviderSettings(stored.config, {
        tier: selectedTier(),
        reasoningEffort: selectedReasoningEffort(),
        priority: priorityEnabled(),
      }),
      prompt: stored.prompt ?? undefined,
    };
    await props.onSaveAgent(agent.name, payload);
    await props.onRefresh();
    await selectAgent(agent);
  }

  return (
    <section class="settings-panel agent-settings-panel">
      <header>
        <span>{t("agentSettings")}</span>
        <small>{visibleAgents().length}</small>
      </header>
      <div class="agent-settings-layout">
        <div class="settings-list agent-list">
          <label class="workspace-search-row provider-search-row agent-search-row">
            <Search size={14} strokeWidth={1.7} />
            <input
              class="workspace-search"
              value={agentQuery()}
              placeholder={`${t("search")}...`}
              onInput={(event) => setAgentQuery(event.currentTarget.value)}
            />
          </label>
          <div class="settings-list provider-config-list agent-config-list">
            <div class="provider-config-group agent-configured-group">
              <div class="provider-config-group-title">
                <span>默认智能体</span>
                <small>{configuredAgentCount()}</small>
              </div>
              <div class="workspace-picker-list agent-list-scroll">
                <For each={filteredAgents()}>
                  {(agent) => (
                    <button
                      type="button"
                      class={classNames(
                        "workspace-pick-row",
                        "agent-pick-row",
                        selectedAgentId() === agent.name && "selected",
                      )}
                      onClick={() => void selectAgent(agent)}
                    >
                      <AgentIcon agent={agent} />
                      <span>{agentDisplayName(agent)}</span>
                      <small>
                        {modelTierLabel(
                          normalizeAgentModelTier(agentModelTier(agent)),
                        )}
                      </small>
                    </button>
                  )}
                </For>
              </div>
            </div>
          </div>
        </div>
        <div class="settings-fields agent-editor">
          <Show when={loadingAgent()}>
            <div class="settings-inline-loading" aria-label={t("loading")}>
              <div class="loading-bar wide" />
              <div class="loading-bar medium" />
            </div>
          </Show>
          <ReadonlyRow
            label={t("agentName")}
            value={agentDisplayName(selectedAgent(), storedAgent())}
          />
          <ReadonlyRow
            label={t("description")}
            value={
              storedAgent()?.summary.description ??
              selectedAgent()?.description ??
              ""
            }
          />
          <div class="field-row">
            <label for="agent-settings-model">{t("model")}</label>
            <AppearanceSelect
              value={selectedTier()}
              options={modelTierOptions()}
              onSelect={(option) =>
                setSelectedTier(normalizeAgentModelTier(option.value))
              }
            />
          </div>
          <div class="field-row">
            <label for="agent-settings-reasoning">
              {t("modelReasoningEffort")}
            </label>
            <AppearanceSelect
              value={selectedReasoningEffort()}
              options={reasoningEffortOptions()}
              onSelect={(option) =>
                setSelectedReasoningEffort(
                  normalizeReasoningEffort(option.value),
                )
              }
            />
          </div>
          <div class="field-row">
            <span>{t("modelPriority")}</span>
            <div class="segmented two agent-priority-segmented">
              <button
                type="button"
                class={classNames(priorityEnabled() && "selected")}
                onClick={() => setPriorityEnabled(true)}
              >
                {t("enabled")}
              </button>
              <button
                type="button"
                class={classNames(!priorityEnabled() && "selected")}
                onClick={() => setPriorityEnabled(false)}
              >
                {t("disabled")}
              </button>
            </div>
          </div>
          <div class="field-row agent-capabilities-row">
            <span>{t("capabilities")}</span>
            <div class="agent-capability-list">
              <Show
                when={selectedCapabilities().length > 0}
                fallback={<span class="settings-note">暂无能力</span>}
              >
                <For each={selectedCapabilities()}>
                  {(capability) => <code>{capability}</code>}
                </For>
              </Show>
            </div>
          </div>
          <div class="settings-actions-row agent-actions-row">
            <button
              type="button"
              class="primary"
              disabled={!selectedAgent() || !storedAgent() || props.saving}
              aria-busy={props.saving}
              onClick={() => void saveAgentSettings()}
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

function agentModelTier(agent?: Agent, stored?: StoredAgent): string {
  return (
    readProviderTier(stored?.config.provider) ??
    readProviderTier(agent?.options?.provider) ??
    readProviderTier(agent?.options) ??
    "thinking"
  );
}

function normalizeAgentModelTier(
  value: string | undefined,
): (typeof AGENT_MODEL_TIERS)[number] {
  return AGENT_MODEL_TIERS.includes(value as (typeof AGENT_MODEL_TIERS)[number])
    ? (value as (typeof AGENT_MODEL_TIERS)[number])
    : "thinking";
}

function agentReasoningEffort(agent?: Agent, stored?: StoredAgent): string {
  return (
    readProviderString(stored?.config.provider, [
      "model_reasoning_effort",
      "reasoning_effort",
      "model_variant",
    ]) ??
    readProviderString(agent?.options?.provider, [
      "model_reasoning_effort",
      "reasoning_effort",
      "model_variant",
    ]) ??
    "medium"
  );
}

function normalizeReasoningEffort(
  value: string | undefined,
): (typeof AGENT_REASONING_EFFORTS)[number] {
  return value === "medium" ||
    value === "high" ||
    value === "xhigh" ||
    value === "highest"
    ? value === "highest"
      ? "xhigh"
      : value
    : "medium";
}

function reasoningEffortLabel(value: string): string {
  const labels: Record<string, TextKey> = {
    high: "modelReasoningEffortHigh",
    low: "modelReasoningEffortLow",
    medium: "modelReasoningEffortMedium",
    xhigh: "modelReasoningEffortXHigh",
  };
  return labels[value] ? t(labels[value]) : value;
}

function agentPriorityEnabled(agent?: Agent, stored?: StoredAgent): boolean {
  const configured =
    readProviderBool(stored?.config.provider, "model_acceleration_enabled") ??
    readProviderBool(agent?.options?.provider, "model_acceleration_enabled");
  if (configured !== undefined) {
    return configured;
  }
  return (
    readProviderString(stored?.config.provider, ["service_tier"]) ===
      "priority" ||
    readProviderString(agent?.options?.provider, ["service_tier"]) ===
      "priority"
  );
}

function readProviderTier(value: unknown): string | undefined {
  return readProviderString(value, ["tura_llm_name"]);
}

function readProviderString(
  value: unknown,
  keys: string[],
): string | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  const record = value as Record<string, unknown>;
  for (const key of keys) {
    const field = record[key];
    if (typeof field === "string" && field.trim()) {
      return field.trim();
    }
  }
  return undefined;
}

function readProviderBool(value: unknown, key: string): boolean | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  const field = (value as Record<string, unknown>)[key];
  return typeof field === "boolean" ? field : undefined;
}

function agentConfigWithProviderSettings(
  config: AgentConfig,
  settings: {
    tier: (typeof AGENT_MODEL_TIERS)[number];
    reasoningEffort: (typeof AGENT_REASONING_EFFORTS)[number];
    priority: boolean;
  },
): AgentConfig {
  const provider =
    config.provider &&
    typeof config.provider === "object" &&
    !Array.isArray(config.provider)
      ? { ...(config.provider as Record<string, unknown>) }
      : {};
  return {
    ...config,
    provider: {
      ...provider,
      tura_llm_name: settings.tier,
      model_reasoning_effort: settings.reasoningEffort,
      model_acceleration_enabled: settings.priority,
      service_tier: settings.priority ? "priority" : "default",
    },
  };
}

function capabilitiesForAgent(agent?: Agent, stored?: StoredAgent): string[] {
  const values = [
    ...(stored?.summary.capabilities ?? []),
    ...readStringList(stored?.config.agent_capabilities),
    ...(Array.isArray(agent?.options?.capabilities)
      ? (agent!.options.capabilities as unknown[])
          .map((item) => (typeof item === "string" ? item : undefined))
          .filter((item): item is string => !!item)
      : []),
  ];
  return [...new Set(values)].sort();
}

function modelForTier(
  modelConfig: TuraConfigResponse | undefined,
  tier: string,
): string {
  const current = modelConfig?.tiers.find(
    (item) => item.tier === tier,
  )?.current;
  return current ? modelOptionValue(current) : "--";
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
  const [avatar, setAvatar] = createSignal<AvatarRenderSettings>(
    props.savedAvatar,
  );
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
          <AgentAvatarSettings
            media={selectedMedia()}
            value={avatar()}
            onChange={setAvatar}
          />
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
  const displayModeOptions: Array<{ value: AvatarDisplayMode; label: string }> =
    [
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
                  class={classNames(
                    props.value.display_mode === option.value && "selected",
                  )}
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
      <div
        class="range-control"
        style={{ "--range-progress": `${progress()}%` }}
      >
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
    personas.find((persona) => persona.summary.id === personaId)?.summary
      .media ??
    personas.find((persona) => persona.summary.id === personaId)?.config
      .media ??
    undefined
  );
}

function readStringList(value: unknown): string[] {
  return Array.isArray(value)
    ? value
        .map((item) => {
          if (typeof item === "string") {
            return item;
          }
          if (
            item &&
            typeof item === "object" &&
            "capability_name" in item &&
            typeof item.capability_name === "string"
          ) {
            return item.capability_name;
          }
          return undefined;
        })
        .filter((item): item is string => !!item)
    : [];
}

export function ReadonlyRow(props: { label: string; value: string }) {
  return (
    <div class="field-row readonly-row">
      <span>{props.label}</span>
      <code>{props.value}</code>
    </div>
  );
}
