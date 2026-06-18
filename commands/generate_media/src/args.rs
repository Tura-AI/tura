use super::config::{
    configured_output_dir, configured_provider_order, configured_speech_output_dir,
    configured_speech_provider_order,
};
use super::types::{
    GenerateMediaArgs, ImageProvider, MediaKind, SpeechProvider, SpeechTone, TextLanguage,
    VoiceRole, DEFAULT_COUNT, DEFAULT_OUTPUT_FORMAT, DEFAULT_PROVIDER_ORDER, DEFAULT_QUALITY,
    DEFAULT_SPEECH_PROVIDER_ORDER, MAX_COUNT,
};
use serde_json::Value;

pub(super) fn parse_args_text(command_line: &str) -> Result<GenerateMediaArgs, String> {
    let trimmed = command_line.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_str::<Value>(trimmed)
            .map_err(|err| format!("invalid generate_media JSON: {err}"))
            .and_then(parse_args_value);
    }
    parse_cli_args(trimmed)
}

pub(super) fn parse_args_value(value: Value) -> Result<GenerateMediaArgs, String> {
    if let Some(text) = value.as_str() {
        return parse_cli_args(text);
    }
    if let Some(cli) = string_field(&value, &["cli", "command_line", "commandLine", "input"]) {
        return parse_cli_args(&cli);
    }
    let object = value
        .as_object()
        .ok_or_else(|| "generate_media input must be object or CLI text".to_string())?;
    let kind = string_field(
        &value,
        &["media_type", "mediaType", "kind", "type", "modality"],
    )
    .map(|value| parse_media_kind(&value))
    .transpose()?;
    let speechish = string_field(
        &value,
        &[
            "text_language",
            "textLanguage",
            "language",
            "voice_role",
            "voiceRole",
            "role",
            "tone",
            "speech_tone",
            "speechTone",
        ],
    )
    .is_some();
    let kind = kind.unwrap_or(if speechish {
        MediaKind::Speech
    } else {
        MediaKind::Image
    });
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
        kind: Some(kind),
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
        speech_provider_order: string_list_field(
            object,
            &[
                "speech_provider_order",
                "speechProviderOrder",
                "voice_provider_order",
                "voiceProviderOrder",
            ],
        ),
        text_language: string_field(&value, &["text_language", "textLanguage", "language"]),
        voice_role: string_field(&value, &["voice_role", "voiceRole", "role"]),
        speech_tone: string_field(&value, &["tone", "speech_tone", "speechTone"]),
        custom_tone_description: string_field(
            &value,
            &[
                "custom_tone_description",
                "customToneDescription",
                "custom_tone",
                "customTone",
            ],
        ),
        custom_voice_description: string_field(
            &value,
            &[
                "custom_voice_description",
                "customVoiceDescription",
                "custom_voice",
                "customVoice",
            ],
        ),
        dry_run: bool_field(&value, &["dry_run", "dryRun"]),
        extra_body: object
            .get("extra_body")
            .or_else(|| object.get("extraBody"))
            .cloned(),
    })
}

pub(super) fn parse_cli_args(input: &str) -> Result<GenerateMediaArgs, String> {
    let words = split_cli_words(input);
    let mut parts = ArgParts::default();
    let mut prompt_parts = Vec::new();
    let mut index = 0usize;
    while index < words.len() {
        let original_word = &words[index];
        if index == 0 && is_generate_media_command_name(original_word) {
            index += 1;
            continue;
        }
        if index == 0 {
            match parse_media_kind(original_word) {
                Ok(kind) => {
                    parts.kind = Some(kind);
                    index += 1;
                    continue;
                }
                Err(_) => {}
            }
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
            "--media-type" | "--media_type" | "--type" | "--kind" => {
                parts.kind = Some(parse_media_kind(&take_value(&mut index)?)?)
            }
            "--text" | "--input-text" | "--input_text" => parts.prompt = take_value(&mut index)?,
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
            "--speech-provider-order"
            | "--speech_provider_order"
            | "--voice-provider-order"
            | "--voice_provider_order" => {
                parts.speech_provider_order = split_csv(&take_value(&mut index)?)
            }
            "--language" | "--text-language" | "--text_language" => {
                parts.text_language = Some(take_value(&mut index)?)
            }
            "--role" | "--voice-role" | "--voice_role" => {
                parts.voice_role = Some(take_value(&mut index)?)
            }
            "--tone" | "--speech-tone" | "--speech_tone" => {
                parts.speech_tone = Some(take_value(&mut index)?)
            }
            "--custom-tone"
            | "--custom_tone"
            | "--custom-tone-description"
            | "--custom_tone_description" => {
                parts.custom_tone_description = Some(take_value(&mut index)?)
            }
            "--custom-voice"
            | "--custom_voice"
            | "--custom-voice-description"
            | "--custom_voice_description" => {
                parts.custom_voice_description = Some(take_value(&mut index)?)
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
    kind: Option<MediaKind>,
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
    speech_provider_order: Vec<String>,
    text_language: Option<String>,
    voice_role: Option<String>,
    speech_tone: Option<String>,
    custom_tone_description: Option<String>,
    custom_voice_description: Option<String>,
    dry_run: bool,
    extra_body: Option<Value>,
}

fn args_from_parts(parts: ArgParts) -> Result<GenerateMediaArgs, String> {
    let kind = parts.kind.unwrap_or(MediaKind::Image);
    let prompt = parts.prompt.trim().to_string();
    if prompt.is_empty() && kind == MediaKind::Image {
        return Err("generate_media image prompt cannot be empty".to_string());
    }
    if prompt.is_empty() && kind == MediaKind::Speech {
        return Err("generate_media speech text cannot be empty".to_string());
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
    let speech_provider_order = if parts.speech_provider_order.is_empty() {
        configured_speech_provider_order()
    } else {
        parse_speech_provider_order(&parts.speech_provider_order)?
    };
    let text_language = required_speech_enum(
        kind,
        parts.text_language.as_deref(),
        "text_language",
        parse_text_language,
    )?;
    let voice_role =
        required_speech_enum(kind, parts.voice_role.as_deref(), "role", parse_voice_role)?;
    let speech_tone = required_speech_enum(
        kind,
        parts.speech_tone.as_deref(),
        "tone",
        parse_speech_tone,
    )?;
    Ok(GenerateMediaArgs {
        kind,
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
            .unwrap_or_else(|| {
                if kind == MediaKind::Speech {
                    configured_speech_output_dir()
                } else {
                    configured_output_dir()
                }
            }),
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
        speech_provider_order,
        text_language,
        voice_role,
        speech_tone,
        custom_tone_description: clean_optional(parts.custom_tone_description),
        custom_voice_description: clean_optional(parts.custom_voice_description),
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
        other => Err(format!("unsupported generate_media provider: {other}")),
    }
}

pub(super) fn parse_speech_provider_order(
    values: &[String],
) -> Result<Vec<SpeechProvider>, String> {
    let mut out = Vec::new();
    for value in values {
        for item in split_csv(value) {
            let provider = parse_speech_provider(&item)?;
            if !out.contains(&provider) {
                out.push(provider);
            }
        }
    }
    if out.is_empty() {
        Ok(DEFAULT_SPEECH_PROVIDER_ORDER.to_vec())
    } else {
        Ok(out)
    }
}

pub(super) fn parse_speech_provider(value: &str) -> Result<SpeechProvider, String> {
    match normalize_token(value).as_str() {
        "openai" | "openai_tts" | "gpt_4o_mini_tts" => Ok(SpeechProvider::OpenAiTts),
        "elevenlabs" | "eleven_labs" => Ok(SpeechProvider::ElevenLabs),
        "qwen" | "dashscope" | "qwen_dashscope" | "qwen_tts" => Ok(SpeechProvider::QwenDashScope),
        "azure" | "azure_speech" | "azure_tts" => Ok(SpeechProvider::AzureSpeech),
        "azure_edge" | "azure_edge_tts" | "edge" | "edge_tts" | "microsoft_edge_tts"
        | "azure_speech_free" => Ok(SpeechProvider::AzureEdgeTts),
        "replicate_qwen" | "replicate_qwen3" | "replicate_qwen3_tts" => {
            Ok(SpeechProvider::ReplicateQwen3Tts)
        }
        "replicate_chatterbox" | "chatterbox" => Ok(SpeechProvider::ReplicateChatterbox),
        other => Err(format!(
            "unsupported generate_media speech provider: {other}"
        )),
    }
}

fn parse_media_kind(value: &str) -> Result<MediaKind, String> {
    match normalize_token(value).as_str() {
        "image" | "picture" | "visual" => Ok(MediaKind::Image),
        "speech" | "audio" | "voice" | "tts" => Ok(MediaKind::Speech),
        other => Err(format!("unsupported generate_media media_type: {other}")),
    }
}

fn parse_text_language(value: &str) -> Result<TextLanguage, String> {
    match normalize_token(value).as_str() {
        "zh" | "zh_cn" | "zh_hans" | "chinese" | "mandarin" => Ok(TextLanguage::ZhCn),
        "en" | "en_us" | "english" => Ok(TextLanguage::EnUs),
        "ja" | "ja_jp" | "jp" | "japanese" => Ok(TextLanguage::JaJp),
        "ko" | "ko_kr" | "kr" | "korean" => Ok(TextLanguage::KoKr),
        "es" | "es_es" | "spanish" => Ok(TextLanguage::EsEs),
        "fr" | "fr_fr" | "french" => Ok(TextLanguage::FrFr),
        other => Err(format!("unsupported generate_media text_language: {other}")),
    }
}

fn parse_voice_role(value: &str) -> Result<VoiceRole, String> {
    match normalize_token(value).as_str() {
        "female_gentle" => Ok(VoiceRole::FemaleGentle),
        "female_bright" => Ok(VoiceRole::FemaleBright),
        "female_confident" => Ok(VoiceRole::FemaleConfident),
        "female_young" => Ok(VoiceRole::FemaleYoung),
        "male_calm" => Ok(VoiceRole::MaleCalm),
        "male_warm" => Ok(VoiceRole::MaleWarm),
        "male_deep" => Ok(VoiceRole::MaleDeep),
        "male_energetic" => Ok(VoiceRole::MaleEnergetic),
        other => Err(format!("unsupported generate_media role: {other}")),
    }
}

fn parse_speech_tone(value: &str) -> Result<SpeechTone, String> {
    match normalize_token(value).as_str() {
        "neutral" => Ok(SpeechTone::Neutral),
        "calm" => Ok(SpeechTone::Calm),
        "cheerful" | "happy" => Ok(SpeechTone::Cheerful),
        "serious" => Ok(SpeechTone::Serious),
        "sad" => Ok(SpeechTone::Sad),
        "whisper" | "whispering" => Ok(SpeechTone::Whisper),
        other => Err(format!("unsupported generate_media tone: {other}")),
    }
}

fn required_speech_enum<T>(
    kind: MediaKind,
    value: Option<&str>,
    name: &str,
    parser: fn(&str) -> Result<T, String>,
) -> Result<Option<T>, String> {
    match (kind, value) {
        (MediaKind::Speech, Some(value)) => parser(value).map(Some),
        (MediaKind::Speech, None) => Err(format!("generate_media speech {name} is required")),
        (MediaKind::Image, Some(value)) => parser(value).map(Some),
        (MediaKind::Image, None) => Ok(None),
    }
}

fn normalize_token(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', '.', ' '], "_")
}

fn normalize_output_format(value: &str) -> Result<String, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "png" => Ok("png".to_string()),
        "jpg" | "jpeg" => Ok("jpeg".to_string()),
        "webp" => Ok("webp".to_string()),
        other => Err(format!("unsupported generate_media output format: {other}")),
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn is_generate_media_command_name(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().replace('-', "_").as_str(),
        "generate_media" | "image_gen" | "generate_image" | "text_to_image" | "t2i"
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
