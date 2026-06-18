use super::args::{parse_args_text, parse_args_value};
use super::types::{
    ImageProvider, SpeechProvider, DEFAULT_PROVIDER_ORDER, DEFAULT_SPEECH_PROVIDER_ORDER,
};
use serde_json::json;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn parses_cli_with_references_negative_prompt_and_dimensions() {
    let args = parse_args_text(
        "generate_media --prompt 'clean product photo' --negative-prompt blur --reference ref.png --width 1536 --height 1024 --provider gemini --n 2 --format webp",
    )
    .expect("parse cli");

    assert_eq!(args.prompt, "clean product photo");
    assert_eq!(args.negative_prompt.as_deref(), Some("blur"));
    assert_eq!(args.references, vec!["ref.png"]);
    assert_eq!(args.width, Some(1536));
    assert_eq!(args.height, Some(1024));
    assert_eq!(args.provider_order, vec![ImageProvider::Gemini31Flash]);
    assert_eq!(args.count, 2);
    assert_eq!(args.output_format, "webp");
}

#[test]
fn parses_json_provider_order_and_common_parameters() {
    let args = parse_args_value(json!({
        "prompt": "poster",
        "negative_prompt": "low quality",
        "references": ["a.png", "b.jpg"],
        "provider_order": "grok3, replicate, openai",
        "aspect_ratio": "16:9",
        "quality": "high",
        "seed": 42,
        "dry_run": true
    }))
    .expect("parse json");

    assert_eq!(args.provider_order[0], ImageProvider::Grok3);
    assert_eq!(args.provider_order[1], ImageProvider::ReplicateZImageTurbo);
    assert_eq!(args.provider_order[2], ImageProvider::ChatGptImage2);
    assert_eq!(args.aspect_ratio.as_deref(), Some("16:9"));
    assert_eq!(args.quality, "high");
    assert_eq!(args.seed, Some(42));
    assert!(args.dry_run);
}

#[test]
fn uses_configured_provider_order_and_output_dir_when_unspecified() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let previous_order = std::env::var("TURA_GENERATE_MEDIA_PROVIDER_ORDER").ok();
    let previous_dir = std::env::var("TURA_GENERATE_MEDIA_OUTPUT_DIRECTORY").ok();
    std::env::set_var("TURA_GENERATE_MEDIA_PROVIDER_ORDER", "gemini,grok3");
    std::env::set_var("TURA_GENERATE_MEDIA_OUTPUT_DIRECTORY", "configured/images");

    let args = parse_args_text("--prompt poster").expect("parse configured defaults");

    assert_eq!(
        args.provider_order,
        vec![ImageProvider::Gemini31Flash, ImageProvider::Grok3]
    );
    assert_eq!(args.output_dir, "configured/images");

    restore_env("TURA_GENERATE_MEDIA_PROVIDER_ORDER", previous_order);
    restore_env("TURA_GENERATE_MEDIA_OUTPUT_DIRECTORY", previous_dir);
}

#[test]
fn default_provider_order_starts_with_replicate_then_openai() {
    assert_eq!(
        DEFAULT_PROVIDER_ORDER,
        [
            ImageProvider::ReplicateZImageTurbo,
            ImageProvider::ChatGptImage2,
            ImageProvider::Gemini31Flash,
            ImageProvider::Grok3,
        ]
    );
}

#[test]
fn default_speech_provider_order_starts_with_qwen_and_keeps_full_fallback() {
    assert_eq!(
        DEFAULT_SPEECH_PROVIDER_ORDER,
        [
            SpeechProvider::QwenDashScope,
            SpeechProvider::AzureEdgeTts,
            SpeechProvider::ReplicateQwen3Tts,
            SpeechProvider::AzureSpeech,
            SpeechProvider::OpenAiTts,
            SpeechProvider::ElevenLabs,
            SpeechProvider::ReplicateChatterbox,
        ]
    );
}

#[test]
fn parses_azure_speech_and_legacy_free_alias_separately() {
    assert_eq!(
        super::args::parse_speech_provider("azure").expect("azure"),
        SpeechProvider::AzureSpeech
    );
    assert_eq!(
        super::args::parse_speech_provider("azure_speech_free").expect("legacy free"),
        SpeechProvider::AzureEdgeTts
    );
    assert_eq!(
        super::args::parse_speech_provider("azure_edge_tts").expect("edge"),
        SpeechProvider::AzureEdgeTts
    );
}

fn restore_env(key: &str, value: Option<String>) {
    match value {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
}
