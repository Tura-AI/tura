use super::config::{
    azure_speech_region, env_value, speech_provider_endpoint, speech_provider_key,
    speech_provider_model,
};
use super::files::{output_dir, relative_or_display, write_unique_download};
use super::types::{
    GenerateMediaArgs, SpeechBytes, SpeechProvider, SpeechTone, TextLanguage, VoiceRole,
};
use base64::{engine::general_purpose, Engine as _};
use reqwest::blocking::{Client, Response};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn dry_run_speech_payload(
    provider: SpeechProvider,
    args: &GenerateMediaArgs,
) -> Result<Value, String> {
    Ok(match provider {
        SpeechProvider::OpenAiTts => openai_payload(args),
        SpeechProvider::ElevenLabs => elevenlabs_payload(args),
        SpeechProvider::QwenDashScope => qwen_payload(args),
        SpeechProvider::AzureEdgeTts => azure_edge_payload(args)?,
        SpeechProvider::AzureSpeech => json!({ "ssml": azure_ssml(args)? }),
        SpeechProvider::ReplicateQwen3Tts => replicate_qwen_payload(args),
        SpeechProvider::ReplicateChatterbox => replicate_chatterbox_payload(args),
    })
}

pub(super) fn call_speech_provider(
    client: &Client,
    provider: SpeechProvider,
    args: &GenerateMediaArgs,
) -> Result<SpeechBytes, String> {
    match provider {
        SpeechProvider::OpenAiTts => call_openai(client, args),
        SpeechProvider::ElevenLabs => call_elevenlabs(client, args),
        SpeechProvider::QwenDashScope => call_qwen(client, args),
        SpeechProvider::AzureEdgeTts => call_azure_edge(client, args),
        SpeechProvider::AzureSpeech => call_azure(client, args),
        SpeechProvider::ReplicateQwen3Tts => call_replicate(
            client,
            provider,
            &speech_provider_endpoint(provider),
            replicate_qwen_payload(args),
        ),
        SpeechProvider::ReplicateChatterbox => call_replicate(
            client,
            provider,
            &speech_provider_endpoint(provider),
            replicate_chatterbox_payload(args),
        ),
    }
}

pub(super) fn write_speech(
    speech: &SpeechBytes,
    args: &GenerateMediaArgs,
    session_dir: &Path,
) -> Result<Value, String> {
    let dir = output_dir(args, session_dir);
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("failed to create output dir {}: {err}", dir.display()))?;
    let path = write_unique_download(
        &dir,
        "generate-media-speech",
        &speech.extension,
        &speech.bytes,
    )?;
    let metadata = std::fs::metadata(&path).ok();
    Ok(json!({
        "path": relative_or_display(&path, session_dir),
        "absolute_path": path.display().to_string(),
        "name": path.file_name().and_then(|value| value.to_str()).unwrap_or_default(),
        "file_type": "audio",
        "content_type": speech.mime_type,
        "size": metadata.map(|m| m.len()).unwrap_or(0),
    }))
}

fn call_openai(client: &Client, args: &GenerateMediaArgs) -> Result<SpeechBytes, String> {
    let key = speech_provider_key(SpeechProvider::OpenAiTts)?;
    let response = client
        .post(speech_provider_endpoint(SpeechProvider::OpenAiTts))
        .bearer_auth(key)
        .json(&openai_payload(args))
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("OpenAI TTS failed: {err}"))?;
    audio_from_response(response, "audio/mpeg", "mp3", json!({}))
}

fn call_elevenlabs(client: &Client, args: &GenerateMediaArgs) -> Result<SpeechBytes, String> {
    let key = speech_provider_key(SpeechProvider::ElevenLabs)?;
    let voice_id = elevenlabs_voice_id(role(args)?);
    let endpoint =
        speech_provider_endpoint(SpeechProvider::ElevenLabs).replace("{voice_id}", &voice_id);
    let response = client
        .post(endpoint)
        .header("xi-api-key", key)
        .json(&elevenlabs_payload(args))
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("ElevenLabs TTS failed: {err}"))?;
    audio_from_response(
        response,
        "audio/mpeg",
        "mp3",
        json!({ "voice_id": voice_id }),
    )
}

fn call_qwen(client: &Client, args: &GenerateMediaArgs) -> Result<SpeechBytes, String> {
    let key = speech_provider_key(SpeechProvider::QwenDashScope)?;
    let response = client
        .post(speech_provider_endpoint(SpeechProvider::QwenDashScope))
        .bearer_auth(key)
        .json(&qwen_payload(args))
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("Qwen DashScope TTS failed: {err}"))?;
    let value = response
        .json::<Value>()
        .map_err(|err| format!("failed to parse Qwen TTS response: {err}"))?;
    if let Some(url) = first_output_url(&value) {
        let response = client
            .get(&url)
            .send()
            .and_then(|reply| reply.error_for_status())
            .map_err(|err| format!("failed to download Qwen TTS output: {err}"))?;
        return audio_from_response(response, "audio/wav", "wav", value);
    }
    let encoded = find_base64_audio(&value)
        .ok_or_else(|| "Qwen TTS response did not include audio data or URL".to_string())?;
    let bytes = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|err| format!("invalid Qwen audio base64: {err}"))?;
    if bytes.is_empty() {
        return Err("Qwen TTS returned empty audio data".to_string());
    }
    Ok(SpeechBytes {
        bytes,
        mime_type: "audio/wav".to_string(),
        extension: "wav".to_string(),
        raw: value,
    })
}

fn call_azure_edge(client: &Client, args: &GenerateMediaArgs) -> Result<SpeechBytes, String> {
    let endpoint = speech_provider_endpoint(SpeechProvider::AzureEdgeTts);
    if !endpoint.is_empty() {
        let response = client
            .post(endpoint)
            .json(&azure_edge_payload(args)?)
            .send()
            .and_then(|reply| reply.error_for_status())
            .map_err(|err| format!("Microsoft Edge TTS failed: {err}"))?;
        return audio_from_response(response, "audio/mpeg", "mp3", json!({}));
    }
    call_azure_edge_cli(args)
}

fn call_azure_edge_cli(args: &GenerateMediaArgs) -> Result<SpeechBytes, String> {
    let command = edge_tts_command()?;
    let output_path = temp_audio_path("generate-media-edge-tts", "mp3");
    let tone = tone(args).unwrap_or(SpeechTone::Neutral);
    let (rate, _, volume) = azure_prosody(tone);
    let mut command_line = if command.ends_with(".py") {
        let mut cmd = Command::new("python");
        cmd.arg(command);
        cmd
    } else {
        Command::new(command)
    };
    command_line
        .arg("--text")
        .arg(&args.prompt)
        .arg("--voice")
        .arg(azure_voice(
            language(args).unwrap_or(TextLanguage::EnUs),
            role(args).unwrap_or(VoiceRole::FemaleGentle),
        ))
        .arg(format!("--rate={rate}"))
        .arg(format!("--volume={volume}"))
        .arg("--write-media")
        .arg(&output_path);
    tura_path::process_hardening::hide_child_console_window(&mut command_line);
    let output = command_line
        .output()
        .map_err(|err| format!("failed to start edge-tts: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "Microsoft Edge TTS failed with status {}{}",
            output.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        ));
    }
    let bytes = std::fs::read(&output_path).map_err(|err| {
        format!(
            "failed to read edge-tts output {}: {err}",
            output_path.display()
        )
    })?;
    let _ = std::fs::remove_file(&output_path);
    if bytes.is_empty() {
        return Err("Microsoft Edge TTS returned empty audio data".to_string());
    }
    Ok(SpeechBytes {
        bytes,
        mime_type: "audio/mpeg".to_string(),
        extension: "mp3".to_string(),
        raw: json!({
            "voice": azure_voice(
                language(args).unwrap_or(TextLanguage::EnUs),
                role(args).unwrap_or(VoiceRole::FemaleGentle)
            )
        }),
    })
}

fn edge_tts_command() -> Result<String, String> {
    if let Some(command) =
        env_value("TURA_GENERATE_MEDIA_EDGE_TTS_COMMAND").or_else(|| env_value("EDGE_TTS_COMMAND"))
    {
        return Ok(command);
    }
    for candidate in edge_tts_command_candidates() {
        if candidate.exists() {
            return Ok(candidate.display().to_string());
        }
    }
    Ok("edge-tts".to_string())
}

fn edge_tts_command_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join(".venv/Scripts/edge-tts.exe"));
        candidates.push(current_dir.join(".venv/bin/edge-tts"));
        candidates.push(current_dir.join("commands/generate_media/.venv/Scripts/edge-tts.exe"));
        candidates.push(current_dir.join("commands/generate_media/.venv/bin/edge-tts"));
    }
    candidates
}

fn temp_audio_path(prefix: &str, extension: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("{prefix}-{}-{now}.{extension}", std::process::id()))
}

fn call_azure(client: &Client, args: &GenerateMediaArgs) -> Result<SpeechBytes, String> {
    let key = speech_provider_key(SpeechProvider::AzureSpeech)?;
    let region = azure_speech_region()?;
    let token_endpoint = env_value("TURA_GENERATE_MEDIA_AZURE_SPEECH_TOKEN_ENDPOINT")
        .unwrap_or_else(|| {
            format!("https://{region}.api.cognitive.microsoft.com/sts/v1.0/issueToken")
        });
    let token = client
        .post(token_endpoint)
        .header("Ocp-Apim-Subscription-Key", &key)
        .body(String::new())
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("Azure Speech token failed: {err}"))?
        .text()
        .map_err(|err| format!("failed to read Azure Speech token: {err}"))?;
    let speech_endpoint =
        env_value("TURA_GENERATE_MEDIA_AZURE_SPEECH_ENDPOINT").unwrap_or_else(|| {
            format!("https://{region}.tts.speech.microsoft.com/cognitiveservices/v1")
        });
    let response = client
        .post(speech_endpoint)
        .bearer_auth(token)
        .header("Content-Type", "application/ssml+xml")
        .header(
            "X-Microsoft-OutputFormat",
            "audio-24khz-48kbitrate-mono-mp3",
        )
        .header("User-Agent", "Tura generate_media")
        .body(azure_ssml(args)?)
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("Azure Speech synthesis failed: {err}"))?;
    audio_from_response(response, "audio/mpeg", "mp3", json!({ "region": region }))
}

fn call_replicate(
    client: &Client,
    provider: SpeechProvider,
    endpoint: &str,
    payload: Value,
) -> Result<SpeechBytes, String> {
    let key = speech_provider_key(provider)?;
    let value = client
        .post(endpoint)
        .bearer_auth(key)
        .header("Prefer", "wait")
        .json(&payload)
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("{} failed: {err}", provider.display_name()))?
        .json::<Value>()
        .map_err(|err| format!("failed to parse Replicate TTS response: {err}"))?;
    let output = first_output_url(&value)
        .ok_or_else(|| "Replicate TTS response did not include audio output URL".to_string())?;
    let response = client
        .get(&output)
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("failed to download Replicate TTS output: {err}"))?;
    audio_from_response(response, "audio/wav", "wav", value)
}

fn openai_payload(args: &GenerateMediaArgs) -> Value {
    json!({
        "model": speech_provider_model(SpeechProvider::OpenAiTts),
        "input": args.prompt,
        "voice": openai_voice(role(args).unwrap_or(VoiceRole::FemaleGentle)),
        "response_format": "mp3",
        "instructions": speech_instruction(args),
    })
}

fn elevenlabs_payload(args: &GenerateMediaArgs) -> Value {
    json!({
        "text": args.prompt,
        "model_id": speech_provider_model(SpeechProvider::ElevenLabs),
        "voice_settings": elevenlabs_voice_settings(tone(args).unwrap_or(SpeechTone::Neutral)),
    })
}

fn qwen_payload(args: &GenerateMediaArgs) -> Value {
    json!({
        "model": speech_provider_model(SpeechProvider::QwenDashScope),
        "input": {
            "text": args.prompt,
        },
        "parameters": {
            "voice": qwen_voice(role(args).unwrap_or(VoiceRole::FemaleGentle)),
            "language_type": qwen_language(language(args).ok()),
            "instructions": speech_instruction(args),
            "optimize_instructions": true,
        },
    })
}

fn replicate_qwen_payload(args: &GenerateMediaArgs) -> Value {
    json!({
        "input": {
            "language": "auto",
            "mode": "custom_voice",
            "speaker": replicate_qwen_voice(role(args).unwrap_or(VoiceRole::FemaleGentle)),
            "voice_description": speech_instruction(args),
            "text": args.prompt,
        }
    })
}

fn replicate_chatterbox_payload(args: &GenerateMediaArgs) -> Value {
    json!({
        "input": {
            "prompt": args.prompt,
            "exaggeration": match tone(args).unwrap_or(SpeechTone::Neutral) {
                SpeechTone::Cheerful => 0.7,
                SpeechTone::Whisper | SpeechTone::Calm | SpeechTone::Sad => 0.35,
                _ => 0.5,
            },
            "cfg_weight": 0.5,
        }
    })
}

fn azure_edge_payload(args: &GenerateMediaArgs) -> Result<Value, String> {
    let tone = tone(args)?;
    let prosody = azure_prosody(tone);
    Ok(json!({
        "text": args.prompt,
        "voice": azure_voice(language(args)?, role(args)?),
        "rate": edge_percent_as_factor(prosody.0),
        "volume": edge_percent_as_factor(prosody.2),
    }))
}

fn azure_ssml(args: &GenerateMediaArgs) -> Result<String, String> {
    let text_language = language(args)?;
    let language = text_language.bcp47();
    let voice = azure_voice(text_language, role(args)?);
    let style = azure_style(tone(args)?);
    let prosody = azure_prosody(tone(args)?);
    Ok(format!(
        r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xmlns:mstts="http://www.w3.org/2001/mstts" xml:lang="{language}"><voice name="{voice}"><mstts:express-as style="{style}"><prosody rate="{rate}" pitch="{pitch}" volume="{volume}">{text}</prosody></mstts:express-as></voice></speak>"#,
        rate = prosody.0,
        pitch = prosody.1,
        volume = prosody.2,
        text = escape_xml(&args.prompt),
    ))
}

fn edge_percent_as_factor(value: &str) -> f32 {
    let percent = value
        .trim()
        .trim_end_matches('%')
        .parse::<f32>()
        .unwrap_or(0.0);
    1.0 + (percent / 100.0)
}

fn speech_instruction(args: &GenerateMediaArgs) -> String {
    let mut parts = vec![
        role_label(role(args).unwrap_or(VoiceRole::FemaleGentle)).to_string(),
        tone_label(tone(args).unwrap_or(SpeechTone::Neutral)).to_string(),
    ];
    if let Some(custom) = args.custom_voice_description.as_deref() {
        parts.push(custom.to_string());
    }
    if let Some(custom) = args.custom_tone_description.as_deref() {
        parts.push(custom.to_string());
    }
    parts.join("; ")
}

fn role(args: &GenerateMediaArgs) -> Result<VoiceRole, String> {
    args.voice_role
        .ok_or_else(|| "generate_media speech role is required".to_string())
}

fn tone(args: &GenerateMediaArgs) -> Result<SpeechTone, String> {
    args.speech_tone
        .ok_or_else(|| "generate_media speech tone is required".to_string())
}

fn language(args: &GenerateMediaArgs) -> Result<TextLanguage, String> {
    args.text_language
        .ok_or_else(|| "generate_media speech text_language is required".to_string())
}

fn role_label(role: VoiceRole) -> &'static str {
    match role {
        VoiceRole::FemaleGentle => "gentle adult female voice",
        VoiceRole::FemaleBright => "bright friendly adult female voice",
        VoiceRole::FemaleConfident => "confident professional adult female voice",
        VoiceRole::FemaleYoung => "young lively female voice",
        VoiceRole::MaleCalm => "calm adult male voice",
        VoiceRole::MaleWarm => "warm friendly adult male voice",
        VoiceRole::MaleDeep => "deep mature male voice",
        VoiceRole::MaleEnergetic => "energetic adult male voice",
    }
}

fn tone_label(tone: SpeechTone) -> &'static str {
    match tone {
        SpeechTone::Neutral => "neutral delivery",
        SpeechTone::Calm => "calm and relaxed delivery",
        SpeechTone::Cheerful => "cheerful and upbeat delivery",
        SpeechTone::Serious => "serious and steady delivery",
        SpeechTone::Sad => "sad and subdued delivery",
        SpeechTone::Whisper => "soft whisper-like delivery",
    }
}

fn openai_voice(role: VoiceRole) -> &'static str {
    match role {
        VoiceRole::FemaleGentle => "shimmer",
        VoiceRole::FemaleBright => "nova",
        VoiceRole::FemaleConfident => "alloy",
        VoiceRole::FemaleYoung => "nova",
        VoiceRole::MaleCalm => "echo",
        VoiceRole::MaleWarm => "fable",
        VoiceRole::MaleDeep => "onyx",
        VoiceRole::MaleEnergetic => "ash",
    }
}

fn elevenlabs_voice_id(role: VoiceRole) -> String {
    let env_key = format!(
        "TURA_GENERATE_MEDIA_ELEVENLABS_{}_VOICE_ID",
        role.id().to_ascii_uppercase()
    );
    env_value(&env_key).unwrap_or_else(|| {
        match role {
            VoiceRole::FemaleGentle => "21m00Tcm4TlvDq8ikWAM",
            VoiceRole::FemaleBright => "EXAVITQu4vr4xnSDxMaL",
            VoiceRole::FemaleConfident => "MF3mGyEYCl7XYWbV9V6O",
            VoiceRole::FemaleYoung => "AZnzlk1XvdvUeBnXmlld",
            VoiceRole::MaleCalm => "pNInz6obpgDQGcFmaJgB",
            VoiceRole::MaleWarm => "TxGEqnHWrfWFTfGW9XjX",
            VoiceRole::MaleDeep => "VR6AewLTigWG4xSOukaG",
            VoiceRole::MaleEnergetic => "ErXwobaYiN019PkySvjV",
        }
        .to_string()
    })
}

fn elevenlabs_voice_settings(tone: SpeechTone) -> Value {
    let (stability, similarity_boost, style) = match tone {
        SpeechTone::Cheerful => (0.35, 0.75, 0.55),
        SpeechTone::Calm | SpeechTone::Whisper | SpeechTone::Sad => (0.75, 0.75, 0.2),
        SpeechTone::Serious => (0.85, 0.7, 0.25),
        SpeechTone::Neutral => (0.55, 0.75, 0.35),
    };
    json!({
        "stability": stability,
        "similarity_boost": similarity_boost,
        "style": style,
        "use_speaker_boost": true,
    })
}

fn qwen_voice(role: VoiceRole) -> &'static str {
    match role {
        VoiceRole::FemaleGentle => "Serena",
        VoiceRole::FemaleBright => "Cherry",
        VoiceRole::FemaleConfident => "Vivian",
        VoiceRole::FemaleYoung => "Momo",
        VoiceRole::MaleCalm => "Eldric Sage",
        VoiceRole::MaleWarm => "Kai",
        VoiceRole::MaleDeep => "Vincent",
        VoiceRole::MaleEnergetic => "Ethan",
    }
}

fn replicate_qwen_voice(role: VoiceRole) -> &'static str {
    match role {
        VoiceRole::FemaleGentle | VoiceRole::FemaleBright | VoiceRole::FemaleYoung => "Serena",
        VoiceRole::FemaleConfident => "Vivian",
        VoiceRole::MaleCalm | VoiceRole::MaleWarm => "Uncle_fu",
        VoiceRole::MaleDeep => "Eric",
        VoiceRole::MaleEnergetic => "Aiden",
    }
}

fn qwen_language(language: Option<TextLanguage>) -> &'static str {
    match language {
        Some(TextLanguage::ZhCn) => "Chinese",
        Some(TextLanguage::EnUs) => "English",
        Some(TextLanguage::JaJp) => "Japanese",
        Some(TextLanguage::KoKr) => "Korean",
        Some(TextLanguage::EsEs) => "Spanish",
        Some(TextLanguage::FrFr) => "French",
        None => "Auto",
    }
}

fn azure_voice(language: TextLanguage, role: VoiceRole) -> &'static str {
    match (language, role) {
        (TextLanguage::ZhCn, VoiceRole::MaleCalm | VoiceRole::MaleWarm) => "zh-CN-YunxiNeural",
        (TextLanguage::ZhCn, VoiceRole::MaleDeep) => "zh-CN-YunjianNeural",
        (TextLanguage::ZhCn, VoiceRole::MaleEnergetic) => "zh-CN-YunyangNeural",
        (TextLanguage::ZhCn, VoiceRole::FemaleBright | VoiceRole::FemaleYoung) => {
            "zh-CN-XiaoyiNeural"
        }
        (TextLanguage::ZhCn, _) => "zh-CN-XiaoxiaoNeural",
        (
            TextLanguage::JaJp,
            VoiceRole::MaleCalm
            | VoiceRole::MaleWarm
            | VoiceRole::MaleDeep
            | VoiceRole::MaleEnergetic,
        ) => "ja-JP-KeitaNeural",
        (TextLanguage::JaJp, _) => "ja-JP-NanamiNeural",
        (
            TextLanguage::KoKr,
            VoiceRole::MaleCalm
            | VoiceRole::MaleWarm
            | VoiceRole::MaleDeep
            | VoiceRole::MaleEnergetic,
        ) => "ko-KR-InJoonNeural",
        (TextLanguage::KoKr, _) => "ko-KR-SunHiNeural",
        (
            TextLanguage::EsEs,
            VoiceRole::MaleCalm
            | VoiceRole::MaleWarm
            | VoiceRole::MaleDeep
            | VoiceRole::MaleEnergetic,
        ) => "es-ES-AlvaroNeural",
        (TextLanguage::EsEs, _) => "es-ES-ElviraNeural",
        (
            TextLanguage::FrFr,
            VoiceRole::MaleCalm
            | VoiceRole::MaleWarm
            | VoiceRole::MaleDeep
            | VoiceRole::MaleEnergetic,
        ) => "fr-FR-HenriNeural",
        (TextLanguage::FrFr, _) => "fr-FR-DeniseNeural",
        (
            TextLanguage::EnUs,
            VoiceRole::MaleCalm
            | VoiceRole::MaleWarm
            | VoiceRole::MaleDeep
            | VoiceRole::MaleEnergetic,
        ) => "en-US-GuyNeural",
        (TextLanguage::EnUs, _) => "en-US-AriaNeural",
    }
}

fn azure_style(tone: SpeechTone) -> &'static str {
    match tone {
        SpeechTone::Neutral => "chat",
        SpeechTone::Calm => "calm",
        SpeechTone::Cheerful => "cheerful",
        SpeechTone::Serious => "serious",
        SpeechTone::Sad => "sad",
        SpeechTone::Whisper => "whispering",
    }
}

fn azure_prosody(tone: SpeechTone) -> (&'static str, &'static str, &'static str) {
    match tone {
        SpeechTone::Neutral => ("+0%", "+0%", "+0%"),
        SpeechTone::Calm => ("-8%", "-4%", "-4%"),
        SpeechTone::Cheerful => ("+7%", "+8%", "+8%"),
        SpeechTone::Serious => ("-5%", "-8%", "+4%"),
        SpeechTone::Sad => ("-10%", "-10%", "-8%"),
        SpeechTone::Whisper => ("-15%", "-10%", "-25%"),
    }
}

fn audio_from_response(
    response: Response,
    fallback_mime: &str,
    fallback_extension: &str,
    raw: Value,
) -> Result<SpeechBytes, String> {
    let mime_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_mime.to_string());
    let bytes = response
        .bytes()
        .map_err(|err| format!("failed to read generated speech bytes: {err}"))?
        .to_vec();
    let extension = extension_for_mime(&mime_type)
        .unwrap_or(fallback_extension)
        .to_string();
    Ok(SpeechBytes {
        bytes,
        mime_type,
        extension,
        raw,
    })
}

fn first_output_url(value: &Value) -> Option<String> {
    let output = value.get("output").unwrap_or(value);
    match output {
        Value::String(url) if url.starts_with("http") => Some(url.clone()),
        Value::Array(items) => items.iter().find_map(|item| {
            item.as_str()
                .filter(|url| url.starts_with("http"))
                .map(str::to_string)
        }),
        Value::Object(map) => ["url", "audio", "output"]
            .iter()
            .find_map(|key| map.get(*key).and_then(first_output_url)),
        _ => None,
    }
}

fn find_base64_audio(value: &Value) -> Option<&str> {
    match value {
        Value::Object(map) => {
            for key in ["audio", "data", "b64_json", "content"] {
                if let Some(text) = map
                    .get(key)
                    .and_then(Value::as_str)
                    .filter(|text| !text.is_empty())
                {
                    return Some(text);
                }
            }
            map.values().find_map(find_base64_audio)
        }
        Value::Array(items) => items.iter().find_map(find_base64_audio),
        _ => None,
    }
}

fn extension_for_mime(mime_type: &str) -> Option<&'static str> {
    match mime_type.split(';').next().unwrap_or("").trim() {
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/wav" | "audio/wave" | "audio/x-wav" => Some("wav"),
        "audio/ogg" => Some("ogg"),
        "audio/webm" => Some("webm"),
        _ => None,
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
