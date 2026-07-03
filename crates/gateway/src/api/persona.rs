use crate::api::registry;
use crate::contracts::{BadRequestError, UpsertPersonaRequest};
use axum::{extract::Path, http::StatusCode, Json};

pub async fn list_personas() -> Json<Vec<tura_persona::store::StoredPersona>> {
    Json(list_personas_value())
}

pub fn list_personas_value() -> Vec<tura_persona::store::StoredPersona> {
    tura_persona::store::discover_personas(&registry::project_root())
}

pub async fn get_persona(
    Path(persona_id): Path<String>,
) -> Result<Json<tura_persona::store::StoredPersona>, (StatusCode, Json<BadRequestError>)> {
    get_persona_value(persona_id)
        .map(Json)
        .map_err(|(status, error)| api_error(status, error))
}

pub fn get_persona_value(
    persona_id: String,
) -> Result<tura_persona::store::StoredPersona, (StatusCode, String)> {
    let root = registry::project_root();
    tura_persona::store::load_persona(&root, &persona_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("persona `{persona_id}` not found"),
        )
    })
}

pub async fn create_persona(
    Json(payload): Json<UpsertPersonaRequest>,
) -> Result<Json<tura_persona::store::StoredPersona>, (StatusCode, Json<BadRequestError>)> {
    create_persona_value(payload).map(Json).map_err(|err| {
        api_error(
            StatusCode::BAD_REQUEST,
            format!("failed to create persona: {err}"),
        )
    })
}

pub fn create_persona_value(
    payload: UpsertPersonaRequest,
) -> Result<tura_persona::store::StoredPersona, String> {
    upsert_persona_in_store(None, payload)
}

pub async fn update_persona(
    Path(persona_id): Path<String>,
    Json(payload): Json<UpsertPersonaRequest>,
) -> Result<Json<tura_persona::store::StoredPersona>, (StatusCode, Json<BadRequestError>)> {
    update_persona_value(persona_id, payload)
        .map(Json)
        .map_err(|err| {
            api_error(
                StatusCode::BAD_REQUEST,
                format!("failed to update persona: {err}"),
            )
        })
}

pub fn update_persona_value(
    persona_id: String,
    payload: UpsertPersonaRequest,
) -> Result<tura_persona::store::StoredPersona, String> {
    upsert_persona_in_store(Some(persona_id), payload)
}

pub async fn delete_persona(
    Path(persona_id): Path<String>,
) -> Result<Json<bool>, (StatusCode, Json<BadRequestError>)> {
    delete_persona_value(persona_id)
        .map(Json)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, err))
}

pub fn delete_persona_value(persona_id: String) -> Result<bool, String> {
    tura_persona::store::delete_dynamic_persona(&registry::project_root(), &persona_id)
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
