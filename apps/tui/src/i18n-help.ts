import { createRequire } from "node:module";
import type { HelpPage } from "./output/help.js";
import { currentLanguage } from "./i18n.js";

export type HelpTopic =
  | "main"
  | "run"
  | "resume"
  | "session"
  | "config"
  | "provider"
  | "agent"
  | "completion"
  | "persona"
  | "project"
  | "file"
  | "command"
  | "inspect"
  | "gateway";

const requireHelp = createRequire(import.meta.url);
const pages = {
  "zh-CN": requireHelp("./locales/help.zh-CN.json"),
  en: requireHelp("./locales/help.en.json"),
} as const satisfies Record<string, Record<HelpTopic, HelpPage>>;

export function helpPage(topic: HelpTopic): HelpPage {
  return pages[currentLanguage()][topic] ?? pages.en[topic];
}
