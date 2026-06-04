use crate::api::{registry, types::BadRequestError};
use axum::{extract::Path, http::StatusCode, Json};

pub async fn list_personas() -> Json<Vec<tura_persona::store::StoredPersona>> {
    Json(tura_persona::store::discover_personas(
        &registry::project_root(),
    ))
}

pub async fn get_persona(
    Path(persona_id): Path<String>,
) -> Result<Json<tura_persona::store::StoredPersona>, (StatusCode, Json<BadRequestError>)> {
    let root = registry::project_root();
    tura_persona::store::load_persona(&root, &persona_id)
        .map(Json)
        .ok_or_else(|| {
            api_error(
                StatusCode::NOT_FOUND,
                format!("persona `{persona_id}` not found"),
            )
        })
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
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

pub async fn create_persona(
    Json(payload): Json<UpsertPersonaRequest>,
) -> Result<Json<tura_persona::store::StoredPersona>, (StatusCode, Json<BadRequestError>)> {
    upsert_persona_in_store(None, payload)
        .map(Json)
        .map_err(|err| {
            api_error(
                StatusCode::BAD_REQUEST,
                format!("failed to create persona: {err}"),
            )
        })
}

pub async fn update_persona(
    Path(persona_id): Path<String>,
    Json(payload): Json<UpsertPersonaRequest>,
) -> Result<Json<tura_persona::store::StoredPersona>, (StatusCode, Json<BadRequestError>)> {
    upsert_persona_in_store(Some(persona_id), payload)
        .map(Json)
        .map_err(|err| {
            api_error(
                StatusCode::BAD_REQUEST,
                format!("failed to update persona: {err}"),
            )
        })
}

pub async fn delete_persona(
    Path(persona_id): Path<String>,
) -> Result<Json<bool>, (StatusCode, Json<BadRequestError>)> {
    tura_persona::store::delete_dynamic_persona(&registry::project_root(), &persona_id)
        .map(Json)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, err))
}

fn api_error(status: StatusCode, error: String) -> (StatusCode, Json<BadRequestError>) {
    (status, Json(BadRequestError { error }))
}

fn upsert_persona_in_store(
    persona_id: Option<String>,
    payload: UpsertPersonaRequest,
) -> Result<tura_persona::store::StoredPersona, String> {
    let root = registry::project_root();
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
        tura_persona::store::load_persona(&root, &persona_id)
            .map(|persona| persona.config)
            .unwrap_or(tura_persona::store::default_persona_config(
                &root,
                &persona_id,
            )?),
    );
    config.persona_name = persona_id;
    tura_persona::store::save_dynamic_persona(
        &root,
        &config,
        payload.persona.as_deref(),
        payload.communication_style.as_deref(),
    )
}
