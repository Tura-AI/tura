import { t } from "../../i18n";
import type { SettingsSection } from "../../state/global-store";

export type SettingsRoute = {
  id: SettingsSection;
  label: string;
};

export function settingsRoutes(): SettingsRoute[] {
  return [
    { id: "appearance", label: t("appearance") },
    { id: "providers", label: t("providers") },
    { id: "models", label: t("models") },
  ];
}

export function settingsRouteTitle(section: SettingsSection): string {
  return (
    settingsRoutes().find((route) => route.id === section)?.label ??
    t("settings")
  );
}
