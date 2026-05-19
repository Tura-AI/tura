use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use image::{
    codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, GenericImageView, ImageFormat,
};
use opencv::{core::Vector, imgcodecs, imgproc, prelude::*, videoio};
use pdfium_render::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct MediaProcessor {
    pub max_image_size: u32,
    pub max_video_frame_size: u32,
    pub video_frames: usize,
}

impl Default for MediaProcessor {
    fn default() -> Self {
        Self {
            max_image_size: 512,
            max_video_frame_size: 512,
            video_frames: 6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    #[serde(rename = "type")]
    pub attachment_type: String,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessAttachmentsResult {
    pub user_content: Vec<Value>,
    pub history_text_entry: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub user_content: Vec<Value>,
    pub history_text_entry: String,
}

impl MediaProcessor {
    pub fn new(max_image_size: u32, max_video_frame_size: u32, video_frames: usize) -> Self {
        Self {
            max_image_size,
            max_video_frame_size,
            video_frames,
        }
    }

    pub fn build_user_message(&self, user_text: impl Into<String>) -> UserMessage {
        let user_text = user_text.into();
        UserMessage {
            user_content: vec![json!({
                "type": "text",
                "text": user_text,
            })],
            history_text_entry: user_text,
        }
    }

    pub fn process_file_to_blocks(&self, file_path: impl AsRef<Path>) -> Result<Vec<Value>> {
        let file_path = file_path.as_ref();
        let ext = lower_ext(file_path);
        let file_name = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        match ext.as_str() {
            ".png" | ".jpg" | ".jpeg" | ".webp" | ".bmp" => {
                let maybe_b64 = self.compress_image(file_path, 80)?;
                Ok(match maybe_b64 {
                    Some(b64_data) => vec![
                        json!({ "type": "text", "text": format!("\n[Image Deliverable: {}]\n", file_name) }),
                        json!({
                            "type": "image_url",
                            "image_url": { "url": format!("data:image/jpeg;base64,{}", b64_data) }
                        }),
                    ],
                    None => vec![json!({
                        "type": "text",
                        "text": format!("\n[Image Error: Failed to process {}]\n", file_name)
                    })],
                })
            }
            ".mp4" | ".avi" | ".mov" | ".mkv" => {
                let frames = self.extract_video_frames(file_path, Some(self.video_frames), 80)?;
                let mut blocks = vec![];
                if !frames.is_empty() {
                    blocks.push(json!({
                        "type": "text",
                        "text": format!("\n[Video Deliverable: {} - Extracted {} frames]\n", file_name, frames.len())
                    }));
                    for frame_b64 in frames {
                        blocks.push(json!({
                            "type": "image_url",
                            "image_url": { "url": format!("data:image/jpeg;base64,{}", frame_b64) }
                        }));
                    }
                }
                if blocks.is_empty() {
                    blocks.push(json!({
                        "type": "text",
                        "text": format!("\n[Video Error: Failed to extract frames from {}]\n", file_name)
                    }));
                }
                Ok(blocks)
            }
            ".txt" | ".md" | ".py" | ".js" | ".ts" | ".html" | ".css" | ".json" | ".xml"
            | ".yaml" | ".yml" | ".csv" | ".sh" | ".log" | ".rs" | ".toml" => {
                let content = truncate_chars(
                    &fs::read_to_string(file_path).with_context(|| {
                        format!("failed to read text file: {}", file_path.display())
                    })?,
                    20_000,
                );
                Ok(vec![json!({
                    "type": "text",
                    "text": format!("\n### [Text Document: {}]\n```\n{}\n```\n", file_name, content)
                })])
            }
            ".docx" | ".xlsx" | ".pptx" | ".odt" | ".ods" | ".odp" => {
                let bytes = fs::read(file_path).with_context(|| {
                    format!("failed to read office document: {}", file_path.display())
                })?;
                let b64_data = general_purpose::STANDARD.encode(bytes);
                let mime_type = office_mime_type(&ext);

                Ok(vec![
                    json!({
                        "type": "text",
                        "text": format!("\n[Office Document Attached: {}]\n", file_name)
                    }),
                    json!({
                        "type": "document",
                        "source": {
                            "type": "base64",
                            "media_type": mime_type,
                            "data": b64_data,
                        }
                    }),
                ])
            }
            ".pdf" => {
                let (frames, pdf_text) = self.extract_pdf_content(file_path, 80)?;
                let mut blocks = vec![];

                if !pdf_text.trim().is_empty() {
                    blocks.push(json!({
                        "type": "text",
                        "text": format!(
                            "\n### [PDF Text Content: {}]\n```\n{}\n```\n",
                            file_name,
                            truncate_chars(pdf_text, 20_000)
                        )
                    }));
                }

                if !frames.is_empty() {
                    blocks.push(json!({
                        "type": "text",
                        "text": format!("\n[PDF Visuals: {} - Extracted {} pages]\n", file_name, frames.len())
                    }));
                    for frame_b64 in frames {
                        blocks.push(json!({
                            "type": "image_url",
                            "image_url": { "url": format!("data:image/jpeg;base64,{}", frame_b64) }
                        }));
                    }
                }

                if blocks.is_empty() {
                    blocks.push(json!({
                        "type": "text",
                        "text": format!("\n[PDF Error: Failed to extract text or visuals from {}]\n", file_name)
                    }));
                }

                Ok(blocks)
            }
            _ => Ok(vec![json!({
                "type": "text",
                "text": format!("\n[Unsupported/Unprocessed File Type: {}]\n", file_name)
            })]),
        }
    }

    pub fn compress_image(
        &self,
        file_path: impl AsRef<Path>,
        quality: u8,
    ) -> Result<Option<String>> {
        let file_path = file_path.as_ref();
        let bytes = fs::read(file_path)
            .with_context(|| format!("failed to read image: {}", file_path.display()))?;

        let img = match image::load_from_memory(&bytes) {
            Ok(img) => img,
            Err(_) => return Ok(None),
        };

        let img = resize_dynamic_image(img, self.max_image_size);
        let jpeg_bytes = encode_jpeg(&img, quality)?;
        Ok(Some(general_purpose::STANDARD.encode(jpeg_bytes)))
    }

    pub fn extract_pdf_pages_as_images(
        &self,
        file_path: impl AsRef<Path>,
        quality: u8,
    ) -> Result<Vec<String>> {
        let (frames, _) = self.extract_pdf_content(file_path, quality)?;
        Ok(frames)
    }

    pub fn extract_pdf_content(
        &self,
        file_path: impl AsRef<Path>,
        quality: u8,
    ) -> Result<(Vec<String>, String)> {
        let file_path = file_path.as_ref();
        let pdfium = bind_pdfium()?;
        let doc = pdfium
            .load_pdf_from_file(file_path, None)
            .with_context(|| format!("failed to open pdf: {}", file_path.display()))?;

        let mut frames_base64 = Vec::new();
        let mut extracted_text = String::new();

        for page in doc.pages().iter() {
            let text = page.text().all();
            if !text.is_empty() {
                extracted_text.push_str(&text);
                extracted_text.push('\n');
            }

            let rendered = page.render_with_config(
                &PdfRenderConfig::new()
                    .set_target_width(self.max_image_size as i32)
                    .render_form_data(true),
            )?;
            let bitmap = rendered.as_image();
            let img = DynamicImage::ImageRgb8(bitmap.to_rgb8());
            let img = resize_dynamic_image(img, self.max_image_size);
            let jpeg_bytes = encode_jpeg(&img, quality)?;
            frames_base64.push(general_purpose::STANDARD.encode(jpeg_bytes));
        }

        Ok((frames_base64, extracted_text))
    }

    pub fn extract_video_frames(
        &self,
        video_path: impl AsRef<Path>,
        max_frames: Option<usize>,
        quality: i32,
    ) -> Result<Vec<String>> {
        let video_path = video_path.as_ref();
        let max_frames = max_frames.unwrap_or(self.video_frames);
        if max_frames == 0 {
            return Ok(Vec::new());
        }

        let path_str = video_path.to_string_lossy();
        let mut cap = videoio::VideoCapture::from_file(&path_str, videoio::CAP_ANY)
            .with_context(|| format!("failed to open video: {}", video_path.display()))?;

        if !videoio::VideoCapture::is_opened(&cap)? {
            return Ok(Vec::new());
        }

        let total_frames = cap.get(videoio::CAP_PROP_FRAME_COUNT)? as usize;
        let total_frames = total_frames.max(1);
        let step = (total_frames / max_frames).max(1);

        let mut frames_base64 = Vec::new();
        let mut frame = Mat::default();
        let mut count = 0usize;

        loop {
            let ok = cap.read(&mut frame)?;
            if !ok || frame.empty() {
                break;
            }

            if count % step == 0 {
                let processed = resize_mat_keep_ratio(&frame, self.max_video_frame_size)?;
                let b64 = mat_to_jpeg_base64(&processed, quality)?;
                frames_base64.push(b64);
                if frames_base64.len() >= max_frames {
                    break;
                }
            }

            count += 1;
        }

        Ok(frames_base64)
    }

    pub async fn process_attachments(
        &self,
        attachments: &[Attachment],
        mut user_content: Vec<Value>,
        mut history_text_entry: String,
    ) -> Result<ProcessAttachmentsResult> {
        for att in attachments {
            let file_type = att.attachment_type.as_str();
            let Some(file_path) = att.path.as_ref().filter(|_| file_type != "sticker_info") else {
                continue;
            };

            let file_name = file_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");

            history_text_entry.push_str(&format!(" [Media: {}]", file_name));

            match file_type {
                "sticker_info" => {}
                "image" => self.handle_image(file_path, &mut user_content)?,
                "video" => self.handle_video(file_path, &mut user_content).await?,
                "voice_audio" | "audio_file" => {
                    self.handle_audio(file_path, file_type, &mut user_content)?
                }
                "document" => self.handle_document(file_path, &mut user_content)?,
                _ => {}
            }
        }

        Ok(ProcessAttachmentsResult {
            user_content,
            history_text_entry,
        })
    }

    fn handle_image(&self, file_path: &Path, user_content: &mut Vec<Value>) -> Result<()> {
        if let Some(b64_data) = self.compress_image(file_path, 80)? {
            user_content.push(json!({
                "type": "image_url",
                "image_url": { "url": format!("data:image/jpeg;base64,{}", b64_data) }
            }));
        }
        Ok(())
    }

    async fn handle_video(&self, file_path: &Path, user_content: &mut Vec<Value>) -> Result<()> {
        let frames = self.extract_video_frames(file_path, Some(8), 80)?;
        if !frames.is_empty() {
            user_content.push(json!({
                "type": "text",
                "text": format!("\n[Video Visuals: {} frames]\n", frames.len())
            }));
            for frame_b64 in frames {
                user_content.push(json!({
                    "type": "image_url",
                    "image_url": { "url": format!("data:image/jpeg;base64,{}", frame_b64) }
                }));
            }
        }

        let ffmpeg_bin = resolve_ffmpeg();
        if ffmpeg_bin.is_none() {
            user_content.push(json!({
                "type": "text",
                "text": "\n[Video Audio Extract Skipped: ffmpeg not found. Set FFMPEG_PATH or add ffmpeg to PATH.]"
            }));
            return Ok(());
        }

        let ffmpeg_bin = ffmpeg_bin.unwrap();
        let audio_extract_path = file_path.with_extension(format!(
            "{}.wav",
            file_path
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("tmp")
        ));

        let output = Command::new(&ffmpeg_bin)
            .arg("-i")
            .arg(file_path)
            .arg("-vn")
            .arg("-acodec")
            .arg("pcm_s16le")
            .arg("-ar")
            .arg("24000")
            .arg("-ac")
            .arg("1")
            .arg("-y")
            .arg(&audio_extract_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("failed to run ffmpeg")?;

        if !output.status.success() {
            user_content.push(json!({
                "type": "text",
                "text": "\n[Video Audio Extract Failed]"
            }));
            let _ = fs::remove_file(&audio_extract_path);
            return Ok(());
        }

        let audio_bytes = fs::read(&audio_extract_path).with_context(|| {
            format!(
                "failed to read extracted audio: {}",
                audio_extract_path.display()
            )
        })?;
        let b64_audio = general_purpose::STANDARD.encode(audio_bytes);
        user_content.push(json!({
            "type": "input_audio",
            "input_audio": { "data": b64_audio, "format": "wav" }
        }));
        user_content.push(json!({
            "type": "text",
            "text": "\n[Video Audio Track Attached]\n"
        }));

        let _ = fs::remove_file(&audio_extract_path);
        Ok(())
    }

    fn handle_audio(
        &self,
        file_path: &Path,
        file_type: &str,
        user_content: &mut Vec<Value>,
    ) -> Result<()> {
        let bytes = fs::read(file_path)
            .with_context(|| format!("failed to read audio: {}", file_path.display()))?;
        let b64_data = general_purpose::STANDARD.encode(bytes);
        let fmt = if file_type == "voice_audio" {
            "ogg"
        } else {
            "wav"
        };
        user_content.push(json!({
            "type": "input_audio",
            "input_audio": { "data": b64_data, "format": fmt }
        }));
        user_content.push(json!({ "type": "text", "text": "\n[Audio attached]" }));
        Ok(())
    }

    fn handle_document(&self, file_path: &Path, user_content: &mut Vec<Value>) -> Result<()> {
        let file_name = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        if lower_ext(file_path) == ".pdf" {
            let (frames, pdf_text) = self.extract_pdf_content(file_path, 80)?;

            if !pdf_text.trim().is_empty() {
                user_content.push(json!({
                    "type": "text",
                    "text": format!(
                        "\n\n--- PDF Text ({}) ---\n{}\n--- End ---\n",
                        file_name,
                        truncate_chars(pdf_text, 20_000)
                    )
                }));
            }

            if !frames.is_empty() {
                user_content.push(json!({
                    "type": "text",
                    "text": format!("\n[PDF Visuals: {} - {} pages]\n", file_name, frames.len())
                }));
                for frame_b64 in frames {
                    user_content.push(json!({
                        "type": "image_url",
                        "image_url": { "url": format!("data:image/jpeg;base64,{}", frame_b64) }
                    }));
                }
            } else if pdf_text.trim().is_empty() {
                user_content.push(json!({
                    "type": "text",
                    "text": format!("\n[PDF Error: Failed to render {}]", file_name)
                }));
            }

            return Ok(());
        }

        match fs::read_to_string(file_path) {
            Ok(doc_text) => {
                user_content.push(json!({
                    "type": "text",
                    "text": format!(
                        "\n\n--- Document ({}) ---\n{}\n--- End ---\n",
                        file_name,
                        truncate_chars(doc_text, 20_000)
                    )
                }));
            }
            Err(_) => {
                user_content.push(json!({
                    "type": "text",
                    "text": format!("\n[Doc Error: {}]", file_name)
                }));
            }
        }

        Ok(())
    }
}

fn lower_ext(path: &Path) -> String {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{}", s.to_ascii_lowercase()))
        .unwrap_or_default()
}

fn truncate_chars<S: Into<String>>(text: S, max_chars: usize) -> String {
    let text = text.into();
    let count = text.chars().count();
    if count <= max_chars {
        text
    } else {
        text.chars().take(max_chars).collect::<String>() + "\n...[Truncated]..."
    }
}

fn office_mime_type(ext: &str) -> &'static str {
    match ext {
        ".docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ".xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ".pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        ".odt" => "application/vnd.oasis.opendocument.text",
        ".ods" => "application/vnd.oasis.opendocument.spreadsheet",
        ".odp" => "application/vnd.oasis.opendocument.presentation",
        _ => "application/octet-stream",
    }
}

fn resize_dynamic_image(img: DynamicImage, max_side: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    if longest <= max_side || longest == 0 {
        img
    } else {
        let scale = max_side as f32 / longest as f32;
        let new_w = ((w as f32) * scale).round().max(1.0) as u32;
        let new_h = ((h as f32) * scale).round().max(1.0) as u32;
        img.resize(new_w, new_h, FilterType::Lanczos3)
    }
}

fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut out, quality);
    encoder.encode_image(img).context("failed to encode jpeg")?;
    Ok(out)
}

fn resize_mat_keep_ratio(src: &Mat, max_side: u32) -> Result<Mat> {
    let width = src.cols();
    let height = src.rows();
    let longest = width.max(height);

    if longest <= max_side as i32 || longest <= 0 {
        return Ok(src.try_clone()?);
    }

    let scale = max_side as f64 / longest as f64;
    let new_w = ((width as f64) * scale).round().max(1.0) as i32;
    let new_h = ((height as f64) * scale).round().max(1.0) as i32;

    let mut dst = Mat::default();
    imgproc::resize(
        src,
        &mut dst,
        opencv::core::Size::new(new_w, new_h),
        0.0,
        0.0,
        imgproc::INTER_AREA,
    )?;
    Ok(dst)
}

fn mat_to_jpeg_base64(mat: &Mat, quality: i32) -> Result<String> {
    let mut buf = Vector::<u8>::new();
    let mut params = Vector::<i32>::new();
    params.push(imgcodecs::IMWRITE_JPEG_QUALITY);
    params.push(quality);
    imgcodecs::imencode(".jpg", mat, &mut buf, &params)?;
    Ok(general_purpose::STANDARD.encode(buf.to_vec()))
}

fn bind_pdfium() -> Result<Pdfium> {
    let bindings = Pdfium::bind_to_system_library()
        .or_else(|_| Pdfium::bind_to_embedded_library())
        .context("failed to bind pdfium; install pdfium or enable embedded pdfium feature")?;
    Ok(Pdfium::new(bindings))
}

fn resolve_ffmpeg() -> Option<String> {
    if let Ok(path) = std::env::var("FFMPEG_PATH") {
        if !path.trim().is_empty() {
            return Some(path);
        }
    }
    Some("ffmpeg".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_chars() {
        let s = "a".repeat(10);
        assert_eq!(truncate_chars(s.clone(), 20), s);
        assert!(truncate_chars(s, 5).contains("[Truncated]"));
    }

    #[test]
    fn test_build_user_message() {
        let mp = MediaProcessor::default();
        let msg = mp.build_user_message("hello");
        assert_eq!(msg.history_text_entry, "hello");
        assert_eq!(msg.user_content.len(), 1);
    }
}
