use serde_json::Value;

pub(super) const DEFAULT_OUTPUT_DIR: &str = "media/image";
pub(super) const DEFAULT_SPEECH_OUTPUT_DIR: &str = "media/audio";
pub(super) const DEFAULT_OUTPUT_FORMAT: &str = "png";
pub(super) const DEFAULT_QUALITY: &str = "auto";
pub(super) const DEFAULT_SIZE: &str = "1024x1024";
pub(super) const DEFAULT_COUNT: usize = 1;
pub(super) const MAX_COUNT: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MediaKind {
    Image,
    Speech,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ImageProvider {
    ChatGptImage2,
    ReplicateZImageTurbo,
    Gemini31Flash,
    Grok3,
}

impl ImageProvider {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::ChatGptImage2 => "chatgpt_image_2",
            Self::ReplicateZImageTurbo => "replicate_z_image_turbo",
            Self::Gemini31Flash => "gemini_3_1_flash",
            Self::Grok3 => "grok3",
        }
    }

    pub(super) fn display_name(self) -> &'static str {
        match self {
            Self::ChatGptImage2 => "ChatGPT Image 2",
            Self::ReplicateZImageTurbo => "Replicate Z-Image Turbo",
            Self::Gemini31Flash => "Gemini 3.1 Flash Image",
            Self::Grok3 => "Grok 3 / xAI image",
        }
    }
}

pub(super) const DEFAULT_PROVIDER_ORDER: [ImageProvider; 4] = [
    ImageProvider::ReplicateZImageTurbo,
    ImageProvider::ChatGptImage2,
    ImageProvider::Gemini31Flash,
    ImageProvider::Grok3,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SpeechProvider {
    OpenAiTts,
    ElevenLabs,
    QwenDashScope,
    AzureEdgeTts,
    AzureSpeech,
    ReplicateQwen3Tts,
    ReplicateChatterbox,
}

impl SpeechProvider {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::OpenAiTts => "openai_tts",
            Self::ElevenLabs => "elevenlabs",
            Self::QwenDashScope => "qwen_dashscope",
            Self::AzureEdgeTts => "azure_edge_tts",
            Self::AzureSpeech => "azure_speech",
            Self::ReplicateQwen3Tts => "replicate_qwen3_tts",
            Self::ReplicateChatterbox => "replicate_chatterbox",
        }
    }

    pub(super) fn display_name(self) -> &'static str {
        match self {
            Self::OpenAiTts => "OpenAI TTS",
            Self::ElevenLabs => "ElevenLabs",
            Self::QwenDashScope => "Qwen DashScope TTS",
            Self::AzureEdgeTts => "Microsoft Edge TTS",
            Self::AzureSpeech => "Azure Speech",
            Self::ReplicateQwen3Tts => "Replicate qwen/qwen3-tts",
            Self::ReplicateChatterbox => "Replicate Chatterbox TTS",
        }
    }
}

pub(super) const DEFAULT_SPEECH_PROVIDER_ORDER: [SpeechProvider; 7] = [
    SpeechProvider::QwenDashScope,
    SpeechProvider::AzureEdgeTts,
    SpeechProvider::ReplicateQwen3Tts,
    SpeechProvider::AzureSpeech,
    SpeechProvider::OpenAiTts,
    SpeechProvider::ElevenLabs,
    SpeechProvider::ReplicateChatterbox,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TextLanguage {
    ZhCn,
    EnUs,
    JaJp,
    KoKr,
    EsEs,
    FrFr,
}

impl TextLanguage {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::ZhCn => "zh_cn",
            Self::EnUs => "en_us",
            Self::JaJp => "ja_jp",
            Self::KoKr => "ko_kr",
            Self::EsEs => "es_es",
            Self::FrFr => "fr_fr",
        }
    }

    pub(super) fn bcp47(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::EnUs => "en-US",
            Self::JaJp => "ja-JP",
            Self::KoKr => "ko-KR",
            Self::EsEs => "es-ES",
            Self::FrFr => "fr-FR",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum VoiceRole {
    FemaleGentle,
    FemaleBright,
    FemaleConfident,
    FemaleYoung,
    MaleCalm,
    MaleWarm,
    MaleDeep,
    MaleEnergetic,
}

impl VoiceRole {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::FemaleGentle => "female_gentle",
            Self::FemaleBright => "female_bright",
            Self::FemaleConfident => "female_confident",
            Self::FemaleYoung => "female_young",
            Self::MaleCalm => "male_calm",
            Self::MaleWarm => "male_warm",
            Self::MaleDeep => "male_deep",
            Self::MaleEnergetic => "male_energetic",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SpeechTone {
    Neutral,
    Calm,
    Cheerful,
    Serious,
    Sad,
    Whisper,
}

impl SpeechTone {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Calm => "calm",
            Self::Cheerful => "cheerful",
            Self::Serious => "serious",
            Self::Sad => "sad",
            Self::Whisper => "whisper",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct GenerateMediaArgs {
    pub(super) kind: MediaKind,
    pub(super) prompt: String,
    pub(super) references: Vec<String>,
    pub(super) output_dir: String,
    pub(super) width: Option<u32>,
    pub(super) height: Option<u32>,
    pub(super) size: Option<String>,
    pub(super) aspect_ratio: Option<String>,
    pub(super) quality: String,
    pub(super) count: usize,
    pub(super) seed: Option<u64>,
    pub(super) output_format: String,
    pub(super) provider_order: Vec<ImageProvider>,
    pub(super) speech_provider_order: Vec<SpeechProvider>,
    pub(super) text_language: Option<TextLanguage>,
    pub(super) voice_role: Option<VoiceRole>,
    pub(super) speech_tone: Option<SpeechTone>,
    pub(super) custom_tone_description: Option<String>,
    pub(super) custom_voice_description: Option<String>,
    pub(super) dry_run: bool,
    pub(super) extra_body: Option<Value>,
}

#[derive(Clone, Debug)]
pub(super) struct ImageBytes {
    pub(super) bytes: Vec<u8>,
    pub(super) mime_type: String,
    pub(super) source_url: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ProviderOutcome {
    pub(super) provider: ImageProvider,
    pub(super) model: String,
    pub(super) images: Vec<ImageBytes>,
    pub(super) raw: Value,
}

#[derive(Clone, Debug)]
pub(super) struct SpeechBytes {
    pub(super) bytes: Vec<u8>,
    pub(super) mime_type: String,
    pub(super) extension: String,
    pub(super) raw: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Dimensions {
    pub(super) width: u32,
    pub(super) height: u32,
}
