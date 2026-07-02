import type { TuraConfigModelPair, TuraConfigResponse } from "@tura/gateway-sdk";
import { currentLanguage, LANGUAGE_OPTIONS, t, type TextKey } from "../../i18n";
import { DEFAULT_CODE_FONT } from "../../config/defaults";
import type { CornerRadiusMode, ThemeMode } from "../../state/global-store";
import type { AppearanceOption } from "./appearance-select";

export const THEME_OPTIONS: Array<{
  id: ThemeMode;
  label: string;
}> = [
  {
    id: "light",
    get label() {
      return t("light");
    },
  },
  {
    id: "dark",
    get label() {
      return t("dark");
    },
  },
  {
    id: "caral",
    label: "Caral",
  },
  {
    id: "uruk",
    label: "Uruk",
  },
  {
    id: "liangzhu",
    label: "Liangzhu",
  },
];

export const CORNER_RADIUS_OPTIONS: Array<{
  id: CornerRadiusMode;
  label: string;
  value: CornerRadiusMode;
  preview: string;
}> = [
  { id: "0px", label: "0px", value: "0px", preview: "inherit" },
  { id: "2px", label: "2px", value: "2px", preview: "inherit" },
  { id: "8px", label: "8px", value: "8px", preview: "inherit" },
  { id: "9.6px", label: "9.6px", value: "9.6px", preview: "inherit" },
];
export const DEFAULT_PROVIDER_DOMAIN = "llm";
const PROVIDER_DOMAIN_LABELS: Record<string, TextKey> = {
  communication: "domainCommunication",
  infrastructure: "domainInfrastructure",
  llm: "domainLlm",
  other: "domainOther",
  productivity: "domainProductivity",
  search: "domainSearch",
};
export const DEFAULT_MODEL_TIERS = ["thinking", "fast"] as const;
export const AGENT_REASONING_EFFORTS = ["low", "medium", "high", "xhigh"] as const;
export const DEFAULT_MODEL_TIER_CONFIG_TIERS = ["thinking", "fast"];
export { LANGUAGE_OPTIONS };

type FontLocale = "en" | "zhHans" | "zhHant" | "es" | "hi" | "ar" | "pt" | "bn" | "ru" | "ja";
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
  return currentLanguage() === "zh-CN" ? "zhHans" : "en";
}

function fontFamilyValue(fonts: Record<FontLocale, string>, preferred: FontLocale): string {
  const ordered = [preferred, ...FONT_LOCALE_ORDER.filter((locale) => locale !== preferred)];
  return [
    ...new Set(ordered.map((locale) => fonts[locale])),
    "ui-sans-serif",
    "system-ui",
    "sans-serif",
  ].join(", ");
}

export function mainFontOptions(): AppearanceOption[] {
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

export function codeFontOptions(): AppearanceOption[] {
  return CODE_FONT_OPTIONS.map((font) => ({
    id: font.label,
    label: font.label,
    value: font.value,
    preview: font.value,
  }));
}

export function providerDomainLabel(domain: string): string {
  const label = PROVIDER_DOMAIN_LABELS[domain];
  return label ? t(label) : domain;
}

export function compareProviderDomains(left: string, right: string): number {
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

export function sizeOptions(min: number, max: number, defaultSize: number): AppearanceOption[] {
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

export function modelOptionValue(
  option?: Pick<TuraConfigModelPair, "provider" | "model"> | null,
): string {
  return option ? `${option.provider}/${option.model}` : "";
}

export function languageLabel(value: string | undefined): string {
  return LANGUAGE_OPTIONS.find((option) => option.id === value)?.label ?? LANGUAGE_OPTIONS[0].label;
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

export function modelTierOptions(tier: TuraConfigResponse["tiers"][number]): AppearanceOption[] {
  const options = tier.options.map(modelConfigOption);
  const currentValue = modelOptionValue(tier.current);
  if (currentValue && !options.some((option) => option.value === currentValue) && tier.current) {
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

export function modelTierLabel(tier: string): string {
  const labels: Record<string, TextKey> = {
    embedding_high: "modelTierEmbeddingHigh",
    embedding_low: "modelTierEmbeddingLow",
    fast: "modelTierFast",
    thinking: "modelTierThinking",
  };
  return labels[tier] ? t(labels[tier]) : tier;
}

export function canonicalDefaultModelTier(
  value: string | undefined,
): (typeof DEFAULT_MODEL_TIERS)[number] {
  switch (value?.trim().toLowerCase()) {
    case "fast":
    case "instant":
      return "fast";
    case "flagship":
    case "flagship_thinking":
    case "thinking":
    default:
      return "thinking";
  }
}
