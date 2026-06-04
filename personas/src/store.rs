use crate::state_machine::{PersonaManagement, PersonaState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const DYNAMIC_PERSONAS_DIR: &str = "personas";
pub const STATIC_PERSONAS_DIR: &str = "personas/src";
pub const PERSONA_CONFIG_FILE: &str = "persona_config.json";
pub const PERSONA_PROMPT_DIR: &str = "prompt";
pub const EXPRESSION_MANIFEST_FILE: &str = "personas/src/expression_manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PersonaSource {
    Dynamic,
    Static,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonaFrameSet {
    #[serde(flatten)]
    pub frames: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonaExpression {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub emoji_aliases: Vec<String>,
    pub source_directory: PathBuf,
    pub grid_path: PathBuf,
    #[serde(default)]
    pub frames: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonaMediaConfig {
    pub name: String,
    pub root_directory: PathBuf,
    pub expression_directory: PathBuf,
    #[serde(default)]
    pub direction_order: Vec<String>,
    pub default_expression: String,
    pub default_direction: String,
    #[serde(default)]
    pub expression_manifest: Option<PathBuf>,
    #[serde(default)]
    pub expressions: Vec<PersonaExpression>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExpressionManifest {
    #[serde(default, rename = "directionOrder")]
    direction_order: Vec<String>,
    #[serde(default)]
    expressions: Vec<ExpressionManifestItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExpressionManifestItem {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "emojiAliases")]
    emoji_aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonaConfig {
    pub persona_name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub short_description: Option<String>,
    #[serde(default)]
    pub default_config: bool,
    pub persona_directory: PathBuf,
    pub prompt_directory: PathBuf,
    #[serde(default)]
    pub media: Option<PersonaMediaConfig>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonaSummary {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub short_description: String,
    pub source: PersonaSource,
    pub path: PathBuf,
    pub default_config: bool,
    pub state: PersonaState,
    #[serde(default)]
    pub media: Option<PersonaMediaConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredPersona {
    pub summary: PersonaSummary,
    pub config: PersonaConfig,
    #[serde(default)]
    pub persona: Option<String>,
    #[serde(default)]
    pub communication_style: Option<String>,
    pub management: PersonaManagement,
}

pub fn discover_personas(project_root: &Path) -> Vec<StoredPersona> {
    let mut personas = BTreeMap::<String, StoredPersona>::new();
    for (source, root) in persona_roots(project_root) {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(persona) = load_persona_at(project_root, &path, source.clone()) {
                personas
                    .entry(persona.summary.id.to_ascii_lowercase())
                    .or_insert(persona);
            }
        }
    }
    personas.into_values().collect()
}

pub fn load_persona(project_root: &Path, persona_id: &str) -> Option<StoredPersona> {
    let normalized = normalize_id(persona_id);
    discover_personas(project_root)
        .into_iter()
        .find(|persona| persona.summary.id.eq_ignore_ascii_case(&normalized))
}

pub fn default_persona_config(
    project_root: &Path,
    persona_id: &str,
) -> Result<PersonaConfig, String> {
    let persona_dir = dynamic_persona_path(project_root, persona_id)?;
    let relative_dir = path_relative_to(project_root, &persona_dir);
    let prompt_directory = relative_dir.join(PERSONA_PROMPT_DIR);
    Ok(PersonaConfig {
        persona_name: normalize_id(persona_id),
        display_name: Some(persona_id.to_string()),
        description: Some("Custom persona".to_string()),
        short_description: Some("Custom".to_string()),
        default_config: false,
        persona_directory: relative_dir,
        prompt_directory,
        media: None,
        metadata: serde_json::json!({}),
    })
}

pub fn save_dynamic_persona(
    project_root: &Path,
    config: &PersonaConfig,
    persona: Option<&str>,
    communication_style: Option<&str>,
) -> Result<StoredPersona, String> {
    if config.default_config {
        return Err("user-created personas cannot set default_config=true".to_string());
    }
    let persona_id = normalize_id(&config.persona_name);
    let persona_dir = dynamic_persona_path(project_root, &persona_id)?;
    fs::create_dir_all(persona_dir.join(PERSONA_PROMPT_DIR)).map_err(|err| {
        format!(
            "failed to create persona directory {}: {err}",
            persona_dir.display()
        )
    })?;

    let mut config = config.clone();
    config.persona_name = persona_id;
    config.persona_directory = path_relative_to(project_root, &persona_dir);
    config.prompt_directory = config.persona_directory.join(PERSONA_PROMPT_DIR);
    config.default_config = false;

    let encoded = serde_json::to_string_pretty(&config)
        .map_err(|err| format!("failed to encode persona config: {err}"))?;
    fs::write(persona_dir.join(PERSONA_CONFIG_FILE), encoded).map_err(|err| {
        format!(
            "failed to write persona config {}: {err}",
            persona_dir.join(PERSONA_CONFIG_FILE).display()
        )
    })?;
    if let Some(persona) = persona {
        fs::write(
            config.prompt_directory(project_root).join("persona.md"),
            persona,
        )
        .map_err(|err| format!("failed to write persona prompt: {err}"))?;
    }
    if let Some(communication_style) = communication_style {
        fs::write(
            config
                .prompt_directory(project_root)
                .join("communication_style.md"),
            communication_style,
        )
        .map_err(|err| format!("failed to write communication style prompt: {err}"))?;
    }

    load_persona_at(project_root, &persona_dir, PersonaSource::Dynamic)
        .ok_or_else(|| format!("failed to reload persona {}", config.persona_name))
}

pub fn delete_dynamic_persona(project_root: &Path, persona_id: &str) -> Result<bool, String> {
    if let Some(persona) = load_persona(project_root, persona_id) {
        if persona.summary.default_config {
            return Err(format!(
                "persona {} is a default_config and cannot be deleted",
                persona.summary.id
            ));
        }
        if persona.summary.source == PersonaSource::Static {
            return Err(format!(
                "persona {} is static and cannot be deleted",
                persona.summary.id
            ));
        }
    }
    let persona_dir = dynamic_persona_path(project_root, persona_id)?;
    if !persona_dir.exists() {
        return Ok(false);
    }
    fs::remove_dir_all(&persona_dir)
        .map_err(|err| format!("failed to delete persona {}: {err}", persona_dir.display()))?;
    Ok(true)
}

pub fn project_root_from_env_or_cwd() -> PathBuf {
    if let Ok(root) = std::env::var("TURA_PROJECT_ROOT") {
        let root = PathBuf::from(root);
        if root.exists() {
            return root;
        }
    }
    let current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for candidate in current.ancestors() {
        if candidate.join("Cargo.toml").exists() && candidate.join("crates").exists() {
            return candidate.to_path_buf();
        }
    }
    current
}

fn load_persona_at(
    project_root: &Path,
    directory: &Path,
    source: PersonaSource,
) -> Option<StoredPersona> {
    let config_path = directory.join(PERSONA_CONFIG_FILE);
    if !config_path.exists() {
        return None;
    }
    let content = fs::read_to_string(&config_path).ok()?;
    let mut config: PersonaConfig = serde_json::from_str(&content).ok()?;
    apply_expression_manifest(project_root, &mut config);
    let prompt_dir = project_root.join(&config.prompt_directory);
    let persona = fs::read_to_string(prompt_dir.join("persona.md")).ok();
    let communication_style = fs::read_to_string(prompt_dir.join("communication_style.md")).ok();
    let id = normalize_id(&config.persona_name);
    let management = PersonaManagement {
        persona_id: id.clone(),
        persona_name: config
            .display_name
            .clone()
            .unwrap_or_else(|| config.persona_name.clone()),
        persona_directory: config.persona_directory.clone(),
        default_config: config.default_config,
        state: PersonaState::Active,
    };
    Some(StoredPersona {
        summary: PersonaSummary {
            id,
            display_name: config
                .display_name
                .clone()
                .unwrap_or_else(|| config.persona_name.clone()),
            description: config.description.clone().unwrap_or_default(),
            short_description: config
                .short_description
                .clone()
                .or_else(|| config.description.clone())
                .unwrap_or_else(|| config.persona_name.clone()),
            source,
            path: path_relative_to(project_root, directory),
            default_config: config.default_config,
            state: PersonaState::Active,
            media: config.media.clone(),
        },
        config,
        persona,
        communication_style,
        management,
    })
}

fn apply_expression_manifest(project_root: &Path, config: &mut PersonaConfig) {
    let Some(media) = config.media.as_mut() else {
        return;
    };
    let manifest_path = media
        .expression_manifest
        .clone()
        .unwrap_or_else(|| PathBuf::from(EXPRESSION_MANIFEST_FILE));
    media.expression_manifest = Some(manifest_path.clone());

    let manifest = fs::read_to_string(project_root.join(manifest_path))
        .ok()
        .and_then(|content| serde_json::from_str::<ExpressionManifest>(&content).ok());
    let Some(manifest) = manifest else {
        return;
    };

    if media.direction_order.is_empty() {
        media.direction_order = manifest.direction_order.clone();
    }

    let by_id = manifest
        .expressions
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect::<BTreeMap<_, _>>();
    for expression in &mut media.expressions {
        if let Some(item) = by_id.get(&expression.id) {
            expression.name = item.name.clone();
            expression.emoji_aliases = item.emoji_aliases.clone();
        } else if expression.name.is_empty() {
            expression.name = expression.id.clone();
        }
    }
}

fn persona_roots(project_root: &Path) -> [(PersonaSource, PathBuf); 2] {
    [
        (
            PersonaSource::Dynamic,
            project_root.join(DYNAMIC_PERSONAS_DIR),
        ),
        (
            PersonaSource::Static,
            project_root.join(STATIC_PERSONAS_DIR),
        ),
    ]
}

fn dynamic_persona_path(project_root: &Path, persona_id: &str) -> Result<PathBuf, String> {
    let id = normalize_id(persona_id);
    if id.is_empty()
        || id.contains('/')
        || id.contains('\\')
        || id == "."
        || id == ".."
        || id
            .chars()
            .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
    {
        return Err(format!("invalid persona id: {persona_id}"));
    }
    Ok(project_root.join(DYNAMIC_PERSONAS_DIR).join(id))
}

fn normalize_id(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}

fn path_relative_to(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

impl PersonaConfig {
    fn prompt_directory(&self, project_root: &Path) -> PathBuf {
        project_root.join(&self.prompt_directory)
    }
}
