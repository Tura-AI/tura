#![deny(clippy::unwrap_used)]
#![forbid(unsafe_code)]

pub mod auth_registry;
pub mod content_type_fallback;
pub mod llm;
pub mod logging;
pub mod metrics;
pub mod response_extraction;
pub mod streaming;
pub mod tura_conf;
pub mod tura_llm;
pub mod tura_llm_conf;
pub mod utils;

pub use auth_registry::*;
pub use content_type_fallback::{
    provider_media_fallback, provider_unsupported_content_type,
    replace_unsupported_content_type_in_messages, ProviderMediaFallback,
};
pub use response_extraction::{
    extract_response_text, extract_tool_calls, openai_compatible_usage_stream_supported,
    prompt_cache_key_supported, strip_thought_blocks, ProviderToolCall,
};
pub use tura_conf::*;
pub use tura_llm::*;
pub use tura_llm_conf::*;
pub use utils::*;

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::OnceLock;

    use tokio::sync::{Mutex, MutexGuard};

    fn env_mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
        env_mutex().blocking_lock()
    }

    pub(crate) async fn env_lock_async() -> MutexGuard<'static, ()> {
        env_mutex().lock().await
    }
}
