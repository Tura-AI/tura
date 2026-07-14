import type { StoredPersona } from "@tura/gateway-sdk";
import { t } from "../i18n";

export function personaDescription(persona: StoredPersona): string {
  switch (persona.summary.id) {
    case "tura":
      return t("personaDescriptionTura");
    case "wonderful":
      return t("personaDescriptionWonderful");
    case "pidan":
      return t("personaDescriptionPidan");
    default:
      return (
        persona.summary.short_description ||
        persona.config.short_description ||
        persona.summary.description ||
        persona.config.description ||
        ""
      );
  }
}
