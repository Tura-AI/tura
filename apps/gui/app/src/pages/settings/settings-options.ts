import type { TuraConfigModelPair, TuraConfigResponse } from "@tura/gateway-sdk";
import { currentLanguage, LANGUAGE_OPTIONS, t, type TextKey } from "../../i18n";
import { DEFAULT_CODE_FONT, DEFAULT_MAIN_FONT } from "../../config/defaults";
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
  | "ja"
  | "ko"
  | "th"
  | "he"
  | "vi";
const FONT_LOCALE_ORDER: FontLocale[] = [
  "en",
  "es",
  "pt",
  "vi",
  "ru",
  "zhHans",
  "zhHant",
  "ja",
  "ko",
  "hi",
  "ar",
  "bn",
  "th",
  "he",
];

const MAIN_FONT_MAP = [
  {
    id: "archivo-plex",
    names: {
      en: "Archivo",
      zhHans: "IBM Plex Sans SC",
      zhHant: "IBM Plex Sans TC",
      es: "Archivo",
      hi: "IBM Plex Sans Devanagari",
      ar: "IBM Plex Sans Arabic",
      pt: "Archivo",
      bn: "IBM Plex Sans Bengali",
      ru: "IBM Plex Sans",
      ja: "IBM Plex Sans JP",
      ko: "IBM Plex Sans KR",
      th: "IBM Plex Sans Thai",
      he: "IBM Plex Sans Hebrew",
      vi: "Archivo",
    },
    families: {
      en: '"Archivo"',
      zhHans: '"IBM Plex Sans SC"',
      zhHant: '"IBM Plex Sans TC"',
      es: '"Archivo"',
      hi: '"IBM Plex Sans Devanagari"',
      ar: '"IBM Plex Sans Arabic"',
      pt: '"Archivo"',
      bn: '"IBM Plex Sans Bengali"',
      ru: '"IBM Plex Sans"',
      ja: '"IBM Plex Sans JP"',
      ko: '"IBM Plex Sans KR"',
      th: '"IBM Plex Sans Thai"',
      he: '"IBM Plex Sans Hebrew"',
      vi: '"Archivo"',
    },
  },
  {
    id: "plex-sans",
    names: {
      en: "IBM Plex Sans",
      zhHans: "IBM Plex Sans SC",
      zhHant: "IBM Plex Sans TC",
      es: "IBM Plex Sans",
      hi: "IBM Plex Sans Devanagari",
      ar: "IBM Plex Sans Arabic",
      pt: "IBM Plex Sans",
      bn: "IBM Plex Sans Bengali",
      ru: "IBM Plex Sans",
      ja: "IBM Plex Sans JP",
      ko: "IBM Plex Sans KR",
      th: "IBM Plex Sans Thai",
      he: "IBM Plex Sans Hebrew",
      vi: "IBM Plex Sans",
    },
    families: {
      en: '"IBM Plex Sans"',
      zhHans: '"IBM Plex Sans SC"',
      zhHant: '"IBM Plex Sans TC"',
      es: '"IBM Plex Sans"',
      hi: '"IBM Plex Sans Devanagari"',
      ar: '"IBM Plex Sans Arabic"',
      pt: '"IBM Plex Sans"',
      bn: '"IBM Plex Sans Bengali"',
      ru: '"IBM Plex Sans"',
      ja: '"IBM Plex Sans JP"',
      ko: '"IBM Plex Sans KR"',
      th: '"IBM Plex Sans Thai"',
      he: '"IBM Plex Sans Hebrew"',
      vi: '"IBM Plex Sans"',
    },
  },
  {
    id: "spline-sarasa",
    names: {
      en: "Spline Sans",
      zhHans: "Sarasa UI SC",
      zhHant: "Sarasa UI TC",
      es: "Spline Sans",
      hi: "IBM Plex Sans Devanagari",
      ar: "IBM Plex Sans Arabic",
      pt: "Spline Sans",
      bn: "IBM Plex Sans Bengali",
      ru: "Spline Sans",
      ja: "Sarasa UI J",
      ko: "Sarasa UI K",
      th: "IBM Plex Sans Thai",
      he: "IBM Plex Sans Hebrew",
      vi: "Spline Sans",
    },
    families: {
      en: '"Spline Sans"',
      zhHans: '"Sarasa UI SC"',
      zhHant: '"Sarasa UI TC"',
      es: '"Spline Sans"',
      hi: '"IBM Plex Sans Devanagari"',
      ar: '"IBM Plex Sans Arabic"',
      pt: '"Spline Sans"',
      bn: '"IBM Plex Sans Bengali"',
      ru: '"Spline Sans"',
      ja: '"Sarasa UI J"',
      ko: '"Sarasa UI K"',
      th: '"IBM Plex Sans Thai"',
      he: '"IBM Plex Sans Hebrew"',
      vi: '"Spline Sans"',
    },
  },
  {
    id: "chivo-tsanger",
    names: {
      en: "Chivo",
      zhHans: "Tsanger YuYangT SC",
      zhHant: "Tsanger YuYangT TC",
      es: "Chivo",
      hi: "IBM Plex Sans Devanagari",
      ar: "IBM Plex Sans Arabic",
      pt: "Chivo",
      bn: "IBM Plex Sans Bengali",
      ru: "Chivo",
      ja: "Tsanger YuYangT JP",
      ko: "IBM Plex Sans KR",
      th: "IBM Plex Sans Thai",
      he: "IBM Plex Sans Hebrew",
      vi: "Chivo",
    },
    families: {
      en: '"Chivo"',
      zhHans: '"Tsanger YuYangT SC"',
      zhHant: '"Tsanger YuYangT TC"',
      es: '"Chivo"',
      hi: '"IBM Plex Sans Devanagari"',
      ar: '"IBM Plex Sans Arabic"',
      pt: '"Chivo"',
      bn: '"IBM Plex Sans Bengali"',
      ru: '"Chivo"',
      ja: '"Tsanger YuYangT JP"',
      ko: '"IBM Plex Sans KR"',
      th: '"IBM Plex Sans Thai"',
      he: '"IBM Plex Sans Hebrew"',
      vi: '"Chivo"',
    },
  },
  {
    id: "hanken-lxgw",
    names: {
      en: "Hanken Grotesk",
      zhHans: "LXGW Neo XiHei",
      zhHant: "LXGW Neo XiHei TC",
      es: "Hanken Grotesk",
      hi: "IBM Plex Sans Devanagari",
      ar: "IBM Plex Sans Arabic",
      pt: "Hanken Grotesk",
      bn: "IBM Plex Sans Bengali",
      ru: "Hanken Grotesk",
      ja: "IBM Plex Sans JP",
      ko: "IBM Plex Sans KR",
      th: "IBM Plex Sans Thai",
      he: "IBM Plex Sans Hebrew",
      vi: "Hanken Grotesk",
    },
    families: {
      en: '"Hanken Grotesk"',
      zhHans: '"LXGW Neo XiHei"',
      zhHant: '"LXGW Neo XiHei TC"',
      es: '"Hanken Grotesk"',
      hi: '"IBM Plex Sans Devanagari"',
      ar: '"IBM Plex Sans Arabic"',
      pt: '"Hanken Grotesk"',
      bn: '"IBM Plex Sans Bengali"',
      ru: '"Hanken Grotesk"',
      ja: '"IBM Plex Sans JP"',
      ko: '"IBM Plex Sans KR"',
      th: '"IBM Plex Sans Thai"',
      he: '"IBM Plex Sans Hebrew"',
      vi: '"Hanken Grotesk"',
    },
  },
] as const;

const CODE_FONT_OPTIONS = [
  { label: "IBM Plex Mono (Default)", value: DEFAULT_CODE_FONT },
  { label: "Iosevka Term", value: '"Iosevka Term", "IBM Plex Mono", ui-monospace, monospace' },
  { label: "Commit Mono", value: '"Commit Mono", "IBM Plex Mono", ui-monospace, monospace' },
  {
    label: "Recursive Mono",
    value: '"Recursive Mono Linear Static", "IBM Plex Mono", ui-monospace, monospace',
  },
  { label: "Sometype Mono", value: '"Sometype Mono", "IBM Plex Mono", ui-monospace, monospace' },
] as const;

function displayFontLocale(): FontLocale {
  return currentLanguage() === "zh-CN" ? "zhHans" : "en";
}

function fontFamilyValue(fonts: Record<FontLocale, string>): string {
  return [
    ...new Set(FONT_LOCALE_ORDER.map((locale) => fonts[locale])),
    "ui-sans-serif",
    "system-ui",
    "sans-serif",
  ].join(", ");
}

export function mainFontOptions(): AppearanceOption[] {
  const locale = displayFontLocale();
  return MAIN_FONT_MAP.map((font) => {
    const value = font.id === "archivo-plex" ? DEFAULT_MAIN_FONT : fontFamilyValue(font.families);
    const localizedName = font.names[locale];
    const englishName = font.names.en;
    const defaultFont = font.id === "archivo-plex";
    return {
      id: font.id,
      label:
        defaultFont
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
