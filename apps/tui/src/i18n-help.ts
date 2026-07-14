import type { HelpPage } from "./output/help.js";
import { currentLanguage } from "./i18n.js";
import enHelp from "./locales/help.en.json" with { type: "json" };
import zhCNHelp from "./locales/help.zh-CN.json" with { type: "json" };

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

const pages = {
  "zh-CN": zhCNHelp,
  en: enHelp,
} as const satisfies Record<string, Record<HelpTopic, HelpPage>>;

export function helpPage(topic: HelpTopic): HelpPage {
  return pages[currentLanguage()][topic] ?? pages.en[topic];
}
