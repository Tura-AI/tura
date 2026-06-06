import en from "./locales/en.json" with { type: "json" };
import zhCN from "./locales/zh-CN.json" with { type: "json" };

export type Language = "zh-CN" | "en";

const dictionaries = {
  "zh-CN": zhCN,
  en,
} as const satisfies Record<Language, Record<string, string>>;

export type TextKey = keyof (typeof dictionaries)["zh-CN"];

let languageOverride: Language | undefined;

export function setLanguage(value: Language | undefined): void {
  languageOverride = value;
}

export function parseLanguage(value: string | undefined): Language | undefined {
  if (!value) return undefined;
  const normalized = value.toLowerCase();
  if (normalized === "zh" || normalized === "zh-cn" || normalized === "cn") return "zh-CN";
  if (normalized === "en" || normalized === "en-us" || normalized === "en-gb") return "en";
  return undefined;
}

export function currentLanguage(): Language {
  return (
    languageOverride ??
    parseLanguage(process.env.TURA_LANG) ??
    parseLanguage(process.env.LANG) ??
    "zh-CN"
  );
}

export function t(key: TextKey, values?: Record<string, string | number>): string {
  const language = currentLanguage();
  let template = dictionaries[language][key] ?? dictionaries.en[key];
  if (!values) return template;
  for (const [name, value] of Object.entries(values)) {
    template = template.replaceAll(`{${name}}`, String(value));
  }
  return template;
}

export function assertDictionaryParity(): void {
  const zh = Object.keys(dictionaries["zh-CN"]).sort();
  const en = Object.keys(dictionaries.en).sort();
  if (zh.join("\n") !== en.join("\n")) {
    throw new Error("TUI i18n dictionaries must keep zh-CN and en keys in sync");
  }
}
