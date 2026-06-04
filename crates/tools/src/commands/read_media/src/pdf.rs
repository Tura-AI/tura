use super::paths::truncate_chars;
use super::previews::encode_preview_jpeg;
use super::types::ReadMediaArgs;
use image::DynamicImage;
use pdfium_render::prelude::*;
use serde_json::{json, Value};
use std::path::Path;

pub(super) fn process_pdf(
    path: &Path,
    args: &ReadMediaArgs,
) -> Result<(String, Vec<Value>), String> {
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
    for (page_index, page) in doc.pages().iter().enumerate().take(args.pdf_max_pages) {
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
                "label": format!("P{}", page_index + 1),
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
    if let Ok(text) =
        extract_pdf_text_with_python_fitz(path, args.max_text_chars, args.pdf_max_pages)
    {
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

fn extract_pdf_text_with_python_fitz(
    path: &Path,
    max_text_chars: usize,
    max_pages: usize,
) -> Result<String, String> {
    let script = r#"
import fitz
import sys

path = sys.argv[1]
max_chars = int(sys.argv[2])
max_pages = int(sys.argv[3])
doc = fitz.open(path)
parts = []
for page_index, page in enumerate(doc):
    if page_index >= max_pages:
        break
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
        .arg(max_pages.to_string())
        .output()
        .map_err(|err| format!("python fitz PDF fallback failed to start: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("python fitz PDF fallback failed: {stderr}"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(truncate_chars(&stdout, max_text_chars))
}
