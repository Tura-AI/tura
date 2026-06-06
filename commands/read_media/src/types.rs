use serde_json::Value;

pub(super) const MAX_VISUALS: usize = 60;

#[derive(Clone, Debug)]
pub(super) struct ReadMediaArgs {
    pub(super) paths: Vec<String>,
    pub(super) include_text: bool,
    pub(super) max_text_chars: usize,
    pub(super) max_visuals: usize,
    pub(super) max_side: u32,
    pub(super) max_files: usize,
    pub(super) pdf_max_pages: usize,
    pub(super) document_attachment_bytes: u64,
    pub(super) audio_preview_bytes: u64,
}

pub(super) struct MediaContent {
    pub(super) text: String,
    pub(super) visual_previews: Vec<Value>,
    pub(super) audio_previews: Vec<Value>,
    pub(super) file_attachments: Vec<Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReadMode {
    Detailed,
    ThumbnailOnly,
}
