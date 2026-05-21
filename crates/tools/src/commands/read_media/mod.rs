use crate::commands::CommandResponse;
use crate::runtime::file_locks::Access;
use crate::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload,
};
use base64::{engine::general_purpose, Engine as _};
use image::{
    codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, GenericImageView, Rgb, RgbImage,
};
use pdfium_render::prelude::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

pub const PROMPT: &str = include_str!("prompt.md");
pub const SCHEMA: &str = include_str!("schema.json");

const DEFAULT_MAX_TEXT_CHARS: usize = 40_000;
const DEFAULT_MAX_VISUALS: usize = 6;
const DEFAULT_MAX_SIDE: u32 = 512;
const MAX_VISUALS: usize = 60;
const MAX_DOCUMENT_ATTACHMENT_BYTES: u64 = 1_000_000;
const MAX_AUDIO_PREVIEW_BYTES: u64 = 1_000_000;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    max_files: usize,
}

struct MediaContent {
    text: String,
    visual_previews: Vec<Value>,
    audio_previews: Vec<Value>,
    file_attachments: Vec<Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadMode {
    Detailed,
    ThumbnailOnly,
}

fn parse_args_text(command_line: &str) -> Result<ReadMediaArgs, String> {
    let trimmed = command_line.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_str::<Value>(trimmed)
            .map_err(|err| format!("invalid read_media command_line JSON: {err}"))
            .and_then(parse_args_value);
    }
    parse_cli_args(trimmed)
}

fn parse_args_value(value: Value) -> Result<ReadMediaArgs, String> {
    if let Some(text) = value.as_str() {
        return parse_cli_args(text);
    }
    if value.is_array() {
        return args_from_parts(
            string_list(&value, &[]),
            true,
            DEFAULT_MAX_TEXT_CHARS,
            DEFAULT_MAX_VISUALS,
            DEFAULT_MAX_SIDE,
            20,
        );
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
    args_from_parts(
        string_list(
            &value,
            &["paths", "path", "files", "file", "media", "media_paths"],
        ),
        bool_value(&value, &["include_text", "includeText", "text"]).unwrap_or(true),
        u64_value(&value, &["max_text_chars", "maxTextChars"])
            .map(|value| value.clamp(1_000, 80_000) as usize)
            .unwrap_or(DEFAULT_MAX_TEXT_CHARS),
        u64_value(&value, &["max_visuals", "maxVisuals", "visuals"])
            .map(|value| value.min(MAX_VISUALS as u64) as usize)
            .unwrap_or(DEFAULT_MAX_VISUALS),
        u64_value(&value, &["max_side", "maxSide"])
            .map(|value| value.clamp(128, 1024) as u32)
            .unwrap_or(DEFAULT_MAX_SIDE),
        u64_value(
            &value,
            &[
                "max_files",
                "maxFiles",
                "max_directory_files",
                "maxDirectoryFiles",
            ],
        )
        .map(|value| value.clamp(1, 100) as usize)
        .unwrap_or(20),
    )
}

fn parse_cli_args(input: &str) -> Result<ReadMediaArgs, String> {
    let words = split_cli_words(input);
    let mut paths = Vec::new();
    let mut include_text = true;
    let mut max_text_chars = DEFAULT_MAX_TEXT_CHARS;
    let mut max_visuals = DEFAULT_MAX_VISUALS;
    let mut max_side = DEFAULT_MAX_SIDE;
    let mut max_files = 20usize;
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
                    .unwrap_or(DEFAULT_MAX_TEXT_CHARS)
                    .clamp(1_000, 80_000)
            }
            "--max-visuals" | "--max_visuals" => {
                max_visuals = take_value(&mut index)?
                    .parse::<usize>()
                    .unwrap_or(DEFAULT_MAX_VISUALS)
                    .min(MAX_VISUALS)
            }
            "--max-side" | "--max_side" => {
                max_side = take_value(&mut index)?
                    .parse::<u32>()
                    .unwrap_or(DEFAULT_MAX_SIDE)
                    .clamp(128, 1024)
            }
            "--max-files" | "--max_files" | "--max-directory-files" | "--max_directory_files" => {
                max_files = take_value(&mut index)?
                    .parse::<usize>()
                    .unwrap_or(20)
                    .clamp(1, 100)
            }
            _ if !word.starts_with('-') => paths.push(word.clone()),
            _ => return Err(format!("unsupported read_media option: {word}")),
        }
        index += 1;
    }

    args_from_parts(
        paths,
        include_text,
        max_text_chars,
        max_visuals,
        max_side,
        max_files,
    )
}

fn args_from_parts(
    paths: Vec<String>,
    include_text: bool,
    max_text_chars: usize,
    max_visuals: usize,
    max_side: u32,
    max_files: usize,
) -> Result<ReadMediaArgs, String> {
    if paths.is_empty() {
        return Err("read_media requires at least one path".to_string());
    }
    Ok(ReadMediaArgs {
        paths,
        include_text,
        max_text_chars,
        max_visuals,
        max_side,
        max_files,
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
    let expanded = expand_media_paths(&args, session_dir)?;
    let mode = if expanded.len() == 1 {
        ReadMode::Detailed
    } else {
        ReadMode::ThumbnailOnly
    };
    let (tx, rx) = mpsc::channel();
    let mut worker_count = 0usize;
    for (index, (path, resolved)) in expanded.into_iter().enumerate() {
        let args = args.clone();
        let tx = tx.clone();
        worker_count += 1;
        std::thread::spawn(move || {
            let item = match process_media_file(&resolved, &args, mode) {
                Ok(content) => media_result(&path, &resolved, content),
                Err(err) => json!({
                    "path": path,
                    "resolved_path": resolved.display().to_string(),
                    "success": false,
                    "error": err.to_string(),
                }),
            };
            let _ = tx.send((index, item));
        });
    }
    drop(tx);
    let mut indexed = Vec::new();
    for _ in 0..worker_count {
        match rx.recv() {
            Ok(item) => indexed.push(item),
            Err(_) => break,
        }
    }
    indexed.sort_by_key(|(index, _)| *index);
    let results = indexed
        .into_iter()
        .map(|(_, item)| item)
        .collect::<Vec<_>>();
    let mut output = json!({
        "media_results": results,
    });
    compact_visual_previews(&mut output)?;
    let summary = output
        .get("media_results")
        .and_then(Value::as_array)
        .map(|items| results_summary(items))
        .unwrap_or_default();
    output["summary_markdown"] = json!(summary);
    Ok(output)
}

fn compact_visual_previews(output: &mut Value) -> Result<(), String> {
    let Some(results) = output
        .get_mut("media_results")
        .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };
    for result in results.iter_mut() {
        compact_result_visual_previews(result)?;
    }
    let mut aggregate = Vec::new();
    for result in results.iter() {
        let path = result
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("media");
        let previews = result
            .get("visual_previews")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (index, preview) in previews.into_iter().enumerate() {
            if let Some(url) = preview_data_url(&preview) {
                let label = preview
                    .get("label")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| format!("#{:02}", index + 1));
                aggregate.push(SheetItem {
                    label: format!("#{:02} {}", aggregate.len() + 1, label),
                    detail: path.to_string(),
                    data_url: url.to_string(),
                });
            }
        }
    }
    if aggregate.len() <= 1 {
        return Ok(());
    }
    let sheets = contact_sheet_previews(&aggregate)?;
    for result in results.iter_mut() {
        if result
            .get("visual_previews")
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false)
        {
            result["visual_previews"] = json!([]);
            result["visual_preview_count"] = json!(0);
            result["visual_previews_compacted_into"] = json!("top_level_contact_sheet");
        }
    }
    output["visual_preview_count"] = json!(sheets.len());
    output["visual_previews"] = json!(sheets);
    output["visual_contact_sheet"] = json!(true);
    Ok(())
}

fn compact_result_visual_previews(result: &mut Value) -> Result<(), String> {
    let Some(previews) = result
        .get("visual_previews")
        .and_then(Value::as_array)
        .cloned()
    else {
        return Ok(());
    };
    if previews.len() <= 1 {
        return Ok(());
    }
    let path = result
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("media");
    let items = previews
        .iter()
        .enumerate()
        .filter_map(|(index, preview)| {
            let data_url = preview_data_url(preview)?;
            let label = preview
                .get("label")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("#{:02}", index + 1));
            Some(SheetItem {
                label: format!("#{:02} {}", index + 1, label),
                detail: path.to_string(),
                data_url: data_url.to_string(),
            })
        })
        .collect::<Vec<_>>();
    if items.len() <= 1 {
        return Ok(());
    }
    let sheets = contact_sheet_previews(&items)?;
    result["visual_previews"] = json!(sheets);
    result["visual_preview_count"] = json!(result["visual_previews"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0));
    result["visual_contact_sheet"] = json!(true);
    Ok(())
}

struct SheetItem {
    label: String,
    detail: String,
    data_url: String,
}

fn contact_sheet_previews(items: &[SheetItem]) -> Result<Vec<Value>, String> {
    let mut sheets = Vec::new();
    for chunk in items.chunks(12) {
        let sheet = render_contact_sheet(chunk)?;
        let encoded = encode_preview_jpeg(sheet, 1024, 76)?;
        sheets.push(json!({
            "type": "image_url",
            "label": "contact_sheet",
            "contact_sheet": true,
            "item_count": chunk.len(),
            "items": chunk.iter().map(|item| json!({
                "label": item.label,
                "path": item.detail,
            })).collect::<Vec<_>>(),
            "image_url": { "url": format!("data:image/jpeg;base64,{}", encoded) }
        }));
    }
    Ok(sheets)
}

fn render_contact_sheet(items: &[SheetItem]) -> Result<DynamicImage, String> {
    let cols = if items.len() <= 4 { 2 } else { 4 };
    let tile_w = 240u32;
    let tile_h = 190u32;
    let rows = (items.len() + cols - 1) / cols;
    let mut sheet = RgbImage::from_pixel(
        tile_w * cols as u32,
        tile_h * rows as u32,
        Rgb([245, 245, 245]),
    );
    for (index, item) in items.iter().enumerate() {
        let image = image_from_data_url(&item.data_url)?;
        let thumb = resize_dynamic_image(image, 150);
        let x = (index % cols) as u32 * tile_w;
        let y = (index / cols) as u32 * tile_h;
        paste_rgb(
            &mut sheet,
            &thumb.to_rgb8(),
            x + (tile_w - thumb.width()) / 2,
            y + 8,
        );
        draw_text(
            &mut sheet,
            x + 8,
            y + 164,
            &item.label.to_ascii_uppercase(),
            Rgb([0, 0, 0]),
        );
    }
    Ok(DynamicImage::ImageRgb8(sheet))
}

fn preview_data_url(value: &Value) -> Option<&str> {
    value
        .get("image_url")
        .and_then(|image| image.get("url"))
        .and_then(Value::as_str)
}

fn image_from_data_url(data_url: &str) -> Result<DynamicImage, String> {
    let (_, encoded) = data_url
        .split_once(',')
        .ok_or_else(|| "invalid image data URL".to_string())?;
    let bytes = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|err| format!("invalid base64 image preview: {err}"))?;
    image::load_from_memory(&bytes).map_err(|err| format!("failed to decode preview image: {err}"))
}

fn paste_rgb(canvas: &mut RgbImage, image: &RgbImage, x: u32, y: u32) {
    for yy in 0..image.height() {
        for xx in 0..image.width() {
            let tx = x + xx;
            let ty = y + yy;
            if tx < canvas.width() && ty < canvas.height() {
                canvas.put_pixel(tx, ty, *image.get_pixel(xx, yy));
            }
        }
    }
}

fn draw_text(canvas: &mut RgbImage, x: u32, y: u32, text: &str, color: Rgb<u8>) {
    let mut cursor = x;
    for ch in text.chars().take(18) {
        draw_char(canvas, cursor, y, ch, color);
        cursor += 7;
    }
}

fn draw_char(canvas: &mut RgbImage, x: u32, y: u32, ch: char, color: Rgb<u8>) {
    let pattern = font_pattern(ch);
    for (row, bits) in pattern.iter().enumerate() {
        for (col, bit) in bits.chars().enumerate() {
            if bit == '1' {
                for dy in 0..2 {
                    for dx in 0..2 {
                        let px = x + col as u32 * 2 + dx;
                        let py = y + row as u32 * 2 + dy;
                        if px < canvas.width() && py < canvas.height() {
                            canvas.put_pixel(px, py, color);
                        }
                    }
                }
            }
        }
    }
}

fn font_pattern(ch: char) -> [&'static str; 7] {
    match ch {
        '0' => ["111", "101", "101", "101", "101", "101", "111"],
        '1' => ["010", "110", "010", "010", "010", "010", "111"],
        '2' => ["111", "001", "001", "111", "100", "100", "111"],
        '3' => ["111", "001", "001", "111", "001", "001", "111"],
        '4' => ["101", "101", "101", "111", "001", "001", "001"],
        '5' => ["111", "100", "100", "111", "001", "001", "111"],
        '6' => ["111", "100", "100", "111", "101", "101", "111"],
        '7' => ["111", "001", "001", "010", "010", "010", "010"],
        '8' => ["111", "101", "101", "111", "101", "101", "111"],
        '9' => ["111", "101", "101", "111", "001", "001", "111"],
        'A' => ["111", "101", "101", "111", "101", "101", "101"],
        'D' => ["110", "101", "101", "101", "101", "101", "110"],
        'F' => ["111", "100", "100", "110", "100", "100", "100"],
        'G' => ["111", "100", "100", "101", "101", "101", "111"],
        'I' => ["111", "010", "010", "010", "010", "010", "111"],
        'M' => ["101", "111", "111", "101", "101", "101", "101"],
        'P' => ["111", "101", "101", "111", "100", "100", "100"],
        'S' => ["111", "100", "100", "111", "001", "001", "111"],
        'T' => ["111", "010", "010", "010", "010", "010", "010"],
        'V' => ["101", "101", "101", "101", "101", "101", "010"],
        '#' => ["010", "111", "010", "010", "111", "010", "000"],
        ':' => ["000", "010", "010", "000", "010", "010", "000"],
        '-' => ["000", "000", "000", "111", "000", "000", "000"],
        ' ' => ["000", "000", "000", "000", "000", "000", "000"],
        _ => ["000", "000", "000", "000", "000", "000", "000"],
    }
}

fn expand_media_paths(
    args: &ReadMediaArgs,
    session_dir: &Path,
) -> Result<Vec<(String, PathBuf)>, String> {
    let mut expanded = Vec::new();
    for path in &args.paths {
        let resolved = resolve_media_path(path, session_dir);
        if resolved.is_dir() {
            let mut entries = std::fs::read_dir(&resolved)
                .map_err(|err| {
                    format!(
                        "failed to read media directory {}: {err}",
                        resolved.display()
                    )
                })?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_file())
                .collect::<Vec<_>>();
            entries.sort_by_key(|path| {
                std::cmp::Reverse(
                    std::fs::metadata(path)
                        .and_then(|metadata| metadata.modified())
                        .ok(),
                )
            });
            for file in entries.into_iter().take(args.max_files) {
                expanded.push((display_input_path(&file, session_dir), file));
            }
        } else {
            expanded.push((path.clone(), resolved));
        }
    }
    Ok(expanded)
}

fn display_input_path(path: &Path, session_dir: &Path) -> String {
    path.strip_prefix(session_dir)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn process_media_file(
    path: &Path,
    args: &ReadMediaArgs,
    mode: ReadMode,
) -> Result<MediaContent, String> {
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
            process_pdf_thumbnail(path, args).unwrap_or_else(|_| file_tile_preview(path, args))
        }
        "video" => {
            process_video_thumbnail(path, args).unwrap_or_else(|_| file_tile_preview(path, args))
        }
        _ => file_tile_preview(path, args),
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
                .set_target_width(args.max_side as i32)
                .render_form_data(true),
        )
        .map_err(|err| format!("failed to render pdf page: {err}"))?;
    let image = DynamicImage::ImageRgb8(rendered.as_image().to_rgb8());
    let encoded = encode_preview_jpeg(image, args.max_side, 80)?;
    Ok(vec![json!({
        "type": "image_url",
        "label": "P1",
        "image_url": { "url": format!("data:image/jpeg;base64,{}", encoded) }
    })])
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
    let status = std::process::Command::new(ffmpeg)
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
        .arg(&frame)
        .status()
        .map_err(|err| {
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
        .map(|metadata| format!("{} KB", (metadata.len() + 1023) / 1024))
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

fn media_result(path: &str, resolved: &Path, content: MediaContent) -> Value {
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

fn process_image(path: &Path, args: &ReadMediaArgs) -> Result<Vec<Value>, String> {
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
    for (page_index, page) in doc.pages().iter().enumerate() {
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

fn process_video(path: &Path, args: &ReadMediaArgs) -> Result<MediaContent, String> {
    let Some(ffmpeg) = resolve_ffmpeg() else {
        return process_video_with_python_cv2(path, args).map(|visual_previews| MediaContent {
            text: String::new(),
            visual_previews,
            audio_previews: Vec::new(),
            file_attachments: Vec::new(),
        });
    };
    let temp_dir = temp_work_dir("tura-read-media");
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
            .map(|visual_previews| MediaContent {
                text: String::new(),
                visual_previews,
                audio_previews: Vec::new(),
                file_attachments: Vec::new(),
            })
            .map_err(|cv_err| format!("ffmpeg failed with status {status}; {cv_err}"));
    }
    let mut previews = Vec::new();
    let mut entries = std::fs::read_dir(&temp_dir)
        .map_err(|err| format!("failed to read temp frames: {err}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for (index, frame) in entries.into_iter().take(args.max_visuals).enumerate() {
        let bytes = std::fs::read(&frame).map_err(|err| format!("failed to read frame: {err}"))?;
        previews.push(json!({
            "type": "image_url",
            "label": format!("T{}MS", index * 1000),
            "image_url": { "url": format!("data:image/jpeg;base64,{}", general_purpose::STANDARD.encode(bytes)) }
        }));
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
    let audio_previews = process_audio(path, args).unwrap_or_default();
    Ok(MediaContent {
        text: String::new(),
        visual_previews: previews,
        audio_previews,
        file_attachments: Vec::new(),
    })
}

fn process_audio(path: &Path, _args: &ReadMediaArgs) -> Result<Vec<Value>, String> {
    let Some(ffmpeg) = resolve_ffmpeg() else {
        return Err("audio extraction unavailable: install ffmpeg".to_string());
    };
    let temp_dir = temp_work_dir("tura-read-media-audio");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|err| format!("failed to create temp audio dir: {err}"))?;
    let output_path = temp_dir.join("preview.mp3");
    let output = std::process::Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(path)
        .arg("-vn")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg("-b:a")
        .arg("24k")
        .arg("-fs")
        .arg(MAX_AUDIO_PREVIEW_BYTES.to_string())
        .arg("-f")
        .arg("mp3")
        .arg("-y")
        .arg(&output_path)
        .output()
        .map_err(|err| {
            let _ = std::fs::remove_dir_all(&temp_dir);
            format!("failed to run ffmpeg for audio extraction: {err}")
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(format!("audio extraction failed: {stderr}"));
    }
    let bytes = std::fs::read(&output_path).map_err(|err| {
        let _ = std::fs::remove_dir_all(&temp_dir);
        format!("failed to read compressed audio preview: {err}")
    })?;
    let _ = std::fs::remove_dir_all(&temp_dir);
    if bytes.is_empty() {
        return Err("audio extraction produced no data".to_string());
    }
    let compressed_to_limit = bytes.len() as u64 >= MAX_AUDIO_PREVIEW_BYTES;
    Ok(vec![json!({
        "type": "audio_url",
        "audio_url": {
            "url": format!("data:audio/mpeg;base64,{}", general_purpose::STANDARD.encode(bytes)),
            "format": "mp3"
        },
        "compressed": true,
        "max_size_bytes": MAX_AUDIO_PREVIEW_BYTES,
        "truncated_to_max_size": compressed_to_limit,
        "note": if compressed_to_limit {
            "Audio preview was compressed and capped to 1000000 bytes."
        } else {
            "Audio preview was compressed."
        }
    })])
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
frame_no = 0
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
            msec = int(cap.get(cv2.CAP_PROP_POS_MSEC) or (frame_no * 1000))
            frames.append({"label": f"T{msec}MS", "data": base64.b64encode(encoded.tobytes()).decode("ascii")})
            frame_no += 1
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
    let frames: Vec<Value> = serde_json::from_str(stdout.trim())
        .map_err(|err| format!("python cv2 fallback returned invalid JSON: {err}"))?;
    if frames.is_empty() {
        return Err("video frame extraction produced no frames".to_string());
    }
    Ok(frames
        .into_iter()
        .map(|frame| {
            let label = frame.get("label").and_then(Value::as_str).unwrap_or("F");
            let data = frame
                .get("data")
                .and_then(Value::as_str)
                .unwrap_or_default();
            json!({
                "type": "image_url",
                "label": label,
                "image_url": { "url": format!("data:image/jpeg;base64,{data}") }
            })
        })
        .collect())
}

fn process_document(path: &Path, args: &ReadMediaArgs) -> Result<MediaContent, String> {
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
    if metadata.len() > MAX_DOCUMENT_ATTACHMENT_BYTES {
        return Ok(MediaContent {
            text: format!(
                "[File attachment omitted: {} is larger than the 1000000 byte attachment limit.]",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("document")
            ),
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
    find_on_path("ffmpeg")
        .map(|path| path.display().to_string())
        .or_else(resolve_imageio_ffmpeg)
}

fn resolve_imageio_ffmpeg() -> Option<String> {
    let output = std::process::Command::new("python")
        .arg("-c")
        .arg("import imageio_ffmpeg; print(imageio_ffmpeg.get_ffmpeg_exe())")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !path.is_empty() && Path::new(&path).exists() {
        Some(path)
    } else {
        None
    }
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

fn temp_work_dir(prefix: &str) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}-{counter}",
        std::process::id(),
        chrono_like_millis()
    ))
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
        let audio_count = result
            .get("audio_preview_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if result.get("success").and_then(Value::as_bool) == Some(true) {
            lines.push(format!(
                "- {path}: {media_type}, {preview_count} visual previews, {audio_count} audio previews"
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

fn extension_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn media_type_for_path(path: &Path) -> &'static str {
    match extension_lower(path).as_deref() {
        Some("png" | "jpg" | "jpeg" | "webp" | "bmp") => "image",
        Some("pdf") => "pdf",
        Some("mp4" | "avi" | "mov" | "mkv" | "webm") => "video",
        Some("mp3" | "wav" | "m4a" | "aac" | "flac" | "ogg" | "opus") => "audio",
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
