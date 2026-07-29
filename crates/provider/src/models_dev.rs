use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::fs;
use url::Url;

use crate::tura_llm::{
    CatalogModelConfig, CatalogModelDetail, CatalogModelLimit, CatalogModelModalities,
    ProviderCatalogConfig, RootConfig, TuraError,
};

pub const MODELS_DEV_CATALOG_ENV: &str = "TURA_MODELS_DEV_CATALOG";
pub const MODELS_DEV_SOURCE_URL: &str = "https://models.dev/api.json";
const OPENAI_COMPATIBLE_NPM: &str = "@ai-sdk/openai-compatible";
const IMPORTED_MODEL_BUCKET: &str = "models_dev";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModelsDevProvenance {
    pub url: String,
    pub revision: String,
    pub sha256: String,
    pub local_path: String,
}

#[derive(Debug, Clone)]
pub struct ModelsDevProjection {
    pub provider_base_url: HashMap<String, String>,
    pub providers: HashMap<String, ProviderCatalogConfig>,
    pub provenance: ModelsDevProvenance,
}

#[derive(Debug, Error)]
pub enum ModelsDevError {
    #[error("models.dev JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("models.dev record `{record}` is invalid: {message}")]
    Invalid { record: String, message: String },
}

#[derive(Debug, Deserialize)]
struct RawProvider {
    id: String,
    env: Vec<String>,
    npm: String,
    api: Option<String>,
    name: String,
    doc: String,
    models: StrictMap<RawModel>,
}

#[derive(Debug, Deserialize)]
struct RawModel {
    id: String,
    name: String,
    description: String,
    family: Option<String>,
    attachment: bool,
    reasoning: bool,
    tool_call: bool,
    structured_output: Option<bool>,
    temperature: Option<bool>,
    knowledge: Option<String>,
    release_date: String,
    last_updated: String,
    modalities: RawModalities,
    open_weights: bool,
    limit: RawLimit,
    status: Option<String>,
    provider: Option<RawModelProvider>,
}

#[derive(Debug, Deserialize)]
struct RawModalities {
    input: Vec<String>,
    output: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawLimit {
    context: u32,
    input: Option<u32>,
    output: u32,
}

#[derive(Debug, Deserialize)]
struct RawModelProvider {
    npm: Option<String>,
    api: Option<String>,
    shape: Option<String>,
    body: Option<HashMap<String, Value>>,
    headers: Option<HashMap<String, String>>,
}

#[derive(Debug)]
struct StrictMap<T>(HashMap<String, T>);

impl<'de, T> Deserialize<'de> for StrictMap<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StrictMapVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for StrictMapVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = StrictMap<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object without duplicate keys")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = HashMap::with_capacity(access.size_hint().unwrap_or_default());
                while let Some((key, value)) = access.next_entry::<String, T>()? {
                    if values.insert(key.clone(), value).is_some() {
                        return Err(serde::de::Error::custom(format!(
                            "duplicate object key `{key}`"
                        )));
                    }
                }
                Ok(StrictMap(values))
            }
        }

        deserializer.deserialize_map(StrictMapVisitor(PhantomData))
    }
}

#[derive(Debug, Default)]
struct ImportedRuntimeMetadata {
    token_envs: HashMap<String, Vec<String>>,
    model_ids: HashSet<(String, String)>,
}

fn runtime_metadata() -> &'static RwLock<ImportedRuntimeMetadata> {
    static METADATA: OnceLock<RwLock<ImportedRuntimeMetadata>> = OnceLock::new();
    METADATA.get_or_init(|| RwLock::new(ImportedRuntimeMetadata::default()))
}

pub fn configured_token_env(provider_id: &str) -> Option<String> {
    configured_token_envs(provider_id).into_iter().next()
}

pub fn configured_token_envs(provider_id: &str) -> Vec<String> {
    let guard = runtime_metadata()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard
        .token_envs
        .get(&provider_id.to_ascii_lowercase())
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn is_imported_model_id(provider_id: &str, model_id: &str) -> bool {
    let guard = runtime_metadata()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.model_ids.contains(&(
        provider_id.to_ascii_lowercase(),
        model_id.trim().to_string(),
    ))
}

pub async fn load_configured_projection() -> Result<Option<ModelsDevProjection>, TuraError> {
    let Some(path) = env::var_os(MODELS_DEV_CATALOG_ENV) else {
        return Ok(None);
    };
    if path.is_empty() {
        return Err(TuraError::Config {
            message: format!("{MODELS_DEV_CATALOG_ENV} must name a local models.dev snapshot"),
        });
    }
    let path = PathBuf::from(path);
    let bytes = fs::read(&path).await.map_err(TuraError::io)?;
    project_catalog(&bytes, path.display().to_string())
        .map(Some)
        .map_err(|error| TuraError::Config {
            message: error.to_string(),
        })
}

pub fn project_catalog(
    bytes: &[u8],
    local_path: impl Into<String>,
) -> Result<ModelsDevProjection, ModelsDevError> {
    let catalog: StrictMap<RawProvider> = serde_json::from_slice(bytes)?;
    let digest = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let provenance = ModelsDevProvenance {
        url: MODELS_DEV_SOURCE_URL.to_string(),
        revision: format!("sha256:{digest}"),
        sha256: digest,
        local_path: local_path.into(),
    };
    let mut provider_base_url = HashMap::new();
    let mut providers = HashMap::new();
    let mut provider_ids = HashSet::new();

    for (provider_id, provider) in catalog.0 {
        validate_provider_identity(&provider_id, &provider)?;
        let canonical_id = provider_id.to_ascii_lowercase();
        if !provider_ids.insert(canonical_id) {
            return Err(invalid(&provider_id, "provider ID collision"));
        }
        if provider.npm != OPENAI_COMPATIBLE_NPM {
            continue;
        }

        let Some(api) = validate_api(&provider_id, provider.api.as_deref())? else {
            continue;
        };
        let env_names = validate_env_names(&provider_id, provider.env)?;
        let models = project_models(&provider_id, &api, provider.models, &provenance)?;
        if models.is_empty() {
            continue;
        }

        let capabilities = provider_capabilities(&models);
        let token_env = env_names.first().cloned();
        provider_base_url.insert(provider_id.clone(), api.clone());
        providers.insert(
            provider_id,
            ProviderCatalogConfig {
                display_name: provider.name,
                runtime_provider: "openai-compatible".to_string(),
                api_style: "openapi".to_string(),
                base_url: api,
                token_env,
                env: env_names,
                domains: vec!["llm".to_string()],
                capabilities,
                auth_methods: vec!["api_key".to_string()],
                api_docs: Some(provider.doc),
                status: Some("configured".to_string()),
                models: HashMap::from([(IMPORTED_MODEL_BUCKET.to_string(), models)]),
            },
        );
    }

    Ok(ModelsDevProjection {
        provider_base_url,
        providers,
        provenance,
    })
}

pub fn merge_projection(
    config: &mut RootConfig,
    projection: ModelsDevProjection,
) -> Result<(), ModelsDevError> {
    let occupied_ids = config
        .provider_base_url
        .keys()
        .chain(config.model_catalog.providers.keys())
        .map(|provider_id| provider_id.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let mut provider_ids = projection.providers.keys().cloned().collect::<Vec<_>>();
    provider_ids.sort();
    let mut additions = Vec::new();

    for provider_id in provider_ids {
        if occupied_ids.contains(&provider_id.to_ascii_lowercase()) {
            continue;
        }
        let provider = projection
            .providers
            .get(&provider_id)
            .cloned()
            .ok_or_else(|| invalid(&provider_id, "projection is missing provider metadata"))?;
        let base_url = projection
            .provider_base_url
            .get(&provider_id)
            .cloned()
            .ok_or_else(|| invalid(&provider_id, "projection is missing the provider base URL"))?;
        additions.push((provider_id, base_url, provider));
    }

    for (provider_id, base_url, provider) in additions {
        config
            .provider_base_url
            .insert(provider_id.clone(), base_url);
        config.model_catalog.providers.insert(provider_id, provider);
    }
    Ok(())
}

pub(crate) fn install_runtime_metadata(settings: &crate::tura_llm::Settings) {
    let mut metadata = ImportedRuntimeMetadata::default();
    for (provider_id, provider) in &settings.model_catalog.providers {
        let imported = provider.models.contains_key(IMPORTED_MODEL_BUCKET);
        if !imported {
            continue;
        }
        metadata
            .token_envs
            .insert(provider_id.to_ascii_lowercase(), provider.env.clone());
        for model in provider.models.values().flatten() {
            metadata
                .model_ids
                .insert((provider_id.to_ascii_lowercase(), model.id().to_string()));
        }
    }
    *runtime_metadata()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = metadata;
}

fn validate_provider_identity(
    provider_id: &str,
    provider: &RawProvider,
) -> Result<(), ModelsDevError> {
    if provider_id.is_empty()
        || !provider_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_.".contains(&byte)
        })
    {
        return Err(invalid(provider_id, "provider ID is not canonical"));
    }
    if provider.id != provider_id {
        return Err(invalid(
            provider_id,
            format!("embedded ID `{}` does not match object key", provider.id),
        ));
    }
    if provider.name.trim().is_empty() || provider.doc.trim().is_empty() {
        return Err(invalid(
            provider_id,
            "provider display name and docs are required",
        ));
    }
    Ok(())
}

fn validate_api(provider_id: &str, api: Option<&str>) -> Result<Option<String>, ModelsDevError> {
    let api = api
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(provider_id, "OpenAI-compatible provider has no API URL"))?;
    if api.contains("${") {
        return Ok(None);
    }
    let url = Url::parse(api).map_err(|error| invalid(provider_id, error.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(invalid(
            provider_id,
            "API URL must be an absolute HTTP(S) URL",
        ));
    }
    Ok(Some(api.trim_end_matches('/').to_string()))
}

fn validate_env_names(
    provider_id: &str,
    env_names: Vec<String>,
) -> Result<Vec<String>, ModelsDevError> {
    if env_names.is_empty() {
        return Err(invalid(provider_id, "authentication env list is empty"));
    }
    let mut seen = HashSet::new();
    for name in &env_names {
        let valid = !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte == b'_' || byte.is_ascii_uppercase() || byte.is_ascii_digit());
        if !valid || !seen.insert(name.clone()) {
            return Err(invalid(
                provider_id,
                format!("invalid or duplicate env `{name}`"),
            ));
        }
    }
    Ok(env_names)
}

fn project_models(
    provider_id: &str,
    provider_api: &str,
    models: StrictMap<RawModel>,
    provenance: &ModelsDevProvenance,
) -> Result<Vec<CatalogModelConfig>, ModelsDevError> {
    let mut result = Vec::new();
    let mut ids = HashSet::new();
    for (model_id, model) in models.0 {
        let record = format!("{provider_id}/{model_id}");
        if model_id.trim().is_empty() || model.id != model_id {
            return Err(invalid(
                &record,
                "embedded model ID does not match object key",
            ));
        }
        if !ids.insert(model_id.to_ascii_lowercase()) {
            return Err(invalid(&record, "case-insensitive model ID collision"));
        }
        validate_model_classification(&record, &model)?;
        if !model.modalities.input.iter().any(|item| item == "text")
            || !model.modalities.output.iter().any(|item| item == "text")
            || !model_protocol_is_supported(&model, provider_api)
            || model_is_unambiguously_non_chat(&model)
            || model.limit.context == 0
            || model.limit.output == 0
        {
            continue;
        }
        validate_model(&record, &model)?;
        result.push(CatalogModelConfig::Detailed(project_model(
            model, provenance,
        )));
    }
    result.sort_by(|left, right| left.id().cmp(right.id()));
    Ok(result)
}

fn validate_model(record: &str, model: &RawModel) -> Result<(), ModelsDevError> {
    if model.name.trim().is_empty()
        || model.description.trim().is_empty()
        || model.release_date.trim().is_empty()
        || model.last_updated.trim().is_empty()
    {
        return Err(invalid(record, "required model metadata is missing"));
    }
    Ok(())
}

fn validate_model_classification(record: &str, model: &RawModel) -> Result<(), ModelsDevError> {
    for values in [&model.modalities.input, &model.modalities.output] {
        if values.is_empty() {
            return Err(invalid(record, "model modalities must not be empty"));
        }
        let mut seen = HashSet::new();
        for value in values {
            if !matches!(value.as_str(), "text" | "audio" | "image" | "video" | "pdf")
                || !seen.insert(value)
            {
                return Err(invalid(record, "model modalities are invalid or ambiguous"));
            }
        }
    }
    if let Some(status) = &model.status
        && !matches!(status.as_str(), "alpha" | "beta" | "deprecated")
    {
        return Err(invalid(record, format!("unknown model status `{status}`")));
    }
    Ok(())
}

fn model_is_unambiguously_non_chat(model: &RawModel) -> bool {
    model.family.as_deref() == Some("text-embedding")
        || model.description.starts_with("Embedding model for ")
        || model.description.starts_with("Reranking model for ")
        || has_non_chat_model_token(&model.id)
        || has_non_chat_model_token(&model.name)
}

fn has_non_chat_model_token(value: &str) -> bool {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .any(|segment| {
            segment == "embedding" || segment.starts_with("embed") || segment.starts_with("rerank")
        })
}

fn model_protocol_is_supported(model: &RawModel, provider_api: &str) -> bool {
    let Some(override_) = &model.provider else {
        return true;
    };
    override_
        .npm
        .as_deref()
        .is_none_or(|npm| npm == OPENAI_COMPATIBLE_NPM)
        && override_
            .shape
            .as_deref()
            .is_none_or(|shape| shape == "completions")
        && override_
            .api
            .as_deref()
            .is_none_or(|api| api.trim_end_matches('/') == provider_api)
        && override_.body.as_ref().is_none_or(HashMap::is_empty)
        && override_.headers.as_ref().is_none_or(HashMap::is_empty)
}

fn project_model(model: RawModel, provenance: &ModelsDevProvenance) -> CatalogModelDetail {
    let mut options = Map::new();
    options.insert("description".to_string(), Value::String(model.description));
    options.insert(
        "last_updated".to_string(),
        Value::String(model.last_updated),
    );
    options.insert("open_weights".to_string(), Value::Bool(model.open_weights));
    options.insert(
        "structured_output".to_string(),
        Value::Bool(model.structured_output.unwrap_or(false)),
    );
    if let Some(knowledge) = model.knowledge {
        options.insert("knowledge".to_string(), Value::String(knowledge));
    }
    options.insert(
        "models_dev_source".to_string(),
        serde_json::to_value(provenance).expect("provenance serialization cannot fail"),
    );
    CatalogModelDetail {
        id: model.id,
        visible: model.status.as_deref() != Some("deprecated"),
        name: model.name,
        family: model.family.unwrap_or_default(),
        release_date: model.release_date,
        attachment: model.attachment,
        reasoning: model.reasoning,
        temperature: model.temperature.unwrap_or(false),
        tool_call: model.tool_call,
        limit: CatalogModelLimit {
            context: model.limit.context,
            input: model.limit.input.unwrap_or(model.limit.context),
            output: model.limit.output,
        },
        modalities: CatalogModelModalities {
            input: model.modalities.input,
            output: model.modalities.output,
        },
        options,
        status: model.status,
    }
}

fn provider_capabilities(models: &[CatalogModelConfig]) -> Vec<String> {
    let mut capabilities = vec!["llm.chat".to_string()];
    let details: Vec<_> = models
        .iter()
        .filter_map(CatalogModelConfig::detail)
        .collect();
    if details.iter().any(|model| model.tool_call) {
        capabilities.push("llm.tool_call".to_string());
    }
    if details.iter().any(|model| {
        model
            .modalities
            .input
            .iter()
            .any(|item| matches!(item.as_str(), "image" | "video" | "pdf"))
    }) {
        capabilities.push("llm.vision".to_string());
    }
    if details.iter().any(|model| {
        model
            .modalities
            .input
            .iter()
            .chain(&model.modalities.output)
            .any(|item| item == "audio")
    }) {
        capabilities.push("audio".to_string());
    }
    capabilities
}

fn invalid(record: impl Into<String>, message: impl Into<String>) -> ModelsDevError {
    ModelsDevError::Invalid {
        record: record.into(),
        message: message.into(),
    }
}
