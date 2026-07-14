import { en } from "./i18n/en";
import type { Dictionary, Language, TextKey } from "./i18n/types";
import { zhCN } from "./i18n/zh-CN";
import { createSignal } from "solid-js";

export type { Language, TextKey };

const dictionaries: Record<Language, Dictionary> = {
  "zh-CN": zhCN,
  en,
};

const defaultLanguage: Language = "en";

const [language, setLanguageSignal] = createSignal<Language>(defaultLanguage);

export const LANGUAGE_OPTIONS: Array<{ id: Language; label: string }> = [
  { id: "en", label: "English" },
  { id: "zh-CN", label: "简体中文" },
];

export function parseLanguage(value: string | undefined | null): Language | undefined {
  if (!value) {
    return undefined;
  }
  const normalized = value.trim().toLowerCase();
  if (normalized === "zh" || normalized === "zh-cn" || normalized === "cn") {
    return "zh-CN";
  }
  if (normalized === "en" || normalized === "en-us" || normalized === "en-gb") {
    return "en";
  }
  return undefined;
}

export function setLanguage(value: string | undefined | null): void {
  if (!value) {
    setLanguageSignal(defaultLanguage);
    return;
  }
  const parsed = parseLanguage(value);
  if (parsed) {
    setLanguageSignal(parsed);
  }
}

export function currentLanguage(): Language {
  return language();
}

export function t(key: TextKey, values?: Record<string, string | number>): string {
  const template = dictionaries[language()][key] ?? dictionaries.en[key];
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
