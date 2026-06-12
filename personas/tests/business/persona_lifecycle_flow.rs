use std::path::{Path, PathBuf};
use tura_persona::state_machine::{PersonaManagement, PersonaState};
use tura_persona::store::{
    default_persona_config, delete_dynamic_persona, discover_personas, load_persona,
    save_dynamic_persona, PersonaConfig, PersonaSource, DYNAMIC_PERSONAS_DIR, PERSONA_CONFIG_FILE,
    PERSONA_PROMPT_DIR, STATIC_PERSONAS_DIR,
};

#[test]
fn persona_business_lifecycle_saves_discovers_prefers_dynamic_and_deletes() {
    let project = temp_project();
    write_static_persona(project.path(), "guide", "Static Guide", false);
    let mut config = default_persona_config(project.path(), "Guide").expect("default config");
    config.display_name = Some("Dynamic Guide".to_string());
    config.description = Some("Guides local workflows".to_string());
    config.short_description = Some("Guide".to_string());
    config.metadata = serde_json::json!({"voice": "direct"});

    let saved = save_dynamic_persona(
        project.path(),
        &config,
        Some("Persona prompt for local business flow."),
        Some("Use concise operational language."),
    )
    .expect("save dynamic persona");

    assert_eq!(saved.summary.id, "guide");
    assert_eq!(saved.summary.display_name, "Dynamic Guide");
    assert_eq!(saved.summary.description, "Guides local workflows");
    assert_eq!(saved.summary.short_description, "Guide");
    assert_eq!(saved.summary.source, PersonaSource::Dynamic);
    assert_eq!(saved.summary.state, PersonaState::Active);
    assert_eq!(saved.management.persona_id, "guide");
    assert_eq!(saved.management.persona_name, "Dynamic Guide");
    assert_eq!(saved.management.state, PersonaState::Active);
    assert_eq!(
        saved.persona.as_deref(),
        Some("Persona prompt for local business flow.")
    );
    assert_eq!(
        saved.communication_style.as_deref(),
        Some("Use concise operational language.")
    );

    let loaded = load_persona(project.path(), "GUIDE").expect("load dynamic persona");
    assert_eq!(loaded.summary.source, PersonaSource::Dynamic);
    assert_eq!(loaded.summary.display_name, "Dynamic Guide");
    let discovered = discover_personas(project.path());
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].summary.id, "guide");
    assert_eq!(
        discovered[0].summary.source,
        PersonaSource::Dynamic,
        "dynamic persona should win over static persona with the same id"
    );
    assert!(project
        .path()
        .join(DYNAMIC_PERSONAS_DIR)
        .join("guide")
        .join(PERSONA_CONFIG_FILE)
        .exists());
    assert!(project
        .path()
        .join(DYNAMIC_PERSONAS_DIR)
        .join("guide")
        .join(PERSONA_PROMPT_DIR)
        .join("persona.md")
        .exists());
    assert!(project
        .path()
        .join(DYNAMIC_PERSONAS_DIR)
        .join("guide")
        .join(PERSONA_PROMPT_DIR)
        .join("communication_style.md")
        .exists());

    let mut management = PersonaManagement::new(
        saved.management.persona_id.clone(),
        saved.management.persona_name.clone(),
        saved.management.persona_directory.clone(),
        saved.management.default_config,
    );
    management
        .transition(PersonaState::Active)
        .expect("draft persona can become active");
    management
        .transition(PersonaState::Archived)
        .expect("active persona can be archived");
    assert!(management.transition(PersonaState::Draft).is_err());

    assert!(delete_dynamic_persona(project.path(), "Guide").expect("delete dynamic"));
    let remaining = load_persona(project.path(), "guide").expect("static persona remains");
    assert_eq!(remaining.summary.source, PersonaSource::Static);
    assert_eq!(remaining.summary.display_name, "Static Guide");
    assert!(!delete_dynamic_persona(project.path(), "missing").expect("missing delete"));
}

#[test]
fn persona_business_rules_reject_user_default_config_and_static_delete() {
    let project = temp_project();
    let mut config = default_persona_config(project.path(), "bad-default").expect("config");
    config.default_config = true;
    let error = save_dynamic_persona(project.path(), &config, None, None)
        .expect_err("user persona cannot set default_config");
    assert_eq!(
        error,
        "user-created personas cannot set default_config=true"
    );

    write_static_persona(project.path(), "builtin", "Built In Persona", true);
    let loaded = load_persona(project.path(), "BUILTIN").expect("load static persona");
    assert_eq!(loaded.summary.source, PersonaSource::Static);
    assert!(loaded.summary.default_config);

    let delete_error = delete_dynamic_persona(project.path(), "builtin")
        .expect_err("static default persona cannot be deleted");
    assert!(delete_error.contains("default_config"), "{delete_error}");
    assert!(project
        .path()
        .join(STATIC_PERSONAS_DIR)
        .join("builtin")
        .exists());
}

fn temp_project() -> tempfile::TempDir {
    let project = tempfile::tempdir().expect("temp project");
    std::fs::create_dir_all(project.path().join(DYNAMIC_PERSONAS_DIR))
        .expect("dynamic personas dir");
    std::fs::create_dir_all(project.path().join(STATIC_PERSONAS_DIR)).expect("static personas dir");
    project
}

fn write_static_persona(root: &Path, id: &str, display_name: &str, default_config: bool) {
    let dir = root.join(STATIC_PERSONAS_DIR).join(id);
    std::fs::create_dir_all(dir.join(PERSONA_PROMPT_DIR)).expect("static prompt dir");
    let config = PersonaConfig {
        persona_name: id.to_string(),
        display_name: Some(display_name.to_string()),
        description: Some(format!("{display_name} description")),
        short_description: Some(display_name.to_string()),
        default_config,
        persona_directory: PathBuf::from(STATIC_PERSONAS_DIR).join(id),
        prompt_directory: PathBuf::from(STATIC_PERSONAS_DIR)
            .join(id)
            .join(PERSONA_PROMPT_DIR),
        media: None,
        metadata: serde_json::json!({}),
    };
    std::fs::write(
        dir.join(PERSONA_CONFIG_FILE),
        serde_json::to_string_pretty(&config).expect("config json"),
    )
    .expect("write static persona config");
    std::fs::write(
        dir.join(PERSONA_PROMPT_DIR).join("persona.md"),
        "Static persona prompt",
    )
    .expect("write static persona prompt");
}
