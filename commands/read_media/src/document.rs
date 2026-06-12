use super::paths::{extension_lower, truncate_chars};
use super::types::{MediaContent, ReadMediaArgs};
use base64::{engine::general_purpose, Engine as _};
use serde_json::json;
use std::path::Path;

pub(super) fn process_document(path: &Path, args: &ReadMediaArgs) -> Result<MediaContent, String> {
    if args.include_text {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                return Ok(MediaContent {
                    text: truncate_chars(&text, args.max_text_chars),
                    visual_previews: Vec::new(),
                    audio_previews: Vec::new(),
                    file_attachments: Vec::new(),
                });
            }
            Err(err) if !is_likely_binary_document(path) => {
                return Ok(MediaContent {
                    text: format!(
                        "[Unsupported file omitted: {} could not be decoded as text and was not uploaded as an attachment: {err}]",
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("file")
                    ),
                    visual_previews: Vec::new(),
                    audio_previews: Vec::new(),
                    file_attachments: Vec::new(),
                });
            }
            Err(_) => {}
        }
    }
    if !is_likely_binary_document(path) {
        return Ok(MediaContent {
            text: String::new(),
            visual_previews: Vec::new(),
            audio_previews: Vec::new(),
            file_attachments: Vec::new(),
        });
    }
    let metadata = std::fs::metadata(path)
        .map_err(|err| format!("failed to read document metadata: {err}"))?;
    if metadata.len() > args.document_attachment_bytes {
        return Ok(MediaContent {
            text: String::new(),
            visual_previews: Vec::new(),
            audio_previews: Vec::new(),
            file_attachments: Vec::new(),
        });
    }
    let mime_type = mime_type_for_path(path);
    if mime_type == "application/octet-stream" {
        return Ok(MediaContent {
            text: format!(
                "[Unsupported file omitted: {} has an unknown MIME type and was not uploaded as an attachment.]",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("file")
            ),
            visual_previews: Vec::new(),
            audio_previews: Vec::new(),
            file_attachments: Vec::new(),
        });
    }
    let bytes = std::fs::read(path).map_err(|err| format!("failed to read document: {err}"))?;
    Ok(MediaContent {
        text: String::new(),
        visual_previews: Vec::new(),
        audio_previews: Vec::new(),
        file_attachments: vec![json!({
            "type": "file",
            "file_name": path.file_name().and_then(|name| name.to_str()).unwrap_or("document"),
            "mime_type": mime_type,
            "size_bytes": metadata.len(),
            "data_base64": general_purpose::STANDARD.encode(bytes),
        })],
    })
}

fn is_likely_binary_document(path: &Path) -> bool {
    matches!(
        extension_lower(path).as_deref(),
        Some(
            "doc"
                | "docx"
                | "xls"
                | "xlsx"
                | "ppt"
                | "pptx"
                | "odt"
                | "ods"
                | "odp"
                | "rtf"
                | "zip"
        )
    )
}

fn mime_type_for_path(path: &Path) -> &'static str {
    match extension_lower(path).as_deref() {
        Some("doc") => "application/msword",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xls") => "application/vnd.ms-excel",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("ppt") => "application/vnd.ms-powerpoint",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("odt") => "application/vnd.oasis.opendocument.text",
        Some("ods") => "application/vnd.oasis.opendocument.spreadsheet",
        Some("odp") => "application/vnd.oasis.opendocument.presentation",
        Some("rtf") => "application/rtf",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::{mime_type_for_path, process_document};
    use crate::types::ReadMediaArgs;
    use base64::{engine::general_purpose, Engine as _};

    fn args(include_text: bool, attachment_bytes: u64) -> ReadMediaArgs {
        ReadMediaArgs {
            paths: vec!["sample.txt".to_string()],
            include_text,
            max_text_chars: 12,
            max_visuals: 2,
            max_side: 256,
            max_files: 10,
            pdf_max_pages: 2,
            document_attachment_bytes: attachment_bytes,
            audio_preview_bytes: 1_000_000,
        }
    }

    #[test]
    fn text_document_is_read_and_truncated_when_requested() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "abcdefghijklmnopqrstuvwxyz").expect("write notes");

        let content = process_document(&file, &args(true, 1_000_000))
            .expect("text document should be processed");

        assert!(content.text.contains("[read_media text truncated]"));
        assert!(content.visual_previews.is_empty());
        assert!(content.file_attachments.is_empty());
    }

    #[test]
    fn binary_document_under_size_limit_is_uploaded_as_attachment() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("archive.zip");
        std::fs::write(&file, b"PK\x03\x04fixture").expect("write archive");

        let content = process_document(&file, &args(false, 1_000_000))
            .expect("zip document should be processed");

        assert_eq!(content.file_attachments.len(), 1);
        assert_eq!(content.file_attachments[0]["mime_type"], "application/zip");
        assert_eq!(
            content.file_attachments[0]["data_base64"],
            general_purpose::STANDARD.encode(b"PK\x03\x04fixture")
        );
    }

    #[test]
    fn oversized_binary_document_is_omitted_without_attachment() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("archive.zip");
        std::fs::write(&file, b"large enough").expect("write archive");

        let content = process_document(&file, &args(false, 1))
            .expect("oversized document should be safely omitted");

        assert!(content.text.is_empty());
        assert!(content.file_attachments.is_empty());
    }

    #[test]
    fn mime_type_mapping_covers_known_documents_and_unknowns() {
        assert_eq!(
            mime_type_for_path(std::path::Path::new("a.docx")),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        );
        assert_eq!(
            mime_type_for_path(std::path::Path::new("a.xlsx")),
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        );
        assert_eq!(
            mime_type_for_path(std::path::Path::new("a.pptx")),
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        );
        assert_eq!(
            mime_type_for_path(std::path::Path::new("a.bin")),
            "application/octet-stream"
        );
    }
}
