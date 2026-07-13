import { t } from "./i18n.js";
import type { StoredPersona } from "./types/gateway.js";

export function personaDescription(persona: StoredPersona): string {
  const id = persona.summary?.id ?? persona.config?.persona_name;
  switch (id) {
    case "tura":
      return t("personaDescriptionTura");
    case "wonderful":
      return t("personaDescriptionWonderful");
    case "pidan":
      return t("personaDescriptionPidan");
    default:
      return (
        persona.summary?.short_description ??
        persona.config?.short_description ??
        persona.summary?.description ??
        persona.config?.description ??
        ""
      );
  }
}

export function personaCommunicationStyle(persona: StoredPersona): string {
  const id = persona.summary?.id ?? persona.config?.persona_name;
  if (id === "tura" || id === "wonderful" || id === "pidan") return "";
  return typeof persona.communication_style === "string" ? persona.communication_style.trim() : "";
}
