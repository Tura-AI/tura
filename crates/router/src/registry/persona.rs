use serde::{Deserialize, Serialize};
use tura_persona::store::{discover_personas, project_root_from_env_or_cwd};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertPersonaRequest {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub config: Option<tura_persona::store::PersonaConfig>,
    #[serde(default)]
    pub persona: Option<String>,
    #[serde(default)]
    pub communication_style: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct PersonaRegistry;

impl PersonaRegistry {
    pub fn from_static() -> Self {
        Self
    }

    pub fn list(&self) -> Vec<tura_persona::store::StoredPersona> {
        discover_personas(&project_root_from_env_or_cwd())
    }

    pub fn get(&self, persona_id: &str) -> Option<tura_persona::store::StoredPersona> {
        tura_persona::store::load_persona(&project_root_from_env_or_cwd(), persona_id)
    }

    pub fn upsert(
        &self,
        persona_id: Option<String>,
        payload: UpsertPersonaRequest,
    ) -> Result<tura_persona::store::StoredPersona, String> {
        let project_root = project_root_from_env_or_cwd();
        let persona_id = persona_id
            .or(payload.id)
            .or_else(|| {
                payload
                    .config
                    .as_ref()
                    .map(|config| config.persona_name.clone())
            })
            .ok_or_else(|| "persona id is required".to_string())?;
        let mut config = payload.config.unwrap_or(
            tura_persona::store::load_persona(&project_root, &persona_id)
                .map(|persona| persona.config)
                .unwrap_or(tura_persona::store::default_persona_config(
                    &project_root,
                    &persona_id,
                )?),
        );
        config.persona_name = persona_id;
        tura_persona::store::save_dynamic_persona(
            &project_root,
            &config,
            payload.persona.as_deref(),
            payload.communication_style.as_deref(),
        )
    }

    pub fn delete(&self, persona_id: &str) -> Result<bool, String> {
        tura_persona::store::delete_dynamic_persona(&project_root_from_env_or_cwd(), persona_id)
    }
}
