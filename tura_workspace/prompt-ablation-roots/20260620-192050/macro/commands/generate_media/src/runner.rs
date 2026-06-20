use super::files::{output_dir, write_images};
use super::output::summarize_output;
use super::providers::{call_provider, dry_run_payload};
use super::speech::{call_speech_provider, dry_run_speech_payload, write_speech};
use super::types::{GenerateMediaArgs, MediaKind};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

pub(super) fn run_generate_media(
    args: Result<GenerateMediaArgs, String>,
    session_dir: &Path,
) -> Result<Value, String> {
    let args = args?;
    let session_dir = session_dir.to_path_buf();
    std::thread::spawn(move || run_generate_media_inner(args, &session_dir))
        .join()
        .map_err(|_| "generate_media worker thread panicked".to_string())?
}

pub(super) fn run_generate_media_inner(
    args: GenerateMediaArgs,
    session_dir: &Path,
) -> Result<Value, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(100))
        .user_agent("Tura generate_media/1.0")
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()
        .map_err(|err| format!("failed to create media generation client: {err}"))?;
    if args.kind == MediaKind::Speech {
        return run_speech_generate_inner(&client, args, session_dir);
    }
    if args.dry_run {
        let providers = args
            .provider_order
            .iter()
            .map(|provider| {
                dry_run_payload(*provider, &args, session_dir).map(|payload| {
                    json!({
                        "provider": provider.id(),
                        "display_name": provider.display_name(),
                        "request": payload,
                    })
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(json!({
            "dry_run": true,
            "prompt": args.prompt,
            "provider_order": args.provider_order.iter().map(|p| p.id()).collect::<Vec<_>>(),
            "output_dir": output_dir(&args, session_dir).display().to_string(),
            "providers": providers,
            "summary_markdown": "generate_media dry run: request payloads prepared without calling providers",
        }));
    }

    let mut attempts = Vec::new();
    let mut errors = Vec::new();
    for provider in &args.provider_order {
        match call_provider(&client, *provider, &args, session_dir) {
            Ok(outcome) => {
                let images =
                    write_images(&outcome.images, &args, session_dir, outcome.provider.id())?;
                let model = outcome.model.clone();
                attempts.push(json!({
                    "provider": outcome.provider.id(),
                    "model": model,
                    "success": true,
                    "image_count": images.len(),
                }));
                let downloaded_files = images.clone();
                let mut output = json!({
                    "prompt": args.prompt,
                    "references": args.references,
                    "provider": outcome.provider.id(),
                    "provider_display_name": outcome.provider.display_name(),
                    "model": model,
                    "provider_order": args.provider_order.iter().map(|p| p.id()).collect::<Vec<_>>(),
                    "result_count": images.len(),
                    "images": images,
                    "downloaded_files": downloaded_files,
                    "attempts": attempts,
                    "raw_response": compact_raw_response(outcome.raw),
                });
                output["summary_markdown"] = Value::String(summarize_output(&output));
                return Ok(output);
            }
            Err(error) => {
                errors.push(format!("{}: {error}", provider.id()));
                attempts.push(json!({
                    "provider": provider.id(),
                    "success": false,
                    "error": error,
                }));
            }
        }
    }
    Err(format!(
        "all generate_media providers failed: {}",
        errors.join(" | ")
    ))
}

fn run_speech_generate_inner(
    client: &Client,
    args: GenerateMediaArgs,
    session_dir: &Path,
) -> Result<Value, String> {
    if args.dry_run {
        let providers = args
            .speech_provider_order
            .iter()
            .map(|provider| {
                dry_run_speech_payload(*provider, &args).map(|payload| {
                    json!({
                        "provider": provider.id(),
                        "display_name": provider.display_name(),
                        "request": payload,
                    })
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(json!({
            "dry_run": true,
            "media_type": "speech",
            "text": args.prompt,
            "text_language": args.text_language.map(|value| value.id()),
            "role": args.voice_role.map(|value| value.id()),
            "tone": args.speech_tone.map(|value| value.id()),
            "output_dir": output_dir(&args, session_dir).display().to_string(),
            "providers": providers,
            "summary_markdown": "generate_media speech dry run: request payloads prepared without calling providers",
        }));
    }

    let mut attempts = Vec::new();
    let mut errors = Vec::new();
    for provider in &args.speech_provider_order {
        match call_speech_provider(client, *provider, &args) {
            Ok(outcome) => {
                let file = write_speech(&outcome, &args, session_dir)?;
                attempts.push(json!({
                    "provider": provider.id(),
                    "success": true,
                    "bytes": file.get("size").and_then(Value::as_u64).unwrap_or(0),
                }));
                let mut output = json!({
                    "media_type": "speech",
                    "text": args.prompt,
                    "text_language": args.text_language.map(|value| value.id()),
                    "role": args.voice_role.map(|value| value.id()),
                    "tone": args.speech_tone.map(|value| value.id()),
                    "custom_tone_description": args.custom_tone_description,
                    "custom_voice_description": args.custom_voice_description,
                    "result_count": 1,
                    "audio": file,
                    "downloaded_files": [file],
                    "attempts": attempts,
                    "raw_response": compact_raw_response(outcome.raw),
                });
                output["summary_markdown"] = Value::String(summarize_output(&output));
                return Ok(output);
            }
            Err(error) => {
                errors.push(format!("{}: {error}", provider.id()));
                attempts.push(json!({
                    "provider": provider.id(),
                    "success": false,
                    "error": error,
                }));
            }
        }
    }
    Err(format!(
        "all generate_media speech providers failed: {}",
        errors.join(" | ")
    ))
}

fn compact_raw_response(mut value: Value) -> Value {
    strip_large_base64(&mut value);
    value
}

fn strip_large_base64(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for key in ["b64_json", "data"] {
                if let Some(Value::String(text)) = object.get_mut(key) {
                    if text.len() > 256 {
                        *text = format!("[base64 omitted: {} chars]", text.len());
                    }
                }
            }
            for child in object.values_mut() {
                strip_large_base64(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                strip_large_base64(child);
            }
        }
        _ => {}
    }
}
