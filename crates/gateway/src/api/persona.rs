use crate::api::registry;
use crate::contracts::{BadRequestError, UpsertPersonaRequest};
use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::path::{Component, Path as FilePath};

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

pub async fn get_persona_media(
    Path((persona_id, media_path)): Path<(String, String)>,
) -> Result<Response, (StatusCode, String)> {
    let root = registry::project_root();
    let persona = tura_persona::store::load_persona(&root, &persona_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("persona `{persona_id}` not found"),
        )
    })?;
    let persona_root = root.join(&persona.summary.path);
    let media = persona.config.media.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("persona `{persona_id}` has no media"),
        )
    })?;
    let configured_media_root = root.join(media.root_directory);
    let media_root = resolve_persona_media_root(&persona_root, &configured_media_root)?;
    let path = resolve_persona_media_path(&media_root, &media_path)?;
    let mime_type = persona_media_mime_type(&path).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "Persona media must be a supported image".to_string(),
        )
    })?;
    let bytes = std::fs::read(&path).map_err(|error| {
        (
            if error.kind() == std::io::ErrorKind::NotFound {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            error.to_string(),
        )
    })?;

    Ok((
        [
            (header::CONTENT_TYPE, mime_type),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response())
}

fn resolve_persona_media_root(
    persona_root: &FilePath,
    media_root: &FilePath,
) -> Result<std::path::PathBuf, (StatusCode, String)> {
    let canonical_persona_root = persona_root.canonicalize().map_err(|error| {
        (
            StatusCode::NOT_FOUND,
            format!("Persona directory is unavailable: {error}"),
        )
    })?;
    let canonical_media_root = media_root.canonicalize().map_err(|error| {
        (
            StatusCode::NOT_FOUND,
            format!("Persona media directory is unavailable: {error}"),
        )
    })?;
    if !canonical_media_root.starts_with(&canonical_persona_root) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Persona media directory must stay inside its persona directory".to_string(),
        ));
    }
    Ok(canonical_media_root)
}

fn resolve_persona_media_path(
    media_root: &FilePath,
    relative_path: &str,
) -> Result<std::path::PathBuf, (StatusCode, String)> {
    if FilePath::new(relative_path)
        .components()
        .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Persona media path must stay inside its media directory".to_string(),
        ));
    }
    let canonical_root = media_root.canonicalize().map_err(|error| {
        (
            StatusCode::NOT_FOUND,
            format!("Persona media directory is unavailable: {error}"),
        )
    })?;
    let requested = media_root.join(relative_path);
    let canonical_requested = requested.canonicalize().map_err(|error| {
        (
            if error.kind() == std::io::ErrorKind::NotFound {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            error.to_string(),
        )
    })?;
    if !canonical_requested.starts_with(&canonical_root) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Persona media path must stay inside its media directory".to_string(),
        ));
    }
    Ok(canonical_requested)
}

fn persona_media_mime_type(path: &FilePath) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        Some("png") => Some("image/png"),
        Some("webp") => Some("image/webp"),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::{persona_media_mime_type, resolve_persona_media_path, resolve_persona_media_root};
    use axum::http::StatusCode;
    use std::fs;
    use std::path::Path;

    #[test]
    fn persona_media_paths_stay_inside_the_persona_directory() {
        let temp = tempfile::tempdir().expect("temp directory");
        let persona_root = temp.path().join("personas/src/tura");
        let media_root = persona_root.join("media");
        fs::create_dir_all(media_root.join("expressions/vigilant/frames"))
            .expect("media directory");
        let frame = media_root.join("expressions/vigilant/frames/right.jpg");
        fs::write(&frame, b"jpg").expect("frame");

        let resolved_root =
            resolve_persona_media_root(&persona_root, &media_root).expect("media root");
        assert_eq!(
            resolve_persona_media_path(&resolved_root, "expressions/vigilant/frames/right.jpg")
                .expect("frame path"),
            frame.canonicalize().expect("canonical frame")
        );
        assert_eq!(
            resolve_persona_media_path(&resolved_root, "../persona_config.json")
                .expect_err("traversal must fail")
                .0,
            StatusCode::BAD_REQUEST
        );

        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).expect("outside directory");
        assert_eq!(
            resolve_persona_media_root(&persona_root, &outside)
                .expect_err("outside media root must fail")
                .0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn persona_media_mime_type_supports_jpeg() {
        assert_eq!(
            persona_media_mime_type(Path::new("avatar.jpg")),
            Some("image/jpeg")
        );
        assert_eq!(persona_media_mime_type(Path::new("avatar.txt")), None);
    }
}
