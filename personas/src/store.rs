use crate::state_machine::{PersonaManagement, PersonaState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const DYNAMIC_PERSONAS_DIR: &str = "personas";
pub const STATIC_PERSONAS_DIR: &str = "personas/src";
pub const PERSONA_CONFIG_FILE: &str = "persona_config.json";
pub const PERSONA_PROMPT_DIR: &str = "prompt";
pub const COMMUNICATION_STYLE_DIR: &str = "communication_style";
pub const COMMUNICATION_STYLE_FILE: &str = "communication_style.md";
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
    #[serde(default, rename = "reactKaomoji")]
    pub react_kaomoji: Vec<String>,
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
    #[serde(default, rename = "reactKaomoji")]
    react_kaomoji: BTreeMap<String, Vec<String>>,
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
                .join(COMMUNICATION_STYLE_FILE),
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
    let communication_style = read_communication_style(project_root, &prompt_dir);
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
            expression.react_kaomoji = item
                .react_kaomoji
                .get(&config.persona_name)
                .cloned()
                .unwrap_or_default();
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

fn read_communication_style(project_root: &Path, prompt_dir: &Path) -> Option<String> {
    [
        project_root
            .join(STATIC_PERSONAS_DIR)
            .join(COMMUNICATION_STYLE_DIR)
            .join(COMMUNICATION_STYLE_FILE),
        prompt_dir.join(COMMUNICATION_STYLE_FILE),
        prompt_dir.join("communication_stlye.md"),
    ]
    .into_iter()
    .find_map(|path| fs::read_to_string(path).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("temp project");
        fs::create_dir_all(temp.path().join(DYNAMIC_PERSONAS_DIR)).expect("dynamic personas dir");
        fs::create_dir_all(temp.path().join(STATIC_PERSONAS_DIR)).expect("static personas dir");
        temp
    }

    fn test_config(root: &Path, name: &str) -> PersonaConfig {
        let mut config = default_persona_config(root, name).expect("default persona");
        config.description = Some("Long persona description".to_string());
        config.short_description = Some("Short persona".to_string());
        config.metadata = serde_json::json!({"tone":"focused"});
        config
    }

    #[test]
    fn default_persona_config_normalizes_name_and_paths() {
        let temp = project();

        let config = default_persona_config(temp.path(), " Helpful Persona ")
            .expect("default persona config");

        assert_eq!(config.persona_name, "helpful_persona");
        assert_eq!(config.display_name.as_deref(), Some(" Helpful Persona "));
        assert_eq!(config.description.as_deref(), Some("Custom persona"));
        assert_eq!(config.short_description.as_deref(), Some("Custom"));
        assert_eq!(
            config.persona_directory,
            PathBuf::from("personas/helpful_persona")
        );
        assert_eq!(
            config.prompt_directory,
            PathBuf::from("personas/helpful_persona/prompt")
        );
        assert_eq!(
            config.prompt_directory(temp.path()),
            temp.path().join("personas/helpful_persona/prompt")
        );
        assert!(!config.default_config);
        assert!(config.media.is_none());
    }

    #[test]
    fn dynamic_persona_path_rejects_invalid_ids_and_accepts_space_normalized_ids() {
        let temp = project();
        for invalid in ["", "  ", ".", "..", "../x", "a/b", r"a\b", "中文"] {
            let error = dynamic_persona_path(temp.path(), invalid)
                .expect_err("invalid persona id should be rejected");
            assert!(error.contains("invalid persona id"), "{error}");
        }

        assert_eq!(
            dynamic_persona_path(temp.path(), "Helpful Persona").expect("valid path"),
            temp.path().join("personas/helpful_persona")
        );
    }

    #[test]
    fn save_dynamic_persona_writes_config_prompts_and_management_state() {
        let temp = project();
        let config = test_config(temp.path(), "Helpful Persona");

        let saved = save_dynamic_persona(
            temp.path(),
            &config,
            Some("Persona prompt"),
            Some("Short and direct"),
        )
        .expect("save persona");

        assert_eq!(saved.summary.id, "helpful_persona");
        assert_eq!(saved.summary.display_name, "Helpful Persona");
        assert_eq!(saved.summary.description, "Long persona description");
        assert_eq!(saved.summary.short_description, "Short persona");
        assert_eq!(saved.summary.source, PersonaSource::Dynamic);
        assert_eq!(
            saved.summary.path,
            PathBuf::from("personas/helpful_persona")
        );
        assert_eq!(saved.summary.state, PersonaState::Active);
        assert_eq!(saved.persona.as_deref(), Some("Persona prompt"));
        assert_eq!(
            saved.communication_style.as_deref(),
            Some("Short and direct")
        );
        assert_eq!(saved.management.persona_id, "helpful_persona");
        assert_eq!(saved.management.persona_name, "Helpful Persona");
        assert_eq!(saved.management.state, PersonaState::Active);

        assert!(temp
            .path()
            .join("personas/helpful_persona")
            .join(PERSONA_CONFIG_FILE)
            .exists());
        assert!(temp
            .path()
            .join("personas/helpful_persona/prompt/persona.md")
            .exists());
        assert!(temp
            .path()
            .join("personas/helpful_persona/prompt/communication_style.md")
            .exists());
    }

    #[test]
    fn save_dynamic_persona_rejects_user_default_config_flag() {
        let temp = project();
        let mut config = test_config(temp.path(), "bad");
        config.default_config = true;

        let error = save_dynamic_persona(temp.path(), &config, None, None)
            .expect_err("user personas cannot set default_config");

        assert_eq!(
            error,
            "user-created personas cannot set default_config=true"
        );
    }

    #[test]
    fn shared_communication_style_is_loaded_before_persona_local_style() {
        let temp = project();
        let shared_dir = temp
            .path()
            .join(STATIC_PERSONAS_DIR)
            .join(COMMUNICATION_STYLE_DIR);
        fs::create_dir_all(&shared_dir).expect("shared communication style dir");
        fs::write(
            shared_dir.join(COMMUNICATION_STYLE_FILE),
            "Shared communication style",
        )
        .expect("shared communication style");

        let saved = save_dynamic_persona(
            temp.path(),
            &test_config(temp.path(), "Helpful Persona"),
            Some("Persona prompt"),
            Some("Local communication style"),
        )
        .expect("save persona");

        assert_eq!(
            saved.communication_style.as_deref(),
            Some("Shared communication style")
        );
    }

    #[test]
    fn discover_personas_prefers_dynamic_over_static_with_same_id_and_sorts() {
        let temp = project();
        save_dynamic_persona(temp.path(), &test_config(temp.path(), "Zulu"), None, None)
            .expect("save zulu");
        save_dynamic_persona(temp.path(), &test_config(temp.path(), "Alpha"), None, None)
            .expect("save alpha");
        write_static_persona(temp.path(), "alpha", "Static Alpha", false);

        let discovered = discover_personas(temp.path());
        let ids = discovered
            .iter()
            .map(|persona| persona.summary.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["alpha", "zulu"]);
        let alpha = load_persona(temp.path(), "ALPHA").expect("load alpha");
        assert_eq!(alpha.summary.source, PersonaSource::Dynamic);
        assert_eq!(alpha.summary.display_name, "Alpha");
    }

    #[test]
    fn discover_personas_skips_missing_and_malformed_configs() {
        let temp = project();
        fs::create_dir_all(temp.path().join("personas/no-config")).expect("no config dir");
        fs::create_dir_all(temp.path().join("personas/bad-json")).expect("bad json dir");
        fs::write(
            temp.path()
                .join("personas/bad-json")
                .join(PERSONA_CONFIG_FILE),
            "{not-json",
        )
        .expect("bad config");
        save_dynamic_persona(temp.path(), &test_config(temp.path(), "valid"), None, None)
            .expect("valid persona");

        let discovered = discover_personas(temp.path());

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].summary.id, "valid");
    }

    #[test]
    fn expression_manifest_enriches_media_without_overwriting_existing_direction_order() {
        let temp = project();
        fs::write(
            temp.path().join(EXPRESSION_MANIFEST_FILE),
            serde_json::json!({
                "directionOrder": ["front", "left"],
                "expressions": [
                    {"id":"happy","name":"Happy","emojiAliases":[":happy:"],"reactKaomoji":{"media-persona":["(^_^)","(^o^)","(^_^)"]}},
                    {"id":"sad","name":"Sad","emojiAliases":[":sad:"]}
                ]
            })
            .to_string(),
        )
        .expect("manifest");
        let mut config = test_config(temp.path(), "media-persona");
        config.media = Some(PersonaMediaConfig {
            name: "Media".to_string(),
            root_directory: PathBuf::from("assets"),
            expression_directory: PathBuf::from("assets/expressions"),
            direction_order: Vec::new(),
            default_expression: "happy".to_string(),
            default_direction: "front".to_string(),
            expression_manifest: None,
            expressions: vec![
                PersonaExpression {
                    id: "happy".to_string(),
                    name: String::new(),
                    emoji_aliases: Vec::new(),
                    react_kaomoji: Vec::new(),
                    source_directory: PathBuf::from("happy"),
                    grid_path: PathBuf::from("happy/grid.png"),
                    frames: BTreeMap::new(),
                },
                PersonaExpression {
                    id: "unknown".to_string(),
                    name: String::new(),
                    emoji_aliases: Vec::new(),
                    react_kaomoji: Vec::new(),
                    source_directory: PathBuf::from("unknown"),
                    grid_path: PathBuf::from("unknown/grid.png"),
                    frames: BTreeMap::new(),
                },
            ],
        });

        let saved = save_dynamic_persona(temp.path(), &config, None, None).expect("save");
        let media = saved.summary.media.expect("media");

        assert_eq!(
            media.expression_manifest,
            Some(PathBuf::from(EXPRESSION_MANIFEST_FILE))
        );
        assert_eq!(media.direction_order, vec!["front", "left"]);
        assert_eq!(media.expressions[0].name, "Happy");
        assert_eq!(media.expressions[0].emoji_aliases, vec![":happy:"]);
        assert_eq!(
            media.expressions[0].react_kaomoji,
            vec!["(^_^)", "(^o^)", "(^_^)"]
        );
        assert_eq!(media.expressions[1].name, "unknown");
        assert!(media.expressions[1].emoji_aliases.is_empty());
        assert!(media.expressions[1].react_kaomoji.is_empty());
    }

    #[test]
    fn expression_manifest_keeps_existing_direction_order_and_missing_manifest_is_nonfatal() {
        let temp = project();
        let mut config = test_config(temp.path(), "media-persona");
        config.media = Some(PersonaMediaConfig {
            name: "Media".to_string(),
            root_directory: PathBuf::from("assets"),
            expression_directory: PathBuf::from("assets/expressions"),
            direction_order: vec!["custom".to_string()],
            default_expression: "happy".to_string(),
            default_direction: "custom".to_string(),
            expression_manifest: Some(PathBuf::from("missing-manifest.json")),
            expressions: vec![PersonaExpression {
                id: "happy".to_string(),
                name: String::new(),
                emoji_aliases: Vec::new(),
                react_kaomoji: Vec::new(),
                source_directory: PathBuf::from("happy"),
                grid_path: PathBuf::from("happy/grid.png"),
                frames: BTreeMap::new(),
            }],
        });

        let saved = save_dynamic_persona(temp.path(), &config, None, None).expect("save");
        let media = saved.summary.media.expect("media");

        assert_eq!(media.direction_order, vec!["custom"]);
        assert_eq!(
            media.expression_manifest,
            Some(PathBuf::from("missing-manifest.json"))
        );
        assert_eq!(media.expressions[0].name, "");
    }

    #[test]
    fn delete_dynamic_persona_is_idempotent_for_missing_and_removes_existing() {
        let temp = project();

        assert_eq!(
            delete_dynamic_persona(temp.path(), "missing").expect("missing delete"),
            false
        );
        save_dynamic_persona(
            temp.path(),
            &test_config(temp.path(), "remove-me"),
            None,
            None,
        )
        .expect("save");

        assert_eq!(
            delete_dynamic_persona(temp.path(), "remove-me").expect("delete"),
            true
        );
        assert!(load_persona(temp.path(), "remove-me").is_none());
        assert_eq!(
            delete_dynamic_persona(temp.path(), "remove-me").expect("second delete"),
            false
        );
    }

    #[test]
    fn delete_dynamic_persona_rejects_static_and_default_personas() {
        let temp = project();
        write_static_persona(temp.path(), "tura", "Tura", false);
        let static_error =
            delete_dynamic_persona(temp.path(), "tura").expect_err("static protected");
        assert!(static_error.contains("static and cannot be deleted"));

        write_static_persona(temp.path(), "builtin", "Built In", true);
        let default_error =
            delete_dynamic_persona(temp.path(), "builtin").expect_err("default protected");
        assert!(default_error.contains("default_config and cannot be deleted"));
    }

    #[test]
    fn summary_fallbacks_use_description_then_persona_name() {
        let temp = project();
        let mut config = default_persona_config(temp.path(), "fallback").expect("config");
        config.display_name = None;
        config.description = Some("Long fallback description".to_string());
        config.short_description = None;

        let saved = save_dynamic_persona(temp.path(), &config, None, None).expect("save");

        assert_eq!(saved.summary.display_name, "fallback");
        assert_eq!(saved.summary.description, "Long fallback description");
        assert_eq!(saved.summary.short_description, "Long fallback description");

        let mut config = default_persona_config(temp.path(), "bare").expect("config");
        config.display_name = None;
        config.description = None;
        config.short_description = None;
        let saved = save_dynamic_persona(temp.path(), &config, None, None).expect("save bare");
        assert_eq!(saved.summary.description, "");
        assert_eq!(saved.summary.short_description, "bare");
    }

    fn write_static_persona(root: &Path, id: &str, display_name: &str, default_config: bool) {
        let dir = root.join(STATIC_PERSONAS_DIR).join(id);
        fs::create_dir_all(dir.join(PERSONA_PROMPT_DIR)).expect("static persona dir");
        let config = PersonaConfig {
            persona_name: id.to_string(),
            display_name: Some(display_name.to_string()),
            description: Some(format!("{display_name} description")),
            short_description: None,
            default_config,
            persona_directory: PathBuf::from(STATIC_PERSONAS_DIR).join(id),
            prompt_directory: PathBuf::from(STATIC_PERSONAS_DIR)
                .join(id)
                .join(PERSONA_PROMPT_DIR),
            media: None,
            metadata: serde_json::json!({}),
        };
        fs::write(
            dir.join(PERSONA_CONFIG_FILE),
            serde_json::to_string_pretty(&config).expect("encode config"),
        )
        .expect("write static persona");
    }
}
