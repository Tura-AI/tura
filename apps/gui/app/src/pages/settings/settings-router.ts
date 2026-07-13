import { t } from "../../i18n";
import type { SettingsSection } from "../../state/global-store";

export type SettingsRoute = {
  id: SettingsSection;
  label: string;
};

export function settingsRoutes(): SettingsRoute[] {
  return [
    { id: "application", label: t("applicationSettings") },
    { id: "appearance", label: t("appearance") },
    { id: "providers", label: t("providers") },
    { id: "models", label: t("models") },
    { id: "agents", label: t("agentSettings") },
    { id: "personalization", label: t("personalization") },
    { id: "about", label: t("about") },
  ];
}

export function settingsRouteTitle(section: SettingsSection): string {
  return settingsRoutes().find((route) => route.id === section)?.label ?? t("settings");
}
