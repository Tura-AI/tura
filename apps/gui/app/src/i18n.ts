import { en } from "./i18n/en";
import type { Dictionary, Language, TextKey } from "./i18n/types";
import { zhCN } from "./i18n/zh-CN";

export type { Language, TextKey };

const dictionaries: Record<Language, Dictionary> = {
  "zh-CN": zhCN,
  en,
};

export const activeLanguage: Language = "zh-CN";

export function t(key: TextKey, values?: Record<string, string | number>): string {
  const template = dictionaries[activeLanguage][key] ?? dictionaries.en[key];
  if (!values) {
    return template;
  }
  let text = template;
  for (const [name, value] of Object.entries(values)) {
    text = text.replaceAll(`{${name}}`, String(value));
  }
  return text;
}

export function assertDictionaryParity(): void {
  const zh = Object.keys(dictionaries["zh-CN"]).sort();
  const enKeys = Object.keys(dictionaries.en).sort();
  if (zh.join("\n") !== enKeys.join("\n")) {
    throw new Error("GUI i18n dictionaries must keep zh-CN and en keys in sync");
  }
}
