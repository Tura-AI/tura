use super::config;
use super::types::{ReadMediaArgs, MAX_VISUALS};
use serde_json::Value;

pub(super) fn parse_args_text(command_line: &str) -> Result<ReadMediaArgs, String> {
    let trimmed = command_line.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_str::<Value>(trimmed)
            .map_err(|err| format!("invalid read_media command_line JSON: {err}"))
            .and_then(parse_args_value);
    }
    parse_cli_args(trimmed)
}

pub(super) fn parse_args_value(value: Value) -> Result<ReadMediaArgs, String> {
    let policy = config::read_media_policy();
    if let Some(text) = value.as_str() {
        return parse_cli_args(text);
    }
    if value.is_array() {
        return args_from_parts(ReadMediaArgParts {
            paths: string_list(&value, &[]),
            include_text: true,
            max_text_chars: policy.max_text_chars,
            max_visuals: policy.max_visuals,
            max_side: policy.max_side,
            max_files: policy.max_files,
            pdf_max_pages: policy.pdf_default_pages,
            document_attachment_bytes: policy.document_attachment_bytes,
            audio_preview_bytes: policy.audio_preview_bytes,
        });
    }
    if let Some(cli) = string_value(
        &value,
        &[
            "cli",
            "command_line",
            "commandLine",
            "input",
            "args",
            "payload",
        ],
    ) {
        return parse_cli_args(&cli);
    }
    args_from_parts(ReadMediaArgParts {
        paths: string_list(
            &value,
            &["paths", "path", "files", "file", "media", "media_paths"],
        ),
        include_text: bool_value(&value, &["include_text", "includeText", "text"]).unwrap_or(true),
        max_text_chars: u64_value(&value, &["max_text_chars", "maxTextChars"])
            .map(|value| value.clamp(1_000, 80_000) as usize)
            .unwrap_or(policy.max_text_chars),
        max_visuals: u64_value(&value, &["max_visuals", "maxVisuals", "visuals"])
            .map(|value| value.min(MAX_VISUALS as u64) as usize)
            .unwrap_or(policy.max_visuals),
        max_side: u64_value(&value, &["max_side", "maxSide"])
            .map(|value| value.clamp(128, 1024) as u32)
            .unwrap_or(policy.max_side),
        max_files: u64_value(
            &value,
            &[
                "max_files",
                "maxFiles",
                "max_directory_files",
                "maxDirectoryFiles",
            ],
        )
        .map(|value| value.clamp(1, 100) as usize)
        .unwrap_or(policy.max_files),
        pdf_max_pages: u64_value(
            &value,
            &["pdf_max_pages", "pdfMaxPages", "pdf_pages", "pdfPages"],
        )
        .map(|value| value.clamp(1, 50) as usize)
        .unwrap_or(policy.pdf_default_pages),
        document_attachment_bytes: u64_value(
            &value,
            &[
                "document_attachment_bytes",
                "documentAttachmentBytes",
                "max_document_attachment_bytes",
                "maxDocumentAttachmentBytes",
            ],
        )
        .map(|value| value.clamp(100_000, 5_000_000))
        .unwrap_or(policy.document_attachment_bytes),
        audio_preview_bytes: u64_value(
            &value,
            &[
                "audio_preview_bytes",
                "audioPreviewBytes",
                "max_audio_preview_bytes",
                "maxAudioPreviewBytes",
            ],
        )
        .map(|value| value.clamp(100_000, 5_000_000))
        .unwrap_or(policy.audio_preview_bytes),
    })
}

fn parse_cli_args(input: &str) -> Result<ReadMediaArgs, String> {
    let policy = config::read_media_policy();
    let words = split_cli_words(input);
    let mut paths = Vec::new();
    let mut include_text = true;
    let mut max_text_chars = policy.max_text_chars;
    let mut max_visuals = policy.max_visuals;
    let mut max_side = policy.max_side;
    let mut max_files = policy.max_files;
    let mut pdf_max_pages = policy.pdf_default_pages;
    let mut document_attachment_bytes = policy.document_attachment_bytes;
    let mut audio_preview_bytes = policy.audio_preview_bytes;
    let mut index = 0usize;

    while index < words.len() {
        let original_word = &words[index];
        if index == 0 && is_read_media_command_name(original_word) {
            index += 1;
            continue;
        }
        let (word, inline_value) = split_cli_assignment(original_word);
        let take_value = |index: &mut usize| -> Result<String, String> {
            if let Some(value) = inline_value.as_ref() {
                return Ok(value.clone());
            }
            *index += 1;
            words
                .get(*index)
                .cloned()
                .ok_or_else(|| format!("{word} requires a value"))
        };
        match word.as_str() {
            "--path" | "--paths" | "-p" => paths.push(take_value(&mut index)?),
            "--include-text" | "--include_text" => include_text = true,
            "--no-text" | "--no-include-text" | "--no_include_text" => include_text = false,
            "--max-text-chars" | "--max_text_chars" => {
                max_text_chars = take_value(&mut index)?
                    .parse::<usize>()
                    .unwrap_or(policy.max_text_chars)
                    .clamp(1_000, 80_000)
            }
            "--max-visuals" | "--max_visuals" => {
                max_visuals = take_value(&mut index)?
                    .parse::<usize>()
                    .unwrap_or(policy.max_visuals)
                    .min(MAX_VISUALS)
            }
            "--max-side" | "--max_side" => {
                max_side = take_value(&mut index)?
                    .parse::<u32>()
                    .unwrap_or(policy.max_side)
                    .clamp(128, 1024)
            }
            "--max-files" | "--max_files" | "--max-directory-files" | "--max_directory_files" => {
                max_files = take_value(&mut index)?
                    .parse::<usize>()
                    .unwrap_or(policy.max_files)
                    .clamp(1, 100)
            }
            "--pdf-pages" | "--pdf_pages" | "--pdf-max-pages" | "--pdf_max_pages" => {
                pdf_max_pages = take_value(&mut index)?
                    .parse::<usize>()
                    .unwrap_or(policy.pdf_default_pages)
                    .clamp(1, 50)
            }
            "--document-attachment-bytes" | "--document_attachment_bytes" => {
                document_attachment_bytes = take_value(&mut index)?
                    .parse::<u64>()
                    .unwrap_or(policy.document_attachment_bytes)
                    .clamp(100_000, 5_000_000)
            }
            "--audio-preview-bytes" | "--audio_preview_bytes" => {
                audio_preview_bytes = take_value(&mut index)?
                    .parse::<u64>()
                    .unwrap_or(policy.audio_preview_bytes)
                    .clamp(100_000, 5_000_000)
            }
            _ if !word.starts_with('-') => paths.push(word.clone()),
            _ => return Err(format!("unsupported read_media option: {word}")),
        }
        index += 1;
    }

    args_from_parts(ReadMediaArgParts {
        paths,
        include_text,
        max_text_chars,
        max_visuals,
        max_side,
        max_files,
        pdf_max_pages,
        document_attachment_bytes,
        audio_preview_bytes,
    })
}

struct ReadMediaArgParts {
    paths: Vec<String>,
    include_text: bool,
    max_text_chars: usize,
    max_visuals: usize,
    max_side: u32,
    max_files: usize,
    pdf_max_pages: usize,
    document_attachment_bytes: u64,
    audio_preview_bytes: u64,
}

fn args_from_parts(parts: ReadMediaArgParts) -> Result<ReadMediaArgs, String> {
    if parts.paths.is_empty() {
        return Err("read_media requires at least one path".to_string());
    }
    Ok(ReadMediaArgs {
        paths: parts.paths,
        include_text: parts.include_text,
        max_text_chars: parts.max_text_chars,
        max_visuals: parts.max_visuals,
        max_side: parts.max_side,
        max_files: parts.max_files,
        pdf_max_pages: parts.pdf_max_pages,
        document_attachment_bytes: parts.document_attachment_bytes,
        audio_preview_bytes: parts.audio_preview_bytes,
    })
}

fn is_read_media_command_name(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "read_media" | "read-media" | "readmedia"
    )
}

fn split_cli_assignment(word: &str) -> (String, Option<String>) {
    if let Some((key, value)) = word.split_once('=') {
        if key.starts_with('-') {
            return (key.to_string(), Some(value.to_string()));
        }
    }
    (word.to_string(), None)
}

fn split_cli_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in input.chars() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(ch),
            (None, c) if c.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn string_value(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
}

fn string_list(value: &Value, keys: &[&str]) -> Vec<String> {
    let selected = if keys.is_empty() {
        Some(value)
    } else {
        keys.iter().find_map(|key| value.get(*key))
    };
    match selected {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToString::to_string)
            .collect(),
        Some(Value::String(text)) => text
            .trim()
            .is_empty()
            .then(Vec::new)
            .unwrap_or_else(|| vec![text.trim().to_string()]),
        _ => Vec::new(),
    }
}

fn bool_value(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|value| {
            value.as_bool().or_else(|| {
                let text = value.as_str()?.trim().to_ascii_lowercase();
                match text.as_str() {
                    "true" | "yes" | "y" | "1" | "on" => Some(true),
                    "false" | "no" | "n" | "0" | "off" => Some(false),
                    _ => None,
                }
            })
        })
    })
}

fn u64_value(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
    })
}
