import { t } from "../../i18n";
import type { MainTab } from "../../state/global-store";

export type MainTabEntry = {
  id: Exclude<MainTab, "settings">;
  label: string;
};

export function mainTabEntries(conversationLabel = t("session")): MainTabEntry[] {
  return [{ id: "conversation", label: conversationLabel }];
}
