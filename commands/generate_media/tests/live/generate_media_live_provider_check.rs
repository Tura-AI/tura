use std::path::PathBuf;

use image::{ImageBuffer, Rgba};
use tura_command_generate_media::execute;
use tura_llm_rust::TuraConfig;

fn truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn falsy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off"
    )
}

fn live_disabled() -> bool {
    std::env::var("TURA_LIVE_GENERATE_MEDIA")
        .ok()
        .is_some_and(|value| falsy(&value))
}

fn provider_requested(provider: &str) -> bool {
    let key = format!("TURA_LIVE_GENERATE_MEDIA_{}", provider.to_ascii_uppercase());
    std::env::var(key)
        .ok()
        .map(|value| truthy(&value))
        .unwrap_or(true)
}

fn provider_has_key(provider: &str) -> bool {
    if provider == "openai" && codex_auth_json_exists() {
        return true;
    }
    if provider_key_names(provider).is_empty() {
        return true;
    }
    provider_key_names(provider)
        .iter()
        .find_map(|key| {
            let value = config_value(key)?;
            if provider == "openai_tts" && looks_like_oauth_token(&value) {
                None
            } else {
                Some(value)
            }
        })
        .is_some_and(|value| !value.is_empty())
}

fn looks_like_oauth_token(value: &str) -> bool {
    value.starts_with("eyJ") && value.split('.').count() >= 2
}

fn config_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            TuraConfig::default()
                .get(key)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn codex_auth_json_exists() -> bool {
    let path = std::env::var_os("CODEX_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|path| path.join("auth.json"))
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(|home| PathBuf::from(home).join(".codex").join("auth.json"))
        });
    path.is_some_and(|path| path.exists())
}

fn provider_key_names(provider: &str) -> &'static [&'static str] {
    match provider {
        "openai" => &[
            "CODEX_OPENAI_OAUTH_TOKEN",
            "CODEX_OAUTH_TOKEN",
            "OPENAI_OAUTH_TOKEN",
            "CHATGPT_OAUTH_TOKEN",
            "OPENAI_OPENAPI_KEY",
            "OPENAI_API_KEY_OPENAPI",
            "OPENAI_API_KEY",
            "CHATGPT_API_KEY",
        ],
        "replicate" => &["REPLICATE_API_TOKEN", "REPLICATE_API_KEY"],
        "gemini" => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        "grok3" => &["XAI_API_KEY", "GROK_API_KEY"],
        "openai_tts" => &[
            "OPENAI_OPENAPI_KEY",
            "OPENAI_API_KEY_OPENAPI",
            "OPENAI_API_KEY",
            "CHATGPT_API_KEY",
        ],
        "elevenlabs" => &["ELEVENLABS_API_KEY", "XI_API_KEY"],
        "qwen_dashscope" => &["QWEN_API_KEY", "DASHSCOPE_API_KEY"],
        "azure_edge_tts" | "azure_speech_free" => &[],
        "azure_speech" => &["SPEECH_KEY", "AZURE_SPEECH_KEY"],
        "replicate_qwen3_tts" | "replicate_chatterbox" => {
            &["REPLICATE_API_TOKEN", "REPLICATE_API_KEY"]
        }
        _ => &[],
    }
}

fn session_dir(name: &str) -> PathBuf {
    let root = std::env::current_dir()
        .expect("current dir")
        .join("target")
        .join("generate-media-live")
        .join(name);
    std::fs::create_dir_all(&root).expect("create live session dir");
    root
}

fn should_run_provider(provider: &str) -> bool {
    if live_disabled() {
        eprintln!("skipping live generate_media {provider}; TURA_LIVE_GENERATE_MEDIA disables it");
        return false;
    }
    if !provider_requested(provider) {
        eprintln!(
            "skipping live generate_media {provider}; TURA_LIVE_GENERATE_MEDIA_{} is not enabled",
            provider.to_ascii_uppercase()
        );
        return false;
    }
    if !provider_has_key(provider) {
        eprintln!(
            "skipping live generate_media {provider}; missing one of {:?}",
            provider_key_names(provider)
        );
        return false;
    }
    true
}

fn should_run_speech_provider(provider: &str) -> bool {
    if live_disabled() {
        eprintln!(
            "skipping live generate_media speech {provider}; TURA_LIVE_GENERATE_MEDIA disables it"
        );
        return false;
    }
    let key = format!(
        "TURA_LIVE_GENERATE_MEDIA_SPEECH_{}",
        provider.to_ascii_uppercase()
    );
    if std::env::var(key)
        .ok()
        .map(|value| !truthy(&value))
        .unwrap_or(false)
    {
        eprintln!("skipping live generate_media speech {provider}; provider disabled by env");
        return false;
    }
    if provider == "azure_speech"
        && config_value("SPEECH_REGION")
            .or_else(|| config_value("AZURE_SPEECH_REGION"))
            .is_none()
    {
        eprintln!("skipping live generate_media speech {provider}; missing Azure Speech region");
        return false;
    }
    if provider == "azure_edge_tts" && !edge_tts_available() {
        eprintln!(
            "skipping live generate_media speech {provider}; edge-tts dependency is unavailable"
        );
        return false;
    }
    if !provider_has_key(provider) {
        eprintln!(
            "skipping live generate_media speech {provider}; missing one of {:?}",
            provider_key_names(provider)
        );
        return false;
    }
    true
}

fn edge_tts_available() -> bool {
    config_value("TURA_GENERATE_MEDIA_AZURE_EDGE_TTS_ENDPOINT")
        .or_else(|| config_value("TURA_GENERATE_MEDIA_EDGE_TTS_ENDPOINT"))
        .or_else(|| config_value("TURA_GENERATE_MEDIA_EDGE_TTS_COMMAND"))
        .or_else(|| config_value("EDGE_TTS_COMMAND"))
        .is_some()
        || std::env::current_dir().ok().is_some_and(|dir| {
            dir.join(".venv/Scripts/edge-tts.exe").exists()
                || dir.join(".venv/bin/edge-tts").exists()
                || dir
                    .join("commands/generate_media/.venv/Scripts/edge-tts.exe")
                    .exists()
                || dir
                    .join("commands/generate_media/.venv/bin/edge-tts")
                    .exists()
        })
        || command_available("edge-tts")
}

fn command_available(command: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|path| {
            let direct = path.join(command);
            direct.exists()
                || path.join(format!("{command}.exe")).exists()
                || path.join(format!("{command}.cmd")).exists()
        })
    })
}

fn assert_generated_image(response: &tura_command_generate_media::CommandResponse, dir: &PathBuf) {
    assert!(
        response.success,
        "live generate_media failed: {}",
        response.stderr
    );
    assert_eq!(response.output["result_count"], 1);
    let path = response.output["images"][0]["path"]
        .as_str()
        .expect("generated image path");
    let absolute = dir.join(path);
    assert!(
        absolute.exists(),
        "generated image missing: {}",
        absolute.display()
    );
    assert!(
        std::fs::metadata(&absolute).expect("metadata").len() > 100,
        "generated image too small: {}",
        absolute.display()
    );
}

fn assert_generated_audio(response: &tura_command_generate_media::CommandResponse, dir: &PathBuf) {
    assert!(
        response.success,
        "live generate_media speech failed: {}",
        response.stderr
    );
    assert_eq!(response.output["media_type"], "speech");
    assert_eq!(response.output["result_count"], 1);
    let path = response.output["audio"]["path"]
        .as_str()
        .expect("generated audio path");
    let absolute = dir.join(path);
    assert!(
        absolute.exists(),
        "generated audio missing: {}",
        absolute.display()
    );
    assert!(
        std::fs::metadata(&absolute).expect("metadata").len() > 100,
        "generated audio too small: {}",
        absolute.display()
    );
}

fn run_provider(provider: &str) {
    if !should_run_provider(provider) {
        return;
    }
    let dir = session_dir(provider);
    let command = format!(
        "--prompt \"minimal black ink icon of a cat, plain white background\" --negative-prompt \"text, watermark, blur\" --provider {provider} --width 1024 --height 1024 --quality low --n 1 --output-dir media/{provider} --format png"
    );
    let response = execute(&command, &dir, 180);
    assert_generated_image(&response, &dir);
}

fn run_speech_provider(provider: &str) {
    if !should_run_speech_provider(provider) {
        return;
    }
    let dir = session_dir(&format!("speech-{provider}"));
    let command = format!(
        r#"{{
            "media_type":"speech",
            "text":"This is a short live speech synthesis check for Tura.",
            "text_language":"en_us",
            "role":"female_gentle",
            "tone":"calm",
            "custom_tone_description":"clear and natural",
            "speech_provider_order":"{provider}",
            "output_dir":"media/speech/{provider}"
        }}"#
    );
    let response = execute(&command, &dir, 240);
    if !response.success && credential_unavailable(&response.stderr) {
        eprintln!(
            "skipping live generate_media speech {provider}; configured credentials were rejected: {}",
            response.stderr
        );
        return;
    }
    assert_generated_audio(&response, &dir);
}

fn credential_unavailable(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("401")
        || lower.contains("403")
        || lower.contains("permissiondenied")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
}

fn write_reference_image(dir: &PathBuf) -> PathBuf {
    let path = dir.join("reference.png");
    let image = ImageBuffer::from_fn(128, 128, |x, y| {
        if (x / 16 + y / 16) % 2 == 0 {
            Rgba([240u8, 80, 80, 255])
        } else {
            Rgba([40u8, 120, 220, 255])
        }
    });
    image.save(&path).expect("write reference image");
    path
}

fn first_reference_provider_with_key() -> Option<&'static str> {
    ["gemini", "grok3", "openai"]
        .into_iter()
        .find(|provider| should_run_provider(provider))
}

#[test]
fn live_generate_media_openai_chatgpt_image_2() {
    run_provider("openai");
}

#[test]
fn live_generate_media_replicate_z_image_turbo() {
    run_provider("replicate");
}

#[test]
fn live_generate_media_gemini_3_1_flash() {
    run_provider("gemini");
}

#[test]
fn live_generate_media_grok3() {
    run_provider("grok3");
}

#[test]
fn live_generate_media_default_order_falls_back_to_available_provider() {
    if live_disabled() {
        eprintln!(
            "skipping live generate_media default order; TURA_LIVE_GENERATE_MEDIA disables it"
        );
        return;
    }
    let dir = session_dir("default-order");
    let response = execute(
        "--prompt \"small colorful glass cube on a clean desk, product render\" --negative-prompt \"text, watermark, blur\" --width 1024 --height 1024 --quality low --n 1 --output-dir media/default --format png",
        &dir,
        240,
    );
    assert_generated_image(&response, &dir);
}

#[test]
fn live_generate_media_reference_image_smoke() {
    let Some(provider) = first_reference_provider_with_key() else {
        eprintln!(
            "skipping live generate_media reference smoke; no reference-capable provider key"
        );
        return;
    };
    let dir = session_dir("reference-smoke");
    let reference = write_reference_image(&dir);
    let command = format!(
        "--prompt \"simple square app icon inspired by the reference colors, no text\" --negative-prompt \"letters, watermark, blur\" --provider {provider} --reference \"{}\" --width 1024 --height 1024 --quality low --n 1 --output-dir media/reference --format png",
        reference.display()
    );
    let response = execute(&command, &dir, 180);
    assert_generated_image(&response, &dir);
    assert_eq!(
        response.output["references"][0],
        reference.display().to_string()
    );
}

#[test]
fn live_generate_media_speech_openai_tts() {
    run_speech_provider("openai_tts");
}

#[test]
fn live_generate_media_speech_elevenlabs() {
    run_speech_provider("elevenlabs");
}

#[test]
fn live_generate_media_speech_qwen_dashscope() {
    run_speech_provider("qwen_dashscope");
}

#[test]
fn live_generate_media_speech_azure_edge_tts() {
    run_speech_provider("azure_edge_tts");
}

#[test]
fn live_generate_media_speech_azure_speech() {
    run_speech_provider("azure_speech");
}

#[test]
fn live_generate_media_speech_replicate_qwen3_tts() {
    run_speech_provider("replicate_qwen3_tts");
}

#[test]
fn live_generate_media_speech_replicate_chatterbox() {
    run_speech_provider("replicate_chatterbox");
}
