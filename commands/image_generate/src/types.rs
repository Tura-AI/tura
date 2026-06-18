use serde_json::Value;

pub(super) const DEFAULT_OUTPUT_DIR: &str = "media/image";
pub(super) const DEFAULT_OUTPUT_FORMAT: &str = "png";
pub(super) const DEFAULT_QUALITY: &str = "auto";
pub(super) const DEFAULT_SIZE: &str = "1024x1024";
pub(super) const DEFAULT_COUNT: usize = 1;
pub(super) const MAX_COUNT: usize = 4;

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

#[derive(Clone, Debug)]
pub(super) struct ImageGenerateArgs {
    pub(super) prompt: String,
    pub(super) negative_prompt: Option<String>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Dimensions {
    pub(super) width: u32,
    pub(super) height: u32,
}
