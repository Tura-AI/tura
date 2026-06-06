use super::previews::encode_preview_jpeg;
use super::types::ReadMediaArgs;
use serde_json::{json, Value};
use std::path::Path;

pub(super) fn process_image(path: &Path, args: &ReadMediaArgs) -> Result<Vec<Value>, String> {
    let bytes = std::fs::read(path).map_err(|err| format!("failed to read image: {err}"))?;
    let image =
        image::load_from_memory(&bytes).map_err(|err| format!("failed to decode image: {err}"))?;
    let encoded = encode_preview_jpeg(image, args.max_side, 80)?;
    Ok(vec![json!({
        "type": "image_url",
        "label": "IMG",
        "image_url": { "url": format!("data:image/jpeg;base64,{}", encoded) }
    })])
}
