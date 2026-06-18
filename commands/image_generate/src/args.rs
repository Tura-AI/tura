use super::config::{configured_output_dir, configured_provider_order};
use super::types::{
    ImageGenerateArgs, ImageProvider, DEFAULT_COUNT, DEFAULT_OUTPUT_FORMAT, DEFAULT_PROVIDER_ORDER,
    DEFAULT_QUALITY, MAX_COUNT,
};
use serde_json::Value;

pub(super) fn parse_args_text(command_line: &str) -> Result<ImageGenerateArgs, String> {
    let trimmed = command_line.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_str::<Value>(trimmed)
            .map_err(|err| format!("invalid image_generate JSON: {err}"))
            .and_then(parse_args_value);
    }
    parse_cli_args(trimmed)
}

pub(super) fn parse_args_value(value: Value) -> Result<ImageGenerateArgs, String> {
    if let Some(text) = value.as_str() {
        return parse_cli_args(text);
    }
    if let Some(cli) = string_field(&value, &["cli", "command_line", "commandLine", "input"]) {
        return parse_cli_args(&cli);
    }
    let object = value
        .as_object()
        .ok_or_else(|| "image_generate input must be object or CLI text".to_string())?;
    let prompt = string_field(
        &value,
        &[
            "prompt",
            "positive_prompt",
            "positivePrompt",
            "text",
            "query",
        ],
    )
    .unwrap_or_default();
    let references = string_list_field(
        object,
        &[
            "references",
            "reference",
            "reference_images",
            "referenceImages",
            "images",
            "image",
            "refs",
        ],
    );
    args_from_parts(ArgParts {
        prompt,
        negative_prompt: string_field(&value, &["negative_prompt", "negativePrompt", "negative"]),
        references,
        output_dir: string_field(
            &value,
            &[
                "output_dir",
                "outputDir",
                "download_dir",
                "downloadDir",
                "out_dir",
                "outDir",
                "dir",
            ],
        ),
        width: u32_field(&value, &["width", "w"]),
        height: u32_field(&value, &["height", "h"]),
        size: string_field(&value, &["size", "resolution"]),
        aspect_ratio: string_field(&value, &["aspect_ratio", "aspectRatio", "ratio"]),
        quality: string_field(&value, &["quality"]),
        count: u64_field(&value, &["n", "count", "num_images", "numImages"]).map(|v| v as usize),
        seed: u64_field(&value, &["seed"]),
        output_format: string_field(&value, &["output_format", "outputFormat", "format"]),
        provider: string_field(&value, &["provider", "model_provider", "modelProvider"]),
        provider_order: string_list_field(
            object,
            &["provider_order", "providerOrder", "providers"],
        ),
        dry_run: bool_field(&value, &["dry_run", "dryRun"]),
        extra_body: object
            .get("extra_body")
            .or_else(|| object.get("extraBody"))
            .cloned(),
    })
}

pub(super) fn parse_cli_args(input: &str) -> Result<ImageGenerateArgs, String> {
    let words = split_cli_words(input);
    let mut parts = ArgParts::default();
    let mut prompt_parts = Vec::new();
    let mut index = 0usize;
    while index < words.len() {
        let original_word = &words[index];
        if index == 0 && is_image_generate_command_name(original_word) {
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
            "--prompt" | "--positive-prompt" | "--positive_prompt" | "-p" => {
                parts.prompt = take_value(&mut index)?
            }
            "--negative-prompt" | "--negative_prompt" | "--negative" => {
                parts.negative_prompt = Some(take_value(&mut index)?)
            }
            "--reference" | "--ref" | "--image" | "--input-image" | "--input_image" => {
                parts.references.push(take_value(&mut index)?)
            }
            "--output-dir" | "--output_dir" | "--download-dir" | "--download_dir" | "-o" => {
                parts.output_dir = Some(take_value(&mut index)?)
            }
            "--width" | "-w" => parts.width = take_value(&mut index)?.parse::<u32>().ok(),
            "--height" | "-h" => parts.height = take_value(&mut index)?.parse::<u32>().ok(),
            "--size" | "--resolution" => parts.size = Some(take_value(&mut index)?),
            "--aspect-ratio" | "--aspect_ratio" | "--ratio" => {
                parts.aspect_ratio = Some(take_value(&mut index)?)
            }
            "--quality" => parts.quality = Some(take_value(&mut index)?),
            "--n" | "--count" | "--num-images" | "--num_images" => {
                parts.count = take_value(&mut index)?.parse::<usize>().ok()
            }
            "--seed" => parts.seed = take_value(&mut index)?.parse::<u64>().ok(),
            "--format" | "--output-format" | "--output_format" => {
                parts.output_format = Some(take_value(&mut index)?)
            }
            "--provider" => parts.provider = Some(take_value(&mut index)?),
            "--provider-order" | "--provider_order" | "--providers" => {
                parts.provider_order = split_csv(&take_value(&mut index)?)
            }
            "--dry-run" | "--dry_run" => parts.dry_run = true,
            _ if !word.starts_with("--") => prompt_parts.push(word.clone()),
            _ => {
                if inline_value.is_none()
                    && words
                        .get(index + 1)
                        .is_some_and(|next| !next.starts_with('-'))
                {
                    index += 1;
                }
            }
        }
        index += 1;
    }
    if parts.prompt.trim().is_empty() {
        parts.prompt = prompt_parts.join(" ");
    }
    args_from_parts(parts)
}

#[derive(Default)]
struct ArgParts {
    prompt: String,
    negative_prompt: Option<String>,
    references: Vec<String>,
    output_dir: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    size: Option<String>,
    aspect_ratio: Option<String>,
    quality: Option<String>,
    count: Option<usize>,
    seed: Option<u64>,
    output_format: Option<String>,
    provider: Option<String>,
    provider_order: Vec<String>,
    dry_run: bool,
    extra_body: Option<Value>,
}

fn args_from_parts(parts: ArgParts) -> Result<ImageGenerateArgs, String> {
    let prompt = parts.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("image_generate prompt cannot be empty".to_string());
    }
    let output_format = normalize_output_format(
        parts
            .output_format
            .as_deref()
            .unwrap_or(DEFAULT_OUTPUT_FORMAT),
    )?;
    let provider = parts.provider.as_deref().map(parse_provider).transpose()?;
    let provider_order = if let Some(provider) = provider {
        vec![provider]
    } else if parts.provider_order.is_empty() {
        configured_provider_order()
    } else {
        parse_provider_order(&parts.provider_order)?
    };
    Ok(ImageGenerateArgs {
        prompt,
        negative_prompt: clean_optional(parts.negative_prompt),
        references: parts
            .references
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        output_dir: parts
            .output_dir
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(configured_output_dir),
        width: parts.width,
        height: parts.height,
        size: clean_optional(parts.size),
        aspect_ratio: clean_optional(parts.aspect_ratio),
        quality: parts
            .quality
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_QUALITY.to_string()),
        count: parts.count.unwrap_or(DEFAULT_COUNT).clamp(1, MAX_COUNT),
        seed: parts.seed,
        output_format,
        provider_order,
        dry_run: parts.dry_run,
        extra_body: parts.extra_body,
    })
}

pub(super) fn parse_provider_order(values: &[String]) -> Result<Vec<ImageProvider>, String> {
    let mut out = Vec::new();
    for value in values {
        for item in split_csv(value) {
            let provider = parse_provider(&item)?;
            if !out.contains(&provider) {
                out.push(provider);
            }
        }
    }
    if out.is_empty() {
        Ok(DEFAULT_PROVIDER_ORDER.to_vec())
    } else {
        Ok(out)
    }
}

pub(super) fn parse_provider(value: &str) -> Result<ImageProvider, String> {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', '.', ' '], "_")
        .as_str()
    {
        "openai" | "chatgpt" | "chatgpt_image" | "chatgpt_image_2" | "gpt_image_2" => {
            Ok(ImageProvider::ChatGptImage2)
        }
        "replicate" | "replicate_z_image" | "replicate_z_image_turbo" | "z_image_turbo" => {
            Ok(ImageProvider::ReplicateZImageTurbo)
        }
        "gemini" | "google" | "gemini_flash" | "gemini_3_1_flash" | "gemini_31_flash" => {
            Ok(ImageProvider::Gemini31Flash)
        }
        "grok" | "grok3" | "xai" | "x_ai" => Ok(ImageProvider::Grok3),
        other => Err(format!("unsupported image_generate provider: {other}")),
    }
}

fn normalize_output_format(value: &str) -> Result<String, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "png" => Ok("png".to_string()),
        "jpg" | "jpeg" => Ok("jpeg".to_string()),
        "webp" => Ok("webp".to_string()),
        other => Err(format!("unsupported image_generate output format: {other}")),
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn is_image_generate_command_name(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().replace('-', "_").as_str(),
        "image_generate" | "image_gen" | "generate_image" | "text_to_image" | "t2i"
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

pub(super) fn split_cli_words(input: &str) -> Vec<String> {
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

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn u64_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
    })
}

fn u32_field(value: &Value, keys: &[&str]) -> Option<u32> {
    u64_field(value, keys).and_then(|value| u32::try_from(value).ok())
}

fn bool_field(value: &Value, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        value.get(*key).is_some_and(|value| {
            value.as_bool().unwrap_or_else(|| {
                value
                    .as_str()
                    .map(|text| {
                        matches!(
                            text.trim().to_ascii_lowercase().as_str(),
                            "1" | "true" | "yes" | "on"
                        )
                    })
                    .unwrap_or(false)
            })
        })
    })
}

fn string_list_field(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Vec<String> {
    for key in keys {
        if let Some(value) = object.get(*key) {
            match value {
                Value::Array(items) => {
                    return items
                        .iter()
                        .filter_map(Value::as_str)
                        .flat_map(split_csv)
                        .collect()
                }
                Value::String(text) => return split_csv(text),
                _ => {}
            }
        }
    }
    Vec::new()
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}
