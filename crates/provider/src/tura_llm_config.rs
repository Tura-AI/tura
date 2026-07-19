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
                message: format!("This route has no provider named '{name}'"),
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
                    if err.is_non_retryable_provider_failure() {
                        warn!(provider = %provider.provider, model = %provider.model, error = %err, "provider failure is not retryable; returning without route fallback");
                        return Err(err);
                    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderLatencyTimeouts {
    pub idle_output_timeout_ms: u64,
    pub first_output_timeout_ms: u64,
    pub total_timeout_ms: u64,
}

impl Default for ProviderLatencyTimeouts {
    fn default() -> Self {
        Self {
            idle_output_timeout_ms: 80_000,
            first_output_timeout_ms: 160_000,
            total_timeout_ms: 960_000,
        }
    }
}

fn default_latency_level() -> String {
    "high".to_string()
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
/// - thinking       -> x-high
/// - fast           -> high
/// - embedding_high -> x-high
/// - embedding_low  -> high
/// - unknown        -> high
pub fn latency_level_for_tier(tier: &str) -> &'static str {
    match tier.trim().to_ascii_lowercase().as_str() {
        "thinking" | "embedding_high" => "x-high",
        "fast" | "embedding_low" => "high",
        _ => "high",
    }
}

impl ProviderLatencyConfig {
    pub fn selected_timeouts(&self) -> ProviderLatencyTimeouts {
        self.timeouts_for_level(&self.active)
    }

    /// Resolve the timeouts for a specific latency level, falling back to the
    /// `high` level and finally to defaults.
    pub fn timeouts_for_level(&self, level: &str) -> ProviderLatencyTimeouts {
        self.levels
            .get(level.trim())
            .copied()
            .or_else(|| self.levels.get("high").copied())
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
        let explicit_config = std::env::var_os("TURA_PROVIDER_CONFIG").is_some();
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
        let runtime_provider = crate::auth_registry::runtime_provider_id(provider);
        self.provider_base_url
            .get(provider)
            .or_else(|| self.provider_base_url.get(runtime_provider))
            .cloned()
            .or_else(|| {
                self.routes()
                    .flat_map(|route| route.providers.iter())
                    .find(|item| item.provider == provider || item.provider == runtime_provider)
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

    pub fn tier_for_model(&self, provider: &str, model: &str) -> Option<String> {
        let catalog = self.model_catalog.providers.get(provider)?;
        let normalized_model = Self::normalize_model_name(provider, model);
        let mut tier_names = self.model_catalog.tiers.clone();
        for tier in catalog.models.keys() {
            if !tier_names.iter().any(|existing| existing == tier) {
                tier_names.push(tier.clone());
            }
        }
        for tier in tier_names {
            let Some(models) = catalog.models.get(&tier) else {
                continue;
            };
            if models.iter().any(|entry| {
                let id = entry.id();
                id == model || Self::normalize_model_name(provider, id) == normalized_model
            }) {
                return Some(tier);
            }
        }
        None
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

#[cfg(test)]
mod tests {
    use super::{
        apply_latency_for_tier, latency_level_for_tier, provider_latency_config,
        provider_latency_timeouts, set_provider_latency_config, set_provider_latency_timeouts,
        CatalogModelConfig, CatalogModelDetail, CatalogModelLimit, CatalogModelModalities,
        ModelCatalog, ProviderCatalogConfig, ProviderEnumCatalog, ProviderLatencyConfig,
        ProviderLatencyTimeouts, RawProviderConfig, RawRouteConfig, RootConfig, RouteConfig,
        Settings,
    };
    use crate::{ProviderConfig, TuraError};
    use serde_json::json;
    use std::collections::HashMap;

    fn base_urls() -> HashMap<String, String> {
        HashMap::from([
            (
                "openai".to_string(),
                "https://api.openai.test/v1".to_string(),
            ),
            ("codex".to_string(), "https://codex.test/v1".to_string()),
            ("google".to_string(), "https://google.test/v1".to_string()),
        ])
    }

    fn provider(provider: &str, model: &str) -> ProviderConfig {
        ProviderConfig {
            provider: provider.to_string(),
            base_url: format!("https://{provider}.test/v1"),
            model: Settings::normalize_model_name(provider, model),
            temperature: 0.2,
        }
    }

    fn settings_with_catalog_and_routes() -> Settings {
        let catalog_provider = ProviderCatalogConfig {
            display_name: "OpenAI".to_string(),
            models: HashMap::from([(
                "flagship".to_string(),
                vec![
                    CatalogModelConfig::Id("openai/gpt-5.5".to_string()),
                    CatalogModelConfig::Detailed(CatalogModelDetail {
                        id: "gpt-5.4-mini".to_string(),
                        visible: true,
                        name: "GPT Mini".to_string(),
                        ..Default::default()
                    }),
                    CatalogModelConfig::Id("gpt-5.5".to_string()),
                ],
            )]),
            ..Default::default()
        };
        Settings {
            provider_base_url: base_urls(),
            routes: HashMap::from([
                (
                    "fast".to_string(),
                    RouteConfig {
                        default_temperature: 0.2,
                        providers: vec![
                            provider("openai", "openai/gpt-5.4-mini"),
                            provider("google", "google/gemini-3.5-flash"),
                        ],
                    },
                ),
                (
                    "thinking".to_string(),
                    RouteConfig {
                        default_temperature: 0.6,
                        providers: vec![provider("openai", "gpt-5.5")],
                    },
                ),
            ]),
            model_catalog: ModelCatalog {
                tiers: vec!["fast".to_string(), "thinking".to_string()],
                providers: HashMap::from([("openai".to_string(), catalog_provider)]),
            },
            provider_enums: ProviderEnumCatalog::default(),
        }
    }

    #[test]
    fn call_options_default_is_empty_and_serializes_without_spurious_values() {
        let value = serde_json::to_value(super::CallOptions::default()).expect("serialize");

        assert_eq!(value["search"], false);
        assert_eq!(value["force_search"], false);
        assert_eq!(value["temperature"], serde_json::Value::Null);
        assert_eq!(value["tools"], serde_json::Value::Null);
        assert_eq!(value["parallel_tool_calls"], serde_json::Value::Null);
    }

    #[test]
    fn route_validate_rejects_out_of_range_default_temperature() {
        let route = RouteConfig {
            default_temperature: 2.1,
            providers: vec![provider("openai", "gpt-5.5")],
        };

        let error = route
            .validate()
            .expect_err("temperature above 2.0 is invalid");

        assert!(matches!(error, TuraError::Validation { .. }));
        assert!(error
            .to_string()
            .contains("default_temperature must be within [0.0, 2.0]"));
    }

    #[test]
    fn route_validate_rejects_invalid_provider_config() {
        let mut invalid = provider("openai", "gpt-5.5");
        invalid.base_url = String::new();
        let route = RouteConfig {
            default_temperature: 0.2,
            providers: vec![invalid],
        };

        let error = route
            .validate()
            .expect_err("provider validation should fail");

        assert!(error.to_string().contains("base_url"));
    }

    #[test]
    fn route_provider_lookup_returns_named_provider_or_contextual_error() {
        let route = RouteConfig {
            default_temperature: 0.2,
            providers: vec![provider("openai", "gpt-5.5"), provider("google", "gemini")],
        };

        assert_eq!(
            route.provider("google").expect("google provider").provider,
            "google"
        );
        let error = route
            .provider("anthropic")
            .expect_err("missing provider should report route context");
        assert!(matches!(error, TuraError::Config { .. }));
        assert!(error
            .to_string()
            .contains("This route has no provider named 'anthropic'"));
    }

    #[test]
    fn catalog_model_config_id_and_detail_accessors_are_variant_specific() {
        let id = CatalogModelConfig::Id("gpt-5.5".to_string());
        assert_eq!(id.id(), "gpt-5.5");
        assert!(id.detail().is_none());

        let detail = CatalogModelConfig::Detailed(CatalogModelDetail {
            id: "gpt-5.4-mini".to_string(),
            name: "GPT Mini".to_string(),
            ..Default::default()
        });
        assert_eq!(detail.id(), "gpt-5.4-mini");
        assert_eq!(
            detail.detail().map(|item| item.name.as_str()),
            Some("GPT Mini")
        );
    }

    #[test]
    fn catalog_model_defaults_preserve_visible_text_only_contract() {
        let detail: CatalogModelDetail = serde_json::from_value(json!({
            "id": "model-a"
        }))
        .expect("missing visible should use serde default");
        let limit = CatalogModelLimit::default();
        let modalities = CatalogModelModalities::default();

        assert!(detail.visible);
        assert!(!CatalogModelDetail::default().visible);
        assert_eq!(limit.context, 200_000);
        assert_eq!(limit.input, 200_000);
        assert_eq!(limit.output, 16_384);
        assert_eq!(modalities.input, vec!["text"]);
        assert_eq!(modalities.output, vec!["text"]);
    }

    #[test]
    fn provider_latency_default_levels_cover_fast_high_and_x_high() {
        let config = ProviderLatencyConfig::default();

        assert_eq!(config.active, "high");
        assert_eq!(config.levels["fast"].idle_output_timeout_ms, 20_000);
        assert_eq!(config.levels["high"].first_output_timeout_ms, 160_000);
        assert_eq!(config.levels["x-high"].total_timeout_ms, 1_200_000);
        assert_eq!(config.selected_timeouts(), config.levels["high"]);
    }

    #[test]
    fn latency_level_for_tier_is_canonical_and_case_insensitive() {
        assert_eq!(latency_level_for_tier("THINKING"), "x-high");
        assert_eq!(latency_level_for_tier("embedding_high"), "x-high");
        assert_eq!(latency_level_for_tier("fast"), "high");
        assert_eq!(latency_level_for_tier("embedding_low"), "high");
        assert_eq!(latency_level_for_tier("unknown"), "high");
    }

    #[test]
    fn latency_config_falls_back_to_high_then_builtin_defaults() {
        let high = ProviderLatencyTimeouts {
            idle_output_timeout_ms: 1,
            first_output_timeout_ms: 2,
            total_timeout_ms: 3,
        };
        let config = ProviderLatencyConfig {
            active: "missing".to_string(),
            levels: HashMap::from([("high".to_string(), high)]),
        };

        assert_eq!(config.selected_timeouts(), high);
        assert_eq!(config.timeouts_for_level("missing"), high);
        assert_eq!(config.timeouts_for_tier("unknown"), high);

        let empty = ProviderLatencyConfig {
            active: "missing".to_string(),
            levels: HashMap::new(),
        };
        assert_eq!(
            empty.timeouts_for_level("missing"),
            ProviderLatencyTimeouts::default()
        );
    }

    #[test]
    fn latency_global_setters_are_idempotent_and_apply_tiers() {
        let fast = ProviderLatencyTimeouts {
            idle_output_timeout_ms: 10,
            first_output_timeout_ms: 20,
            total_timeout_ms: 30,
        };
        let high = ProviderLatencyTimeouts {
            idle_output_timeout_ms: 40,
            first_output_timeout_ms: 50,
            total_timeout_ms: 60,
        };
        let x_high = ProviderLatencyTimeouts {
            idle_output_timeout_ms: 70,
            first_output_timeout_ms: 80,
            total_timeout_ms: 90,
        };
        let config = ProviderLatencyConfig {
            active: "high".to_string(),
            levels: HashMap::from([
                ("fast".to_string(), fast),
                ("high".to_string(), high),
                ("x-high".to_string(), x_high),
            ]),
        };

        set_provider_latency_config(config.clone());
        assert_eq!(provider_latency_config().selected_timeouts(), high);
        set_provider_latency_timeouts(high);
        assert_eq!(provider_latency_timeouts(), high);
        assert_eq!(apply_latency_for_tier("thinking"), x_high);
        assert_eq!(provider_latency_timeouts(), x_high);
        set_provider_latency_config(config);
        assert_eq!(apply_latency_for_tier("fast"), high);
    }

    #[test]
    fn provider_auth_config_serializes_only_configured_optional_fields() {
        let auth = super::ProviderAuthConfig {
            auth_type: "oauth".to_string(),
            status: Some("connected".to_string()),
            token_env: Some("OPENAI_API_KEY".to_string()),
            ..Default::default()
        };

        let value = serde_json::to_value(&auth).expect("serialize auth");

        assert_eq!(value["type"], "oauth");
        assert_eq!(value["status"], "connected");
        assert_eq!(value["token_env"], "OPENAI_API_KEY");
        assert!(value.get("refresh_env").is_none());
        assert!(value.get("account_id").is_none());
    }

    #[test]
    fn raw_route_config_defaults_temperature_and_empty_provider_list() {
        let route: RawRouteConfig = serde_json::from_value(json!({})).expect("default route");

        assert_eq!(route.default_temperature, 0.2);
        assert!(route.providers.is_empty());
    }

    #[test]
    fn normalize_model_name_strips_only_matching_internal_prefixes() {
        assert_eq!(
            Settings::normalize_model_name("openai", " openai/gpt-5.5 "),
            "gpt-5.5"
        );
        assert_eq!(
            Settings::normalize_model_name("codex", "codex/gpt-5.5"),
            "gpt-5.5"
        );
        assert_eq!(
            Settings::normalize_model_name("google", "openai/gpt-5.5"),
            "openai/gpt-5.5"
        );
        assert_eq!(
            Settings::normalize_model_name("google", "google/gemini-3.5-flash"),
            "gemini-3.5-flash"
        );
    }

    #[test]
    fn settings_route_iteration_and_lookup_are_consistent() {
        let settings = settings_with_catalog_and_routes();
        let route_names = settings.routes.keys().cloned().collect::<Vec<_>>();
        let route_count = settings.routes().count();

        assert_eq!(route_count, route_names.len());
        assert!(settings.route_by_name("fast").is_some());
        assert!(settings.route_by_name("missing").is_none());
    }

    #[test]
    fn provider_base_url_prefers_config_then_canonical_alias_then_route() {
        let mut settings = settings_with_catalog_and_routes();
        assert_eq!(
            settings.provider_base_url("openai").as_deref(),
            Some("https://api.openai.test/v1")
        );
        assert_eq!(
            settings.provider_base_url("gemini-api").as_deref(),
            Some("https://google.test/v1")
        );

        settings.provider_base_url.remove("google");
        assert_eq!(
            settings.provider_base_url("google").as_deref(),
            Some("https://google.test/v1")
        );
        assert_eq!(settings.provider_base_url("missing"), None);

        let minimal_settings = Settings {
            provider_base_url: HashMap::from([(
                "openai".to_string(),
                "https://openai.test/v1".to_string(),
            )]),
            routes: HashMap::new(),
            model_catalog: ModelCatalog::default(),
            provider_enums: ProviderEnumCatalog::default(),
        };
        assert_eq!(minimal_settings.provider_base_url("codex"), None);
    }

    #[test]
    fn configured_model_catalog_merges_catalog_and_routes_deduped_and_sorted() {
        let settings = settings_with_catalog_and_routes();

        let catalog = settings.configured_model_catalog();

        assert_eq!(
            catalog.get("openai"),
            Some(&vec!["gpt-5.4-mini".to_string(), "gpt-5.5".to_string()])
        );
        assert_eq!(
            catalog.get("google"),
            Some(&vec!["gemini-3.5-flash".to_string()])
        );
    }

    #[test]
    fn tier_for_model_resolves_catalog_tier_from_actual_provider_model() {
        let settings = Settings {
            provider_base_url: base_urls(),
            routes: HashMap::new(),
            model_catalog: ModelCatalog {
                tiers: vec!["thinking".to_string(), "fast".to_string()],
                providers: HashMap::from([(
                    "codex".to_string(),
                    ProviderCatalogConfig {
                        models: HashMap::from([
                            (
                                "thinking".to_string(),
                                vec![CatalogModelConfig::Id("gpt-5.5".to_string())],
                            ),
                            (
                                "fast".to_string(),
                                vec![CatalogModelConfig::Id("gpt-5.3-codex-spark".to_string())],
                            ),
                        ]),
                        ..Default::default()
                    },
                )]),
            },
            provider_enums: ProviderEnumCatalog::default(),
        };

        assert_eq!(
            settings.tier_for_model("codex", "codex/gpt-5.5").as_deref(),
            Some("thinking")
        );
        assert_eq!(
            settings
                .tier_for_model("codex", "gpt-5.3-codex-spark")
                .as_deref(),
            Some("fast")
        );
        assert_eq!(settings.tier_for_model("codex", "unknown"), None);
    }

    #[test]
    fn make_provider_applies_route_default_temperature_and_model_normalization() {
        let provider = Settings::make_provider(&base_urls(), "openai", "openai/gpt-5.5", None, 0.7)
            .expect("provider");

        assert_eq!(provider.provider, "openai");
        assert_eq!(provider.base_url, "https://api.openai.test/v1");
        assert_eq!(provider.model, "gpt-5.5");
        assert_eq!(provider.temperature, 0.7);
    }

    #[test]
    fn make_provider_overrides_temperature_when_provider_sets_one() {
        let provider = Settings::make_provider(&base_urls(), "openai", "gpt-5.5", Some(0.1), 0.7)
            .expect("provider");

        assert_eq!(provider.temperature, 0.1);
    }

    #[test]
    fn make_provider_reports_unknown_provider_without_guessing_base_url() {
        let error = Settings::make_provider(&base_urls(), "missing", "model", None, 0.2)
            .expect_err("unknown provider should fail");

        assert!(matches!(error, TuraError::UnknownProvider { .. }));
        assert!(error.to_string().contains("missing"));
    }

    #[test]
    fn make_route_preserves_provider_order_and_default_temperature() {
        let route = Settings::make_route(
            &base_urls(),
            &[
                RawProviderConfig {
                    provider: "openai".to_string(),
                    model: "openai/gpt-5.5".to_string(),
                    temperature: None,
                },
                RawProviderConfig {
                    provider: "google".to_string(),
                    model: "google/gemini-3.5-flash".to_string(),
                    temperature: Some(0.0),
                },
            ],
            0.4,
        )
        .expect("route");

        assert_eq!(route.default_temperature, 0.4);
        assert_eq!(route.providers[0].provider, "openai");
        assert_eq!(route.providers[0].model, "gpt-5.5");
        assert_eq!(route.providers[0].temperature, 0.4);
        assert_eq!(route.providers[1].provider, "google");
        assert_eq!(route.providers[1].model, "gemini-3.5-flash");
        assert_eq!(route.providers[1].temperature, 0.0);
    }

    #[test]
    fn make_route_rejects_invalid_default_temperature_after_provider_build() {
        let error = Settings::make_route(
            &base_urls(),
            &[RawProviderConfig {
                provider: "openai".to_string(),
                model: "gpt-5.5".to_string(),
                temperature: None,
            }],
            -0.1,
        )
        .expect_err("route validation should reject invalid default temperature");

        assert!(matches!(error, TuraError::Validation { .. }));
    }

    #[test]
    fn root_config_deserializes_optional_catalog_auth_and_latency_sections() {
        let config: RootConfig = serde_json::from_value(json!({
            "provider_base_url": { "openai": "https://api.openai.test/v1" },
            "routes": {
                "fast": {
                    "providers": [
                        { "provider": "openai", "model": "openai/gpt-5.5" }
                    ]
                }
            }
        }))
        .expect("root config");

        assert_eq!(
            config.provider_base_url.get("openai").map(String::as_str),
            Some("https://api.openai.test/v1")
        );
        assert_eq!(config.routes["fast"].default_temperature, 0.2);
        assert!(config.model_catalog.providers.is_empty());
        assert!(config.provider_auth.is_empty());
        assert_eq!(config.provider_latency.active, "high");
    }

    #[test]
    fn catalog_provider_config_deserializes_openapi_service_metadata() {
        let provider: ProviderCatalogConfig = serde_json::from_value(json!({
            "display_name": "Feishu",
            "runtime_provider": "openapi",
            "api_style": "openapi",
            "base_url": "https://open.feishu.cn/open-apis",
            "token_env": "FEISHU_TOKEN",
            "env": ["FEISHU_APP_ID"],
            "domains": ["productivity"],
            "capabilities": ["message.send"],
            "auth_methods": ["app_token"],
            "api_docs": "https://open.feishu.cn/document",
            "status": "stable"
        }))
        .expect("provider catalog");

        assert_eq!(provider.display_name, "Feishu");
        assert_eq!(provider.runtime_provider, "openapi");
        assert_eq!(provider.api_style, "openapi");
        assert_eq!(provider.base_url, "https://open.feishu.cn/open-apis");
        assert_eq!(provider.token_env.as_deref(), Some("FEISHU_TOKEN"));
        assert_eq!(provider.env, vec!["FEISHU_APP_ID"]);
        assert_eq!(provider.domains, vec!["productivity"]);
        assert_eq!(provider.capabilities, vec!["message.send"]);
        assert_eq!(provider.auth_methods, vec!["app_token"]);
        assert_eq!(
            provider.api_docs.as_deref(),
            Some("https://open.feishu.cn/document")
        );
        assert_eq!(provider.status.as_deref(), Some("stable"));
    }
}
