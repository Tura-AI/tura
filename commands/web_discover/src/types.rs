pub(super) const DEFAULT_MAX_RESULTS: usize = 5;
pub(super) const DEFAULT_MIN_SIZE: u64 = 1;
pub(super) const DEFAULT_IMAGE_MIN_SIZE: u64 = 10_000;
pub(super) const DEFAULT_MAX_SIZE: u64 = 80_000_000;
pub(super) const MAX_WEBSITE_RESPONSE_SIZE: usize = 5 * 1024 * 1024;
pub(super) const MIN_WEBSITE_TEXT_CHARS_FOR_READER: usize = 1_200;

#[derive(Clone, Debug)]
pub(super) struct WebDiscoverArgs {
    pub(super) kind: String,
    pub(super) query: String,
    pub(super) include_regex: Option<String>,
    pub(super) exclude_regex: Option<String>,
    pub(super) max_results: usize,
    pub(super) download_dir: Option<String>,
    pub(super) min_size: u64,
    pub(super) max_size: u64,
    pub(super) format_selector: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct SearchResult {
    pub(super) title: String,
    pub(super) url: String,
    pub(super) snippet: String,
    pub(super) source: String,
    pub(super) page_url: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct WebsiteContent {
    pub(super) title: Option<String>,
    pub(super) text: String,
    pub(super) content_type: String,
    pub(super) fetch_mode: String,
}
