use std::sync::OnceLock;

/// The ring-backed rustls provider could not be installed for this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RustlsCryptoProviderError;

impl std::fmt::Display for RustlsCryptoProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            "the ring-backed rustls crypto provider could not be installed; another provider may already be configured",
        )
    }
}

impl std::error::Error for RustlsCryptoProviderError {}

/// Install the ring-backed rustls provider once for the current process.
///
/// Reqwest is built with `rustls-no-provider`, so every process that may build
/// a reqwest client must call this before the first client construction. The
/// result is cached so library callers and application roots can safely share
/// this boundary without attempting the process-global rustls installation
/// more than once.
pub fn install_rustls_crypto_provider() -> Result<(), RustlsCryptoProviderError> {
    static INSTALL_RESULT: OnceLock<Result<(), RustlsCryptoProviderError>> = OnceLock::new();

    *INSTALL_RESULT.get_or_init(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .map_err(|_| RustlsCryptoProviderError)
    })
}

#[cfg(test)]
mod tests {
    use super::install_rustls_crypto_provider;

    #[test]
    fn ring_provider_install_is_idempotent_and_builds_reqwest_client() {
        install_rustls_crypto_provider().expect("install ring crypto provider");
        install_rustls_crypto_provider().expect("reuse ring crypto provider");
        reqwest::Client::builder()
            .build()
            .expect("build reqwest client with ring provider");
    }
}
