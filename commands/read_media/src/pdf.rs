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
                        .set_target_width(scaled_side(args.max_side, 2, 1) as i32)
                        .render_form_data(true),
                )
                .map_err(|err| format!("failed to render pdf page: {err}"))?;
            let bitmap = rendered.as_image();
            let image = DynamicImage::ImageRgb8(bitmap.to_rgb8());
            let encoded = encode_preview_jpeg(image, scaled_side(args.max_side, 2, 1), 80)?;
            previews.push(json!({
                "type": "image_url",
                "label": format!("P{}", page_index + 1),
                "image_url": { "url": format!("data:image/jpeg;base64,{}", encoded) }
            }));
        }
    }
    if args.include_text && text.trim().is_empty() {
        if let Ok(fallback) = fallback_pdf_text(path, args) {
            text = fallback;
        }
    }
    Ok((truncate_chars(&text, args.max_text_chars), previews))
}

fn scaled_side(value: u32, numerator: u32, denominator: u32) -> u32 {
    value.saturating_mul(numerator).div_ceil(denominator)
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
        let Some(end_relative) = find_pdf_literal_string_end(&raw[start..]) else {
            break;
        };
        let end = start + end_relative;
        let candidate = decode_pdf_literal_string(&raw[start..end]);
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

fn find_pdf_literal_string_end(text: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            ')' => return Some(index),
            _ => {}
        }
    }
    None
}

fn decode_pdf_literal_string(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }
        let Some(next) = chars.next() else {
            decoded.push('\\');
            break;
        };
        match next {
            '(' | ')' | '\\' => decoded.push(next),
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            't' => decoded.push('\t'),
            'b' => decoded.push('\u{0008}'),
            'f' => decoded.push('\u{000c}'),
            other => {
                decoded.push('\\');
                decoded.push(other);
            }
        }
    }
    decoded
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
    let mut command = std::process::Command::new("python");
    command
        .arg("-c")
        .arg(script)
        .arg(path)
        .arg(max_text_chars.to_string())
        .arg(max_pages.to_string());
    tura_path::process_hardening::hide_child_console_window(&mut command);
    let output = command
        .output()
        .map_err(|err| format!("python fitz PDF fallback failed to start: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("python fitz PDF fallback failed: {stderr}"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(truncate_chars(&stdout, max_text_chars))
}

#[cfg(test)]
mod tests {
    use super::{decode_pdf_literal_string, fallback_pdf_text, find_pdf_literal_string_end};
    use crate::types::ReadMediaArgs;

    fn args(include_text: bool, max_text_chars: usize) -> ReadMediaArgs {
        ReadMediaArgs {
            paths: vec!["document.pdf".to_string()],
            include_text,
            max_text_chars,
            max_visuals: 2,
            max_side: 320,
            max_files: 10,
            pdf_max_pages: 2,
            document_attachment_bytes: 1_000_000,
            audio_preview_bytes: 1_000_000,
        }
    }

    #[test]
    fn fallback_pdf_text_returns_empty_when_text_is_disabled() {
        let file = tempfile::NamedTempFile::new().expect("pdf file");
        std::fs::write(file.path(), b"(Visible text)").expect("write fake pdf");

        let text = fallback_pdf_text(file.path(), &args(false, 100)).expect("fallback text");

        assert_eq!(text, "");
    }

    #[test]
    fn fallback_pdf_text_extracts_plain_pdf_string_objects() {
        let file = tempfile::NamedTempFile::new().expect("pdf file");
        std::fs::write(
            file.path(),
            b"%PDF-1.4\n(Hello world) Tj\n(12345) Tj\n(Second line with letters) Tj\n",
        )
        .expect("write fake pdf");

        let text = fallback_pdf_text(file.path(), &args(true, 500)).expect("fallback text");

        assert!(text.contains("Hello world"));
        assert!(text.contains("Second line with letters"));
        assert!(!text.contains("12345\n"));
    }

    #[test]
    fn fallback_pdf_text_unescapes_parentheses_and_truncates_to_limit() {
        let file = tempfile::NamedTempFile::new().expect("pdf file");
        std::fs::write(
            file.path(),
            b"(Alpha \\(inside\\) Beta Gamma Delta Epsilon Zeta Eta Theta Iota Kappa)",
        )
        .expect("write fake pdf");

        let text = fallback_pdf_text(file.path(), &args(true, 30)).expect("fallback text");

        assert!(text.contains("Alpha"));
        assert!(text.contains("inside"));
        assert!(text.contains("...[read_media text truncated]..."));
        assert!(
            text.chars().count() > 30,
            "marker is added around retained text"
        );
    }

    #[test]
    fn pdf_literal_string_helpers_skip_escaped_closing_parentheses() {
        let raw = r"Alpha \(inside\) Beta)";
        let end = find_pdf_literal_string_end(raw).expect("literal end");

        assert_eq!(&raw[end..=end], ")");
        assert_eq!(
            decode_pdf_literal_string(&raw[..end]),
            "Alpha (inside) Beta"
        );
        assert_eq!(
            decode_pdf_literal_string(r"line\nnext\tindent"),
            "line\nnext\tindent"
        );
    }

    #[test]
    fn fallback_pdf_text_reports_missing_file_with_path_context() {
        let missing = std::env::temp_dir().join("missing-read-media-pdf-for-test.pdf");

        let error = fallback_pdf_text(&missing, &args(true, 100)).expect_err("missing file");

        assert!(error.contains("failed to read pdf"));
    }

    #[test]
    fn fallback_pdf_text_returns_explanatory_message_when_no_text_objects_exist() {
        let file = tempfile::NamedTempFile::new().expect("pdf file");
        std::fs::write(file.path(), b"%PDF-1.4\nstream\n123456\nendstream\n")
            .expect("write fake pdf");

        let text = fallback_pdf_text(file.path(), &args(true, 500)).expect("fallback text");

        assert!(text.contains("PDF text extraction unavailable"));
    }
}
