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
  type JSX,
} from "solid-js";
import { Portal } from "solid-js/web";
import LayoutList from "lucide-solid/icons/layout-list";
import ArrowLeft from "lucide-solid/icons/arrow-left";
import Check from "lucide-solid/icons/check";
import ChevronDown from "lucide-solid/icons/chevron-down";
import FolderSearch from "lucide-solid/icons/folder-search";
import MessageSquare from "lucide-solid/icons/message-square";
import Search from "lucide-solid/icons/search";
import {
  type SdkProvider,
  type TuraConfigModelPair,
  type TuraConfigResponse,
} from "@tura/gateway-sdk";
import {
  systemThemeMode,
  type AppState,
  type MainTab,
  type SettingsSection,
  type ThemeMode,
} from "../../state/global-store";
import { classNames } from "../../state/format";
import { activeLanguage, t, type TextKey } from "../../i18n";

import { ProviderConfigGroup } from "./provider-settings";
import {
  providerConfigured,
} from "../../utils/settings";
import { settingsRoutes, settingsRouteTitle } from "./settings-router";
const DEFAULT_MAIN_FONT =
  'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
const DEFAULT_CODE_FONT =
  'ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace';
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
type AppearanceOption = {
  id: string;
  label: string;
  value: string;
  preview: string;
  size?: number;
};

type AppearanceSelectFooter = {
  label: string;
  onSelect: () => void;
};

const CONFIGURE_PROVIDER_OPTION = "__configure_provider__";

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
  return t(PROVIDER_DOMAIN_LABELS[domain] ?? "unknown");
}

function providerDomains(provider: SdkProvider): string[] {
  const directDomains = [
    ...(Array.isArray(provider.domains) ? provider.domains : []),
    ...(Array.isArray(provider.domain) ? provider.domain : []),
    ...(typeof provider.domain === "string" ? [provider.domain] : []),
  ];
  const optionDomains = provider.options.domains;
  const domains = [
    ...directDomains,
    ...(Array.isArray(optionDomains)
      ? optionDomains.filter(
          (domain): domain is string => typeof domain === "string",
        )
      : []),
  ];
  return [...new Set(domains.filter(Boolean))];
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
        <For each={settingsRoutes()}>
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
  onModelTier: (tier: string, option: TuraConfigModelPair) => void;
  onConfigureProviders: () => void;
  onTheme: (theme: ThemeMode) => void;
  onMainFont: (font: string) => void;
  onCodeFont: (font: string) => void;
  onMainFontSize: (size: number) => void;
  onCodeFontSize: (size: number) => void;
  onProviderSearch: (value: string) => void;
  onOpenProviderAuth: (providerId: string) => void;
}) {
  const providers = createMemo(() => props.state.providers?.all ?? []);
  const [providerDomainFilter, setProviderDomainFilter] = createSignal(
    DEFAULT_PROVIDER_DOMAIN,
  );
  const selectedProvider = createMemo(
    () =>
      providers().find(
        (provider) => provider.id === props.state.selectedProviderId,
      ) ?? providers()[0],
  );
  const title = createMemo(
    () => settingsRouteTitle(props.section),
  );
  const providerDomainOptions = createMemo(() => {
    const domains = new Set<string>([
      ...(props.state.providers?.enums.domains ?? []),
      ...providers().flatMap(providerDomains),
      DEFAULT_PROVIDER_DOMAIN,
    ]);
    return [...domains].sort((left, right) => {
      if (left === DEFAULT_PROVIDER_DOMAIN) {
        return -1;
      }
      if (right === DEFAULT_PROVIDER_DOMAIN) {
        return 1;
      }
      if (left === "other") {
        return 1;
      }
      if (right === "other") {
        return -1;
      }
      return providerDomainLabel(left).localeCompare(
        providerDomainLabel(right),
      );
    });
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
    <section class="settings-view">
      <header class="page-head">
        <div class="page-title">
          <span>{t("settings")}</span>
          <h1>{title()}</h1>
        </div>
        <div class="page-actions" />
      </header>

      <main class="settings-canvas">
        <section class="settings-stack">
          <Switch>
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
                      options={sizeOptions(12, 15, 13)}
                      onSelect={(option) =>
                        props.onMainFontSize(Number(option.value))
                      }
                    />
                  </div>
                  <div class="field-row">
                    <span>{t("codeFontSize")}</span>
                    <AppearanceSelect
                      value={String(props.state.codeFontSize)}
                      options={sizeOptions(10, 15, 12)}
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
                    <For each={props.state.modelConfig?.tiers ?? []}>
                      {(tier) => (
                        <div class="field-row">
                          <span>{modelTierLabel(tier.tier)}</span>
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
          </Switch>

          <Show when={props.state.settingsNotice}>
            <div class="settings-note">
              {props.state.settingsNotice}
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

function AppearanceSelect(props: {
  value: string;
  options: AppearanceOption[];
  placeholder?: string;
  footer?: AppearanceSelectFooter;
  onSelect: (option: AppearanceOption) => void;
}) {
  const [open, setOpen] = createSignal(false);
  const [menuPosition, setMenuPosition] = createSignal({
    left: 0,
    top: 0,
    width: 340,
    maxHeight: 320,
  });
  let root: HTMLElement | undefined;
  let menu: HTMLDivElement | undefined;
  const selected = createMemo(
    () =>
      props.options.find((option) => option.value === props.value) ??
      props.options[0],
  );
  const visibleOptions = createMemo(() =>
    props.options.length > 0
      ? props.options
      : props.footer
        ? [
            {
              id: CONFIGURE_PROVIDER_OPTION,
              label: props.footer.label,
              value: CONFIGURE_PROVIDER_OPTION,
              preview: "inherit",
            },
          ]
        : [],
  );
  const buttonLabel = createMemo(
    () => selected()?.label ?? props.placeholder ?? t("selectStep"),
  );

  function updateMenuPosition() {
    if (!root) {
      return;
    }
    const rect = root.getBoundingClientRect();
    const gap = 6;
    const viewportPadding = 16;
    const preferredWidth = Math.max(260, rect.width);
    const width = Math.min(preferredWidth, window.innerWidth - viewportPadding * 2);
    const left = Math.min(
      Math.max(viewportPadding, rect.left),
      Math.max(viewportPadding, window.innerWidth - width - viewportPadding),
    );
    const top = Math.min(
      rect.bottom + gap,
      Math.max(viewportPadding, window.innerHeight - viewportPadding - 120),
    );
    setMenuPosition({
      left,
      top,
      width,
      maxHeight: Math.max(120, window.innerHeight - top - viewportPadding),
    });
  }

  onMount(() => {
    const closeOutside = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!root?.contains(target) && !menu?.contains(target)) {
        setOpen(false);
      }
    };
    const reposition = () => {
      if (open()) {
        updateMenuPosition();
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    onCleanup(() => {
      document.removeEventListener("pointerdown", closeOutside);
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    });
  });

  createEffect(() => {
    if (open()) {
      updateMenuPosition();
    }
  });

  return (
    <section class="appearance-select" ref={root}>
      <button
        type="button"
        class="appearance-select-button"
        style={{
          "font-family": selected()?.preview,
          "font-size": selected()?.size ? `${selected()!.size}px` : undefined,
        }}
        onClick={(event) => {
          event.preventDefault();
          const nextOpen = !open();
          setOpen(nextOpen);
          if (nextOpen) {
            updateMenuPosition();
          }
        }}
      >
        <span>{buttonLabel()}</span>
        <ChevronDown size={13} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <Portal>
          <div
            ref={menu}
            class="plan-session-menu appearance-select-menu"
            style={{
              left: `${menuPosition().left}px`,
              top: `${menuPosition().top}px`,
              width: `${menuPosition().width}px`,
              "max-height": `${menuPosition().maxHeight}px`,
            }}
            onPointerDown={(event) => event.stopPropagation()}
          >
            <For each={visibleOptions()}>
              {(option) => (
                <button
                  type="button"
                  class={classNames(
                    "plan-trigger-option",
                    props.value === option.value && "selected",
                  )}
                  style={{
                    "font-family": option.preview,
                    "font-size": option.size ? `${option.size}px` : undefined,
                  }}
                  onClick={(event) => {
                    event.preventDefault();
                    props.onSelect(option);
                    setOpen(false);
                  }}
                >
                  <span>{option.label}</span>
                  <Show when={props.value === option.value}>
                    <Check size={14} strokeWidth={1.8} />
                  </Show>
                </button>
              )}
            </For>
            <Show when={props.options.length > 0 ? props.footer : undefined}>
              {(footer) => (
                <button
                  type="button"
                  class="plan-trigger-option appearance-select-footer"
                  onClick={(event) => {
                    event.preventDefault();
                    footer().onSelect();
                    setOpen(false);
                  }}
                >
                  <span>{footer().label}</span>
                </button>
              )}
            </Show>
          </div>
        </Portal>
      </Show>
    </section>
  );
}
