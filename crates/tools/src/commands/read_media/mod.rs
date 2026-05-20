use crate::commands::CommandResponse;
use crate::runtime::file_locks::Access;
use crate::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload,
};
use base64::{engine::general_purpose, Engine as _};
use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, GenericImageView};
use pdfium_render::prelude::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const PROMPT: &str = include_str!("prompt.md");
pub const SCHEMA: &str = include_str!("schema.json");

const DEFAULT_MAX_TEXT_CHARS: usize = 40_000;
const DEFAULT_MAX_VISUALS: usize = 6;
const DEFAULT_MAX_SIDE: u32 = 512;

pub fn execute(command_line: &str, session_dir: &Path) -> CommandResponse {
    match run_read_media(parse_args_text(command_line), session_dir) {
        Ok(output) => CommandResponse {
            success: true,
            exit_code: 0,
            stdout: summary_text(&output),
            stderr: String::new(),
            output,
            changes: Vec::new(),
        },
        Err(err) => CommandResponse {
            success: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: err.clone(),
            output: json!({ "error": err }),
            changes: Vec::new(),
        },
    }
}

pub fn access(command_line: &str, session_dir: &Path) -> Access {
    let Ok(args) = parse_args_text(command_line) else {
        return Access::default();
    };
    let mut access = Access::default();
    access.read_paths = args
        .paths
        .iter()
        .filter_map(|path| workspace_relative_path(path, session_dir))
        .map(|path| path.display().to_string())
        .collect();
    access
}

pub struct ReadMediaHandler;

#[async_trait::async_trait]
impl ToolHandler for ReadMediaHandler {
    fn tool_name(&self) -> &'static str {
        "read_media"
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    async fn is_mutating(&self, _call: &ToolCall, _ctx: &ToolContext) -> bool {
        false
    }

    async fn access(&self, call: &ToolCall, ctx: &ToolContext) -> Access {
        match &call.payload {
            ToolPayload::Function { arguments } => access_for_value(arguments, &ctx.session_dir),
            ToolPayload::Freeform { input } => access(input, &ctx.session_dir),
        }
    }

    async fn handle(
        &self,
        call: ToolCall,
        ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError> {
        let args = match call.payload {
            ToolPayload::Function { arguments } => parse_args_value(arguments),
            ToolPayload::Freeform { input } => parse_args_text(&input),
        }
        .map_err(ToolError::RespondToModel)?;
        let output =
            run_read_media(Ok(args), &ctx.session_dir).map_err(ToolError::RespondToModel)?;
        Ok(FunctionToolOutput::from_value(output, Some(true)))
    }
}

#[derive(Clone, Debug)]
struct ReadMediaArgs {
    paths: Vec<String>,
    include_text: bool,
    max_text_chars: usize,
    max_visuals: usize,
    max_side: u32,
}

fn parse_args_text(command_line: &str) -> Result<ReadMediaArgs, String> {
    serde_json::from_str::<Value>(command_line.trim())
        .map_err(|err| format!("invalid read_media command_line JSON: {err}"))
        .and_then(parse_args_value)
}

fn parse_args_value(value: Value) -> Result<ReadMediaArgs, String> {
    let paths = value
        .get("paths")
        .and_then(Value::as_array)
        .ok_or_else(|| "read_media requires paths array".to_string())?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Err("read_media paths must not be empty".to_string());
    }
    let include_text = value
        .get("include_text")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let max_text_chars = value
        .get("max_text_chars")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(1_000, 80_000) as usize)
        .unwrap_or(DEFAULT_MAX_TEXT_CHARS);
    let max_visuals = value
        .get("max_visuals")
        .and_then(Value::as_u64)
        .map(|value| value.min(12) as usize)
        .unwrap_or(DEFAULT_MAX_VISUALS);
    let max_side = value
        .get("max_side")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(128, 1024) as u32)
        .unwrap_or(DEFAULT_MAX_SIDE);
    Ok(ReadMediaArgs {
        paths,
        include_text,
        max_text_chars,
        max_visuals,
        max_side,
    })
}

fn access_for_value(value: &Value, session_dir: &Path) -> Access {
    let Ok(args) = parse_args_value(value.clone()) else {
        return Access::default();
    };
    let mut access = Access::default();
    access.read_paths = args
        .paths
        .iter()
        .filter_map(|path| workspace_relative_path(path, session_dir))
        .map(|path| path.display().to_string())
        .collect();
    access
}

fn run_read_media(
    args: Result<ReadMediaArgs, String>,
    session_dir: &Path,
) -> Result<Value, String> {
    let args = args?;
    let mut results = Vec::new();
    for path in &args.paths {
        let resolved = resolve_media_path(path, session_dir);
        let item = match process_media_file(&resolved, &args) {
            Ok((text, visual_previews)) => media_result(path, &resolved, text, visual_previews),
            Err(err) => json!({
                "path": path,
                "resolved_path": resolved.display().to_string(),
                "success": false,
                "error": err.to_string(),
            }),
        };
        results.push(item);
    }
    Ok(json!({
        "media_results": results,
        "summary_markdown": results_summary(&results),
    }))
}

fn process_media_file(path: &Path, args: &ReadMediaArgs) -> Result<(String, Vec<Value>), String> {
    match media_type_for_path(path) {
        "image" => process_image(path, args).map(|preview| (String::new(), preview)),
        "pdf" => process_pdf(path, args),
        "video" => process_video(path, args).map(|preview| (String::new(), preview)),
        _ => process_text_like(path, args).map(|text| (text, Vec::new())),
    }
}

fn media_result(path: &str, resolved: &Path, text: String, visual_previews: Vec<Value>) -> Value {
    let metadata = std::fs::metadata(resolved).ok();
    let media_type = media_type_for_path(resolved);
    json!({
        "path": path,
        "resolved_path": resolved.display().to_string(),
        "success": true,
        "media_type": media_type,
        "size_bytes": metadata.map(|m| m.len()),
        "extracted_text": text,
        "visual_preview_count": visual_previews.len(),
        "visual_previews": visual_previews,
    })
}

fn process_image(path: &Path, args: &ReadMediaArgs) -> Result<Vec<Value>, String> {
    let bytes = std::fs::read(path).map_err(|err| format!("failed to read image: {err}"))?;
    let image =
        image::load_from_memory(&bytes).map_err(|err| format!("failed to decode image: {err}"))?;
    let encoded = encode_preview_jpeg(image, args.max_side, 80)?;
    Ok(vec![json!({
        "type": "image_url",
        "image_url": { "url": format!("data:image/jpeg;base64,{}", encoded) }
    })])
}

fn process_pdf(path: &Path, args: &ReadMediaArgs) -> Result<(String, Vec<Value>), String> {
    let bindings = match Pdfium::bind_to_system_library() {
        Ok(bindings) => bindings,
        Err(err) => {
            let fallback = fallback_pdf_text(path, args)?;
            if fallback.trim().is_empty() {
                return Err(format!("failed to bind pdfium: {err}"));
            }
            return Ok((fallback, Vec::new()));
        }
    };
    let pdfium = Pdfium::new(bindings);
    let doc = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|err| format!("failed to open pdf: {err}"))?;
    let mut text = String::new();
    let mut previews = Vec::new();
    for page in doc.pages().iter() {
        if args.include_text {
            let page_text = page
                .text()
                .map_err(|err| format!("failed to extract pdf text: {err}"))?
                .all();
            if !page_text.trim().is_empty() {
                text.push_str(&page_text);
                text.push('\n');
            }
        }
        if previews.len() < args.max_visuals {
            let rendered = page
                .render_with_config(
                    &PdfRenderConfig::new()
                        .set_target_width(args.max_side as i32)
                        .render_form_data(true),
                )
                .map_err(|err| format!("failed to render pdf page: {err}"))?;
            let bitmap = rendered.as_image();
            let image = DynamicImage::ImageRgb8(bitmap.to_rgb8());
            let encoded = encode_preview_jpeg(image, args.max_side, 80)?;
            previews.push(json!({
                "type": "image_url",
                "image_url": { "url": format!("data:image/jpeg;base64,{}", encoded) }
            }));
        }
    }
    Ok((truncate_chars(&text, args.max_text_chars), previews))
}

fn fallback_pdf_text(path: &Path, args: &ReadMediaArgs) -> Result<String, String> {
    if !args.include_text {
        return Ok(String::new());
    }
    if let Ok(text) = extract_pdf_text_with_python_fitz(path, args.max_text_chars) {
        if !text.trim().is_empty() {
            return Ok(text);
        }
    }
    let bytes = std::fs::read(path).map_err(|err| format!("failed to read pdf: {err}"))?;
    let raw = String::from_utf8_lossy(&bytes);
    let mut text = String::new();
    let mut index = 0usize;
    while let Some(start) = raw[index..].find('(') {
        let start = index + start + 1;
        let Some(end) = raw[start..].find(')') else {
            break;
        };
        let end = start + end;
        let candidate = raw[start..end].replace("\\)", ")").replace("\\(", "(");
        if candidate.chars().any(|ch| ch.is_alphabetic()) {
            text.push_str(&candidate);
            text.push('\n');
        }
        index = end + 1;
        if text.len() >= args.max_text_chars {
            break;
        }
    }
    if text.trim().is_empty() {
        text.push_str("[PDF text extraction unavailable: pdfium not installed and no plain text objects found]");
    }
    Ok(truncate_chars(&text, args.max_text_chars))
}

fn extract_pdf_text_with_python_fitz(path: &Path, max_text_chars: usize) -> Result<String, String> {
    let script = r#"
import fitz
import sys

path = sys.argv[1]
max_chars = int(sys.argv[2])
doc = fitz.open(path)
parts = []
for page in doc:
    text = page.get_text("text")
    if text:
        parts.append(text)
    if sum(len(part) for part in parts) >= max_chars:
        break
doc.close()
sys.stdout.write("\n".join(parts))
"#;
    let output = std::process::Command::new("python")
        .arg("-c")
        .arg(script)
        .arg(path)
        .arg(max_text_chars.to_string())
        .output()
        .map_err(|err| format!("python fitz PDF fallback failed to start: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("python fitz PDF fallback failed: {stderr}"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(truncate_chars(&stdout, max_text_chars))
}

fn process_video(path: &Path, args: &ReadMediaArgs) -> Result<Vec<Value>, String> {
    let Some(ffmpeg) = resolve_ffmpeg() else {
        return process_video_with_python_cv2(path, args);
    };
    let temp_dir = std::env::temp_dir().join(format!(
        "tura-read-media-{}-{}",
        std::process::id(),
        chrono_like_millis()
    ));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|err| format!("failed to create temp frame dir: {err}"))?;
    let pattern = temp_dir.join("frame_%03d.jpg");
    let fps_filter = format!("fps=1,scale='min({0},iw)':-2", args.max_side);
    let status = std::process::Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(path)
        .arg("-vf")
        .arg(fps_filter)
        .arg("-frames:v")
        .arg(args.max_visuals.to_string())
        .arg("-q:v")
        .arg("4")
        .arg("-y")
        .arg(&pattern)
        .status()
        .map_err(|err| {
            let _ = std::fs::remove_dir_all(&temp_dir);
            format!("failed to run ffmpeg: {err}")
        })?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return process_video_with_python_cv2(path, args)
            .map_err(|cv_err| format!("ffmpeg failed with status {status}; {cv_err}"));
    }
    let mut previews = Vec::new();
    let mut entries = std::fs::read_dir(&temp_dir)
        .map_err(|err| format!("failed to read temp frames: {err}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for frame in entries.into_iter().take(args.max_visuals) {
        let bytes = std::fs::read(&frame).map_err(|err| format!("failed to read frame: {err}"))?;
        previews.push(json!({
            "type": "image_url",
            "image_url": { "url": format!("data:image/jpeg;base64,{}", general_purpose::STANDARD.encode(bytes)) }
        }));
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(previews)
}

fn process_video_with_python_cv2(path: &Path, args: &ReadMediaArgs) -> Result<Vec<Value>, String> {
    let script = r#"
import base64, json, sys
import cv2

path = sys.argv[1]
max_frames = int(sys.argv[2])
max_side = int(sys.argv[3])
cap = cv2.VideoCapture(path)
if not cap.isOpened():
    raise SystemExit("failed to open video")
total = int(cap.get(cv2.CAP_PROP_FRAME_COUNT) or 0)
step = max(1, total // max(1, max_frames)) if total > 0 else 1
frames = []
index = 0
while len(frames) < max_frames:
    ok, frame = cap.read()
    if not ok:
        break
    if index % step == 0:
        h, w = frame.shape[:2]
        longest = max(w, h)
        if longest > max_side:
            scale = max_side / float(longest)
            frame = cv2.resize(frame, (max(1, int(w * scale)), max(1, int(h * scale))))
        ok, encoded = cv2.imencode(".jpg", frame, [int(cv2.IMWRITE_JPEG_QUALITY), 80])
        if ok:
            frames.append(base64.b64encode(encoded.tobytes()).decode("ascii"))
    index += 1
cap.release()
print(json.dumps(frames))
"#;
    let output = std::process::Command::new("python")
        .arg("-c")
        .arg(script)
        .arg(path)
        .arg(args.max_visuals.to_string())
        .arg(args.max_side.to_string())
        .output()
        .map_err(|err| {
            format!("ffmpeg not found and python cv2 fallback failed to start: {err}")
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "video frame extraction unavailable: install ffmpeg or python cv2; {stderr}"
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let frames: Vec<String> = serde_json::from_str(stdout.trim())
        .map_err(|err| format!("python cv2 fallback returned invalid JSON: {err}"))?;
    if frames.is_empty() {
        return Err("video frame extraction produced no frames".to_string());
    }
    Ok(frames
        .into_iter()
        .map(|frame| {
            json!({
                "type": "image_url",
                "image_url": { "url": format!("data:image/jpeg;base64,{frame}") }
            })
        })
        .collect())
}

fn process_text_like(path: &Path, args: &ReadMediaArgs) -> Result<String, String> {
    if !args.include_text {
        return Ok(String::new());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("unsupported media or non-UTF-8 document: {err}"))?;
    Ok(truncate_chars(&text, args.max_text_chars))
}

fn encode_preview_jpeg(image: DynamicImage, max_side: u32, quality: u8) -> Result<String, String> {
    let image = resize_dynamic_image(image, max_side);
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut bytes, quality);
    encoder
        .encode_image(&image)
        .map_err(|err| format!("failed to encode preview jpeg: {err}"))?;
    Ok(general_purpose::STANDARD.encode(bytes))
}

fn resize_dynamic_image(image: DynamicImage, max_side: u32) -> DynamicImage {
    let (width, height) = image.dimensions();
    let longest = width.max(height);
    if longest <= max_side || longest == 0 {
        return image;
    }
    let scale = max_side as f32 / longest as f32;
    let new_width = ((width as f32) * scale).round().max(1.0) as u32;
    let new_height = ((height as f32) * scale).round().max(1.0) as u32;
    image.resize(new_width, new_height, FilterType::Lanczos3)
}

fn resolve_ffmpeg() -> Option<String> {
    if let Ok(path) = std::env::var("FFMPEG_PATH") {
        if !path.trim().is_empty() && Path::new(&path).exists() {
            return Some(path);
        }
    }
    find_on_path("ffmpeg").map(|path| path.display().to_string())
}

fn find_on_path(exe: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(if cfg!(windows) {
            format!("{exe}.exe")
        } else {
            exe.to_string()
        });
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn chrono_like_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn results_summary(results: &[Value]) -> String {
    let mut lines = Vec::new();
    for result in results {
        let path = result.get("path").and_then(Value::as_str).unwrap_or("");
        let media_type = result
            .get("media_type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let preview_count = result
            .get("visual_preview_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if result.get("success").and_then(Value::as_bool) == Some(true) {
            lines.push(format!(
                "- {path}: {media_type}, {preview_count} visual previews"
            ));
        } else {
            let error = result
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("error");
            lines.push(format!("- {path}: failed: {error}"));
        }
    }
    lines.join("\n")
}

fn summary_text(value: &Value) -> String {
    value
        .get("summary_markdown")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn media_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "webp" | "bmp" => "image",
        "pdf" => "pdf",
        "mp4" | "avi" | "mov" | "mkv" => "video",
        _ => "document",
    }
}

fn resolve_media_path(path: &str, session_dir: &Path) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        session_dir.join(candidate)
    }
}

fn workspace_relative_path(path: &str, session_dir: &Path) -> Option<PathBuf> {
    let resolved = resolve_media_path(path, session_dir);
    resolved
        .strip_prefix(session_dir)
        .ok()
        .map(Path::to_path_buf)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let head = max_chars / 2;
    let tail = max_chars.saturating_sub(head);
    let start = text.chars().take(head).collect::<String>();
    let end = text
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}\n...[read_media text truncated]...\n{end}")
}
