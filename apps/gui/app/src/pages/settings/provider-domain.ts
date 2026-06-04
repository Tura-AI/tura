import type { SdkProvider } from "@tura/gateway-sdk";

export function providerDomains(provider: SdkProvider): string[] {
  const directDomains = [
    ...(Array.isArray(provider.domains) ? provider.domains : []),
    ...(Array.isArray(provider.domain) ? provider.domain : []),
    ...(typeof provider.domain === "string" ? [provider.domain] : []),
  ];
  const optionDomains = provider.options.domains;
  const capabilities = providerCapabilities(provider);
  const domains = [
    ...directDomains,
    ...(Array.isArray(optionDomains)
      ? optionDomains.filter(
          (domain): domain is string => typeof domain === "string",
        )
      : []),
  ];
  const normalized = [...new Set(domains.filter(Boolean))];
  if (normalized.length > 0) {
    return normalized;
  }
  if (capabilities.some((capability) => capability.startsWith("llm."))) {
    return ["llm"];
  }
  if (Object.keys(provider.models).length > 0) {
    return ["llm"];
  }
  return ["other"];
}

function providerCapabilities(provider: SdkProvider): string[] {
  const value = provider.options.capabilities;
  return Array.isArray(value)
    ? value.filter(
        (capability): capability is string => typeof capability === "string",
      )
    : [];
}
