use super::document::process_document;
use super::media_image::process_image;
use super::paths::{media_type_for_path, temp_work_dir};
use super::pdf::process_pdf;
use super::previews::{draw_text, encode_preview_jpeg};
use super::types::{MediaContent, ReadMediaArgs, ReadMode};
use super::video::{process_audio, process_video, process_video_with_python_cv2, resolve_ffmpeg};
use base64::{engine::general_purpose, Engine as _};
use image::{DynamicImage, Rgb, RgbImage};
use pdfium_render::prelude::*;
use serde_json::{json, Value};
use std::path::Path;

pub(super) fn process_media_file(
    path: &Path,
    args: &ReadMediaArgs,
    mode: ReadMode,
) -> Result<MediaContent, String> {
    if !path.exists() {
        return Err(format!("media path does not exist: {}", path.display()));
    }
    if mode == ReadMode::ThumbnailOnly {
        return process_media_thumbnail(path, args);
    }
    match media_type_for_path(path) {
        "image" => process_image(path, args).map(|visual_previews| MediaContent {
            text: String::new(),
            visual_previews,
            audio_previews: Vec::new(),
            file_attachments: Vec::new(),
        }),
        "pdf" => process_pdf(path, args).map(|(text, visual_previews)| MediaContent {
            text,
            visual_previews,
            audio_previews: Vec::new(),
            file_attachments: Vec::new(),
        }),
        "video" => process_video(path, args),
        "audio" => process_audio(path, args).map(|audio_previews| MediaContent {
            text: String::new(),
            visual_previews: Vec::new(),
            audio_previews,
            file_attachments: Vec::new(),
        }),
        _ => process_document(path, args),
    }
}

fn process_media_thumbnail(path: &Path, args: &ReadMediaArgs) -> Result<MediaContent, String> {
    let visual_previews = match media_type_for_path(path) {
        "image" => process_image(path, args)?,
        "pdf" => {
            let (text, visual_previews) = process_pdf(path, args).unwrap_or_else(|_| {
                (
                    String::new(),
                    process_pdf_thumbnail(path, args)
                        .unwrap_or_else(|_| file_tile_preview(path, args)),
                )
            });
            return Ok(MediaContent {
                text,
                visual_previews,
                audio_previews: Vec::new(),
                file_attachments: Vec::new(),
            });
        }
        "video" => {
            process_video_thumbnail(path, args).unwrap_or_else(|_| file_tile_preview(path, args))
        }
        _ => {
            let mut content = process_document(path, args)?;
            content.file_attachments.clear();
            if content.visual_previews.is_empty() {
                content.visual_previews = file_tile_preview(path, args);
            }
            return Ok(content);
        }
    };
    Ok(MediaContent {
        text: String::new(),
        visual_previews,
        audio_previews: Vec::new(),
        file_attachments: Vec::new(),
    })
}

fn process_pdf_thumbnail(path: &Path, args: &ReadMediaArgs) -> Result<Vec<Value>, String> {
    let bindings =
        Pdfium::bind_to_system_library().map_err(|err| format!("failed to bind pdfium: {err}"))?;
    let pdfium = Pdfium::new(bindings);
    let doc = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|err| format!("failed to open pdf: {err}"))?;
    let page = doc
        .pages()
        .first()
        .map_err(|err| format!("failed to read first pdf page: {err}"))?;
    let rendered = page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(scaled_side(args.max_side, 2, 1) as i32)
                .render_form_data(true),
        )
        .map_err(|err| format!("failed to render pdf page: {err}"))?;
    let image = DynamicImage::ImageRgb8(rendered.as_image().to_rgb8());
    let encoded = encode_preview_jpeg(image, scaled_side(args.max_side, 2, 1), 80)?;
    Ok(vec![json!({
        "type": "image_url",
        "label": "P1",
        "image_url": { "url": format!("data:image/jpeg;base64,{}", encoded) }
    })])
}

fn scaled_side(value: u32, numerator: u32, denominator: u32) -> u32 {
    value.saturating_mul(numerator).div_ceil(denominator)
}

fn process_video_thumbnail(path: &Path, args: &ReadMediaArgs) -> Result<Vec<Value>, String> {
    let mut thumb_args = args.clone();
    thumb_args.max_visuals = 1;
    let Some(ffmpeg) = resolve_ffmpeg() else {
        return process_video_with_python_cv2(path, &thumb_args);
    };
    let temp_dir = temp_work_dir("tura-read-media-video-thumb");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|err| format!("failed to create temp video thumbnail dir: {err}"))?;
    let frame = temp_dir.join("frame.jpg");
    let mut command = std::process::Command::new(ffmpeg);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(path)
        .arg("-vf")
        .arg(format!("thumbnail,scale='min({},iw)':-2", args.max_side))
        .arg("-frames:v")
        .arg("1")
        .arg("-q:v")
        .arg("4")
        .arg("-y")
        .arg(&frame);
    tura_path::process_hardening::hide_child_console_window(&mut command);
    let status = command.status().map_err(|err| {
        let _ = std::fs::remove_dir_all(&temp_dir);
        format!("failed to run ffmpeg for video thumbnail: {err}")
    })?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return process_video_with_python_cv2(path, &thumb_args).map_err(|cv_err| {
            format!("ffmpeg video thumbnail failed with status {status}; {cv_err}")
        });
    }
    let bytes = std::fs::read(&frame).map_err(|err| {
        let _ = std::fs::remove_dir_all(&temp_dir);
        format!("failed to read video thumbnail: {err}")
    })?;
    let _ = std::fs::remove_dir_all(&temp_dir);
    if bytes.is_empty() {
        return Err("video thumbnail produced no data".to_string());
    }
    Ok(vec![json!({
        "type": "image_url",
        "label": "T0MS",
        "image_url": { "url": format!("data:image/jpeg;base64,{}", general_purpose::STANDARD.encode(bytes)) }
    })])
}

fn file_tile_preview(path: &Path, args: &ReadMediaArgs) -> Vec<Value> {
    let mut image = RgbImage::from_pixel(360, 220, Rgb([244, 244, 244]));
    for x in 0..360 {
        image.put_pixel(x, 0, Rgb([210, 210, 210]));
        image.put_pixel(x, 219, Rgb([210, 210, 210]));
    }
    for y in 0..220 {
        image.put_pixel(0, y, Rgb([210, 210, 210]));
        image.put_pixel(359, y, Rgb([210, 210, 210]));
    }
    let kind = media_type_for_path(path).to_ascii_uppercase();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("FILE")
        .to_ascii_uppercase();
    let size = std::fs::metadata(path)
        .map(|metadata| format!("{} KB", metadata.len().div_ceil(1024)))
        .unwrap_or_else(|_| "UNKNOWN SIZE".to_string());
    draw_text(&mut image, 24, 42, &kind, Rgb([20, 20, 20]));
    draw_text(&mut image, 24, 96, &name, Rgb([20, 20, 20]));
    draw_text(&mut image, 24, 150, &size, Rgb([80, 80, 80]));
    let encoded =
        encode_preview_jpeg(DynamicImage::ImageRgb8(image), args.max_side, 78).unwrap_or_default();
    vec![json!({
        "type": "image_url",
        "label": "FILE",
        "image_url": { "url": format!("data:image/jpeg;base64,{}", encoded) }
    })]
}

pub(super) fn media_result(path: &str, resolved: &Path, content: MediaContent) -> Value {
    let metadata = std::fs::metadata(resolved).ok();
    let media_type = media_type_for_path(resolved);
    let modified_unix_ms = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis());
    json!({
        "path": path,
        "resolved_path": resolved.display().to_string(),
        "file_name": resolved.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
        "success": true,
        "media_type": media_type,
        "size_bytes": metadata.as_ref().map(|m| m.len()),
        "modified_unix_ms": modified_unix_ms,
        "extracted_text": content.text,
        "visual_preview_count": content.visual_previews.len(),
        "visual_previews": content.visual_previews,
        "audio_preview_count": content.audio_previews.len(),
        "audio_previews": content.audio_previews,
        "file_attachment_count": content.file_attachments.len(),
        "file_attachments": content.file_attachments,
    })
}

#[cfg(test)]
mod tests {
    use super::{media_result, process_media_file};
    use crate::types::{MediaContent, ReadMediaArgs, ReadMode};
    use serde_json::json;

    fn args() -> ReadMediaArgs {
        ReadMediaArgs {
            paths: vec!["sample.txt".to_string()],
            include_text: true,
            max_text_chars: 40_000,
            max_visuals: 2,
            max_side: 256,
            max_files: 10,
            pdf_max_pages: 2,
            document_attachment_bytes: 1_000_000,
            audio_preview_bytes: 1_000_000,
        }
    }

    #[test]
    fn missing_media_path_is_an_error_instead_of_synthetic_success() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing.txt");

        let error = match process_media_file(&missing, &args(), ReadMode::Detailed) {
            Ok(_) => panic!("missing file should be reported as an error"),
            Err(error) => error,
        };

        assert!(
            error.contains("media path does not exist"),
            "unexpected missing file error: {error}"
        );
    }

    #[test]
    fn media_result_records_metadata_counts_and_payloads() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("sample.txt");
        std::fs::write(&file, "hello").expect("write sample");

        let value = media_result(
            "sample.txt",
            &file,
            MediaContent {
                text: "hello".to_string(),
                visual_previews: vec![json!({ "type": "image_url" })],
                audio_previews: vec![json!({ "type": "input_audio" })],
                file_attachments: vec![json!({ "type": "file" })],
            },
        );

        assert_eq!(value["success"], true);
        assert_eq!(value["path"], "sample.txt");
        assert_eq!(value["media_type"], "document");
        assert_eq!(value["size_bytes"], 5);
        assert_eq!(value["visual_preview_count"], 1);
        assert_eq!(value["audio_preview_count"], 1);
        assert_eq!(value["file_attachment_count"], 1);
        assert_eq!(value["extracted_text"], "hello");
    }
}
