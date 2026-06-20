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

#[cfg(test)]
mod tests {
    use super::process_image;
    use crate::types::ReadMediaArgs;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;

    fn args(max_side: u32) -> ReadMediaArgs {
        ReadMediaArgs {
            paths: vec!["image.png".to_string()],
            include_text: true,
            max_text_chars: 10_000,
            max_visuals: 4,
            max_side,
            max_files: 10,
            pdf_max_pages: 2,
            document_attachment_bytes: 100_000,
            audio_preview_bytes: 100_000,
        }
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_fn(width, height, |x, y| {
            if (x + y) % 2 == 0 {
                Rgba([240, 20, 80, 255])
            } else {
                Rgba([10, 120, 240, 255])
            }
        }));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .expect("encode png");
        bytes
    }

    #[test]
    fn process_image_returns_single_jpeg_data_url_preview() {
        let dir = tempfile::tempdir().expect("tempdir");
        let image_path = dir.path().join("sample.png");
        std::fs::write(&image_path, png_bytes(32, 24)).expect("write image");

        let previews = process_image(&image_path, &args(128)).expect("image should decode");

        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0]["type"], "image_url");
        assert_eq!(previews[0]["label"], "IMG");
        let url = previews[0]["image_url"]["url"]
            .as_str()
            .expect("data url string");
        assert!(url.starts_with("data:image/jpeg;base64,"));
        assert!(url.len() > "data:image/jpeg;base64,".len());
    }

    #[test]
    fn process_image_respects_small_preview_side_without_changing_contract() {
        let dir = tempfile::tempdir().expect("tempdir");
        let image_path = dir.path().join("large.png");
        std::fs::write(&image_path, png_bytes(200, 80)).expect("write image");

        let previews = process_image(&image_path, &args(16)).expect("image should downscale");
        let url = previews[0]["image_url"]["url"]
            .as_str()
            .expect("data url string");

        assert!(url.starts_with("data:image/jpeg;base64,"));
        assert!(!url.contains('\n'));
    }

    #[test]
    fn process_image_reports_decode_errors_with_context() {
        let dir = tempfile::tempdir().expect("tempdir");
        let image_path = dir.path().join("not-image.bin");
        std::fs::write(&image_path, b"not an image").expect("write invalid image");

        let error = process_image(&image_path, &args(128)).expect_err("bad image should fail");

        assert!(error.contains("failed to decode image"));
    }

    #[test]
    fn process_image_reports_missing_file_errors_with_context() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing.png");

        let error = process_image(&missing, &args(128)).expect_err("missing image should fail");

        assert!(error.contains("failed to read image"));
    }
}
