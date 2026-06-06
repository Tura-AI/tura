use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

use crate::tura_conf::TuraConfig;

use super::{ProviderConfig, ProviderResponse, ProviderStreamEventSink, TuraError};

pub static SETTINGS: OnceLock<Arc<Settings>> = OnceLock::new();
static PROVIDER_LATENCY_TIMEOUTS: OnceLock<RwLock<ProviderLatencyTimeouts>> = OnceLock::new();
static PROVIDER_LATENCY_CONFIG: OnceLock<RwLock<ProviderLatencyConfig>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CallOptions {
    pub response_format: Option<Value>,
    pub search: bool,
    pub force_search: bool,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub n: Option<u64>,
    pub stop: Option<Value>,
    pub max_completion_tokens: Option<u64>,
    pub max_tokens: Option<u64>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub logit_bias: Option<Value>,
    pub logprobs: Option<bool>,
    pub top_logprobs: Option<u64>,
    pub seed: Option<u64>,
    pub user: Option<String>,
    pub safety_identifier: Option<String>,
    pub prompt_cache_key: Option<String>,
    pub codex_session_id: Option<String>,
    pub reasoning_effort: Option<String>,
    pub prediction: Option<Value>,
    pub modalities: Option<Vec<String>>,
    pub audio: Option<Value>,
    pub stream: Option<bool>,
    pub stream_options: Option<Value>,
    pub store: Option<bool>,
    pub metadata: Option<HashMap<String, String>>,
    pub service_tier: Option<String>,
    pub verbosity: Option<String>,
    pub web_search_options: Option<Value>,
    pub tools: Option<Vec<Value>>,
    pub tool_choice: Option<Value>,
    pub parallel_tool_calls: Option<bool>,
    pub extra_body: Option<Value>,
    pub context_window: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    pub default_temperature: f64,
    pub providers: Vec<ProviderConfig>,
}

impl RouteConfig {
    pub fn validate(&self) -> Result<(), TuraError> {
        if !(0.0..=2.0).contains(&self.default_temperature) {
            return Err(TuraError::Validation {
                message: "default_temperature must be within [0.0, 2.0]".into(),
            });
        }
        for p in &self.providers {
            p.validate()?;
        }
        Ok(())
    }

    pub fn provider(&self, name: &str) -> Result<&ProviderConfig, TuraError> {
        self.providers
            .iter()
            .find(|p| p.provider == name)
            .ok_or_else(|| TuraError::Config {
                message: format!("This route has no provider named '{}'", name),
            })
    }

    pub async fn embed(&self, text: &str, conf: &TuraConfig) -> Result<Vec<f32>, TuraError> {
        self.validate()?;
        self.providers[0].embed(text, conf).await
    }

    pub async fn run(
        &self,
        conf: &TuraConfig,
        messages: Vec<Value>,
        options: CallOptions,
    ) -> Result<ProviderResponse, TuraError> {
        self.run_with_stream_events(conf, messages, options, None)
            .await
    }

    pub async fn run_with_stream_events(
        &self,
        conf: &TuraConfig,
        messages: Vec<Value>,
        options: CallOptions,
        stream_events: Option<ProviderStreamEventSink>,
    ) -> Result<ProviderResponse, TuraError> {
        self.validate()?;

        let mut failures = Vec::new();
        for provider in &self.providers {
            let mut effective = options.clone();
            if effective.temperature.is_none() {
                effective.temperature = Some(provider.temperature);
            }
            match provider
                .call_with_stream_events(conf, messages.clone(), effective, stream_events.clone())
                .await
            {
                Ok(result) => return Ok(result),
                Err(err) => {
                    warn!(provider = %provider.provider, model = %provider.model, error = %err, "route fallback to next provider");
                    failures.push(format!(
                        "{}:{} => {}",
                        provider.provider, provider.model, err
                    ));
                }
            }
        }

        Err(TuraError::AllProvidersFailed {
            message: failures.join(" | "),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub provider_base_url: HashMap<String, String>,
    pub routes: HashMap<String, RouteConfig>,
    #[serde(default)]
    pub model_catalog: ModelCatalog,
    #[serde(default)]
    pub provider_enums: ProviderEnumCatalog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootConfig {
    pub provider_base_url: HashMap<String, String>,
    pub routes: HashMap<String, RawRouteConfig>,
    #[serde(default)]
    pub model_catalog: ModelCatalog,
    #[serde(default)]
    pub provider_enums: ProviderEnumCatalog,
    #[serde(default)]
    pub provider_auth: HashMap<String, ProviderAuthConfig>,
    #[serde(default)]
    pub provider_latency: ProviderLatencyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelCatalog {
    #[serde(default)]
    pub tiers: Vec<String>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderCatalogConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderEnumCatalog {
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub api_styles: Vec<String>,
    #[serde(default)]
    pub auth_methods: Vec<String>,
    #[serde(default)]
    pub statuses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderCatalogConfig {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub runtime_provider: String,
    #[serde(default)]
    pub api_style: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub token_env: Option<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub auth_methods: Vec<String>,
    #[serde(default)]
    pub api_docs: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub models: HashMap<String, Vec<CatalogModelConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CatalogModelConfig {
    Id(String),
    Detailed(CatalogModelDetail),
}

impl CatalogModelConfig {
    pub fn id(&self) -> &str {
        match self {
            Self::Id(id) => id,
            Self::Detailed(detail) => &detail.id,
        }
    }

    pub fn detail(&self) -> Option<&CatalogModelDetail> {
        match self {
            Self::Id(_) => None,
            Self::Detailed(detail) => Some(detail),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CatalogModelDetail {
    pub id: String,
    #[serde(default = "default_model_visible")]
    pub visible: bool,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub release_date: String,
    #[serde(default)]
    pub attachment: bool,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub temperature: bool,
    #[serde(default)]
    pub tool_call: bool,
    #[serde(default)]
    pub limit: CatalogModelLimit,
    #[serde(default)]
    pub modalities: CatalogModelModalities,
    #[serde(default)]
    pub options: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub status: Option<String>,
}

fn default_model_visible() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CatalogModelLimit {
    pub context: u32,
    pub input: u32,
    pub output: u32,
}

impl Default for CatalogModelLimit {
    fn default() -> Self {
        Self {
            context: 200_000,
            input: 200_000,
            output: 16_384,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogModelModalities {
    pub input: Vec<String>,
    pub output: Vec<String>,
}

impl Default for CatalogModelModalities {
    fn default() -> Self {
        Self {
            input: vec!["text".to_string()],
            output: vec!["text".to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderLatencyConfig {
    #[serde(default = "default_latency_level")]
    pub active: String,
    #[serde(default = "default_latency_levels")]
    pub levels: HashMap<String, ProviderLatencyTimeouts>,
}

impl Default for ProviderLatencyConfig {
    fn default() -> Self {
        Self {
            active: default_latency_level(),
            levels: default_latency_levels(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProviderLatencyTimeouts {
    pub idle_output_timeout_ms: u64,
    pub first_output_timeout_ms: u64,
    pub total_timeout_ms: u64,
}

impl Default for ProviderLatencyTimeouts {
    fn default() -> Self {
        Self {
            idle_output_timeout_ms: 20_000,
            first_output_timeout_ms: 40_000,
            total_timeout_ms: 240_000,
        }
    }
}

fn default_latency_level() -> String {
    "fast".to_string()
}

fn default_latency_levels() -> HashMap<String, ProviderLatencyTimeouts> {
    HashMap::from([
        (
            "fast".to_string(),
            ProviderLatencyTimeouts {
                idle_output_timeout_ms: 20_000,
                first_output_timeout_ms: 40_000,
                total_timeout_ms: 240_000,
            },
        ),
        (
            "high".to_string(),
            ProviderLatencyTimeouts {
                idle_output_timeout_ms: 80_000,
                first_output_timeout_ms: 160_000,
                total_timeout_ms: 960_000,
            },
        ),
        (
            "x-high".to_string(),
            ProviderLatencyTimeouts {
                idle_output_timeout_ms: 100_000,
                first_output_timeout_ms: 180_000,
                total_timeout_ms: 1_200_000,
            },
        ),
    ])
}

/// Map a model-catalog tier (the "tier flag", i.e. the route / `tura_llm_name`)
/// to a provider-latency level. Level selection is driven entirely by the tier,
/// never by the thinking / reasoning_effort parameter.
///
/// - flagship_thinking -> x-high
/// - thinking          -> high
/// - fast / instant    -> fast (lowest)
/// - embedding_high    -> high
/// - embedding_low     -> fast
pub fn latency_level_for_tier(tier: &str) -> &'static str {
    match tier.trim().to_ascii_lowercase().as_str() {
        "flagship_thinking" => "x-high",
        "thinking" => "high",
        "embedding_high" => "high",
        "fast" | "instant" | "embedding_low" => "fast",
        _ => "fast",
    }
}

impl ProviderLatencyConfig {
    pub fn selected_timeouts(&self) -> ProviderLatencyTimeouts {
        self.timeouts_for_level(&self.active)
    }

    /// Resolve the timeouts for a specific latency level, falling back to the
    /// `fast` level and finally to defaults.
    pub fn timeouts_for_level(&self, level: &str) -> ProviderLatencyTimeouts {
        self.levels
            .get(level.trim())
            .copied()
            .or_else(|| self.levels.get("fast").copied())
            .unwrap_or_default()
    }

    /// Resolve timeouts for a model-catalog tier (the tier flag).
    pub fn timeouts_for_tier(&self, tier: &str) -> ProviderLatencyTimeouts {
        self.timeouts_for_level(latency_level_for_tier(tier))
    }
}

pub fn set_provider_latency_timeouts(timeouts: ProviderLatencyTimeouts) {
    let lock =
        PROVIDER_LATENCY_TIMEOUTS.get_or_init(|| RwLock::new(ProviderLatencyTimeouts::default()));
    if let Ok(mut guard) = lock.write() {
        *guard = timeouts;
    }
}

pub fn provider_latency_timeouts() -> ProviderLatencyTimeouts {
    let lock =
        PROVIDER_LATENCY_TIMEOUTS.get_or_init(|| RwLock::new(ProviderLatencyTimeouts::default()));
    lock.read().map(|guard| *guard).unwrap_or_default()
}

/// Store the active provider-latency config (level table + active level) so it
/// can later be resolved per-tier at runtime-construction time.
pub fn set_provider_latency_config(config: ProviderLatencyConfig) {
    let lock =
        PROVIDER_LATENCY_CONFIG.get_or_init(|| RwLock::new(ProviderLatencyConfig::default()));
    if let Ok(mut guard) = lock.write() {
        *guard = config;
    }
}

pub fn provider_latency_config() -> ProviderLatencyConfig {
    let lock =
        PROVIDER_LATENCY_CONFIG.get_or_init(|| RwLock::new(ProviderLatencyConfig::default()));
    lock.read().map(|guard| guard.clone()).unwrap_or_default()
}

/// Resolve and install the global latency timeouts for a model-catalog tier
/// (the tier flag). Level selection is driven entirely by the tier, never by
/// the thinking / reasoning_effort parameter. Returns the applied timeouts.
pub fn apply_latency_for_tier(tier: &str) -> ProviderLatencyTimeouts {
    let timeouts = provider_latency_config().timeouts_for_tier(tier);
    set_provider_latency_timeouts(timeouts);
    timeouts
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderAuthConfig {
    #[serde(rename = "type")]
    pub auth_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RawRouteConfig {
    #[serde(default = "default_temperature")]
    pub default_temperature: f64,
    #[serde(default)]
    pub providers: Vec<RawProviderConfig>,
}

fn default_temperature() -> f64 {
    0.2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawProviderConfig {
    pub provider: String,
    pub model: String,
    pub temperature: Option<f64>,
}

impl Settings {
    pub async fn default() -> Result<Arc<Self>, TuraError> {
        let explicit_config = std::env::var_os("TURA_PROVIDER_CONFIG").is_some()
            || std::env::var_os("TURALLM_CONFIG").is_some();
        if !explicit_config {
            if let Some(settings) = SETTINGS.get() {
                return Ok(Arc::clone(settings));
            }
        }
        let loaded = Arc::new(crate::tura_llm_conf::load_settings().await?);
        if !explicit_config {
            let _ = SETTINGS.set(Arc::clone(&loaded));
        }
        Ok(loaded)
    }

    pub fn normalize_model_name(provider: &str, model: &str) -> String {
        let model = model.trim();
        let prefix = format!("{provider}/");
        if model.starts_with(&prefix) {
            return model[prefix.len()..].to_string();
        }
        if provider == "openai" && model.starts_with("openai/") {
            return model["openai/".len()..].to_string();
        }
        if provider == "codex" && model.starts_with("codex/") {
            return model["codex/".len()..].to_string();
        }
        model.to_string()
    }

    pub fn route_by_name(&self, name: &str) -> Option<&RouteConfig> {
        self.routes.get(name)
    }

    pub fn routes(&self) -> impl Iterator<Item = &RouteConfig> {
        self.routes.values()
    }

    pub fn provider_base_url(&self, provider: &str) -> Option<String> {
        self.provider_base_url.get(provider).cloned().or_else(|| {
            self.routes()
                .flat_map(|route| route.providers.iter())
                .find(|item| item.provider == provider)
                .map(|item| item.base_url.clone())
        })
    }

    pub fn configured_model_catalog(&self) -> HashMap<String, Vec<String>> {
        let mut catalog = HashMap::<String, Vec<String>>::new();
        for (provider, config) in &self.model_catalog.providers {
            let models = catalog.entry(provider.clone()).or_default();
            for model in config.models.values().flatten() {
                let normalized = Self::normalize_model_name(provider, model.id());
                if !models.iter().any(|existing| existing == &normalized) {
                    models.push(normalized);
                }
            }
        }
        for route in self.routes() {
            for provider in &route.providers {
                let model = Self::normalize_model_name(&provider.provider, &provider.model);
                let models = catalog.entry(provider.provider.clone()).or_default();
                if !models.iter().any(|existing| existing == &model) {
                    models.push(model);
                }
            }
        }
        for models in catalog.values_mut() {
            models.sort();
        }
        catalog
    }

    pub fn make_provider(
        provider_base_url: &HashMap<String, String>,
        provider: &str,
        model: &str,
        temperature: Option<f64>,
        route_default_temp: f64,
    ) -> Result<ProviderConfig, TuraError> {
        let base_url =
            provider_base_url
                .get(provider)
                .cloned()
                .ok_or_else(|| TuraError::UnknownProvider {
                    provider: provider.to_string(),
                })?;

        let config = ProviderConfig {
            provider: provider.to_string(),
            base_url,
            model: Self::normalize_model_name(provider, model),
            temperature: temperature.unwrap_or(route_default_temp),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn make_route(
        provider_base_url: &HashMap<String, String>,
        items: &[RawProviderConfig],
        default_temperature: f64,
    ) -> Result<RouteConfig, TuraError> {
        let mut providers = Vec::with_capacity(items.len());
        for item in items {
            providers.push(Self::make_provider(
                provider_base_url,
                &item.provider,
                &item.model,
                item.temperature,
                default_temperature,
            )?);
        }
        let route = RouteConfig {
            default_temperature,
            providers,
        };
        route.validate()?;
        Ok(route)
    }
}
