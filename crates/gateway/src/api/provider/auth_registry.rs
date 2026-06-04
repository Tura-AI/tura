pub(super) fn entries() -> &'static [tura_llm_rust::ProviderAuthRegistryEntry] {
    tura_llm_rust::provider_auth_registry()
}

pub(super) fn entry(
    provider_id: &str,
) -> Option<&'static tura_llm_rust::ProviderAuthRegistryEntry> {
    tura_llm_rust::provider_auth_registry_entry(provider_id)
}
