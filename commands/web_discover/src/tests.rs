use super::args::parse_args_text;
use super::download::*;
use super::filter::*;
use super::html::*;
use super::search::*;
use super::types::{SearchResult, WebDiscoverArgs, DEFAULT_MAX_SIZE};
use reqwest::blocking::Client;
use std::path::PathBuf;

#[test]
fn direct_webpage_url_accepts_only_single_http_url() {
    assert_eq!(
        direct_webpage_url("https://cloud.google.com/vertex-ai/generative-ai/docs/image")
            .as_deref(),
        Some("https://cloud.google.com/vertex-ai/generative-ai/docs/image")
    );
    assert_eq!(
        direct_webpage_url("\"https://example.com/docs\"").as_deref(),
        Some("https://example.com/docs")
    );
    assert!(direct_webpage_url("site:cloud.google.com Vertex AI docs").is_none());
    assert!(direct_webpage_url("https://example.com/docs extra words").is_none());
    assert!(direct_webpage_url("https://example.com/a.jpg https://example.com/b.jpg").is_none());
    assert!(direct_webpage_url("ftp://example.com/file").is_none());
    assert_eq!(
        direct_webpage_urls("https://example.com/a.jpg https://example.com/b.jpg"),
        vec!["https://example.com/a.jpg", "https://example.com/b.jpg"]
    );
}

#[test]
fn direct_media_urls_bypass_search() {
    let client = Client::builder().build().expect("client");
    let image = search_media_links(
        &client,
        "image",
        "https://officialsite.cds-jp.online/prod/profile_member/105/158/2c38bd5497b94e38aba150b784a7de87.webp",
        5,
    )
    .expect("direct image url");
    assert_eq!(image.len(), 1);
    assert_eq!(
        image[0].url,
        "https://officialsite.cds-jp.online/prod/profile_member/105/158/2c38bd5497b94e38aba150b784a7de87.webp"
    );
    assert_eq!(image[0].source, "direct_image_url");
    let images = search_media_links(
        &client,
        "image",
        "https://example.com/a.jpg https://example.com/b.webp",
        5,
    )
    .expect("direct image urls");
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].url, "https://example.com/a.jpg");
    assert_eq!(images[1].url, "https://example.com/b.webp");

    let video = search_media_links(
        &client,
        "video",
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        5,
    )
    .expect("direct video url");
    assert_eq!(video.len(), 1);
    assert_eq!(video[0].url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    assert_eq!(video[0].source, "direct_video_url");

    let audio = search_media_links(
        &client,
        "audio",
        "\"https://www.bilibili.com/video/BV1xx411c7mD\"",
        5,
    )
    .expect("direct audio url");
    assert_eq!(audio.len(), 1);
    assert_eq!(audio[0].url, "https://www.bilibili.com/video/BV1xx411c7mD");
    assert_eq!(audio[0].source, "direct_audio_url");
}

#[test]
fn parse_video_format_selector_for_ytdlp_downloads() {
    let args = parse_args_text(
        r#"video "product demo" --download-dir media/video --format "bestvideo[height<=1080]+bestaudio/best""#,
    )
    .expect("parse web_discover args");

    assert_eq!(args.kind, "video");
    assert_eq!(
        args.format_selector.as_deref(),
        Some("bestvideo[height<=1080]+bestaudio/best")
    );
}

#[test]
fn default_ytdlp_formats_prefer_best_available_media() {
    assert_eq!(default_ytdlp_format("audio"), "bestaudio/best");
    assert_eq!(
        default_ytdlp_format("video"),
        "best[height<=540][ext=mp4]/best[height<=540]/best"
    );
}

#[test]
fn video_download_candidates_prefer_video_files_over_larger_audio() {
    let mut paths = [
        PathBuf::from("clip.f251.webm"),
        PathBuf::from("clip.f134.mp4"),
        PathBuf::from("clip.f251.m4a"),
    ];
    paths.sort_by_key(|path| ytdlp_download_candidate_rank(path, "video"));

    assert_eq!(paths[0], PathBuf::from("clip.f134.mp4"));
}

#[test]
fn parse_exa_web_results_reads_sse_title_url_blocks() {
    let raw = r#"event: message
data: {"result":{"content":[{"type":"text","text":"Title: prunaai/z-image-turbo | API reference - Replicate\nURL: https://replicate.com/prunaai/z-image-turbo/api/api-reference\nPublished: 2026-02-27T15:34:39.000Z\nAuthor: N/A\nHighlights:\nPlayground API Examples README\n\n---\n\nTitle: Z-Image Turbo | Readme\nURL: https://replicate.com/prunaai/z-image-turbo/readme\nHighlights:\nReadme text"}]},"jsonrpc":"2.0","id":1}
"#;

    let results = parse_exa_web_results(raw, 5).expect("exa results");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].source, "exa_web");
    assert_eq!(
        results[0].url,
        "https://replicate.com/prunaai/z-image-turbo/api/api-reference"
    );
    assert!(results[0].title.contains("API reference"));
}

#[test]
fn extract_page_image_url_reads_meta_and_resolves_relative_urls() {
    let html = r#"
        <html>
          <head><meta property="og:image" content="/images/minji.webp"></head>
          <body><img src="/fallback.jpg"></body>
        </html>
    "#;

    assert_eq!(
        extract_page_image_url(html, "https://example.com/profile/minji").as_deref(),
        Some("https://example.com/images/minji.webp")
    );
}

#[test]
fn extract_reader_title_reads_jina_style_title() {
    assert_eq!(
        extract_reader_title("Title: Replicate API\n\nMarkdown body").as_deref(),
        Some("Replicate API")
    );
    assert_eq!(
        extract_reader_title("# Markdown Heading\n\nBody").as_deref(),
        Some("Markdown Heading")
    );
}

#[test]
fn html_to_markdown_text_preserves_structure_and_drops_page_noise() {
    let html = r#"
        <html>
          <head>
            <meta property="og:image" content="/social-card.webp">
            <style>.hero { color: red; }</style>
          </head>
          <body>
            <nav>Home Docs Pricing</nav>
            <main>
              <h1>API Reference</h1>
              <img src="/media/profile.webp" alt="Profile photo">
              <p>Create an image with this endpoint.</p>
              <ul><li>Send a prompt</li><li>Read the output URL</li></ul>
              <pre><code class="language-bash">curl https://api.example.com/v1/images</code></pre>
              <a href="/docs/images">Image docs</a>
              <script>window.payload = "https:\/\/cdn.example.com\/prod\/photo.jpg";</script>
            </main>
            <script>window.noise = true</script>
          </body>
        </html>
    "#;

    let markdown = html_to_markdown_text(html, "https://example.com/reference/");

    assert!(markdown.contains("# API Reference"));
    assert!(markdown.contains("- Send a prompt"));
    assert!(markdown.contains("```bash"));
    assert!(markdown.contains("[Image docs](https://example.com/docs/images)"));
    assert!(markdown.contains("![Profile photo](https://example.com/media/profile.webp)"));
    assert!(markdown.contains("https://example.com/social-card.webp"));
    assert!(markdown.contains("https://cdn.example.com/prod/photo.jpg"));
    assert!(!markdown.contains("window.noise"));
    assert!(!markdown.contains("color: red"));
}

#[test]
fn site_filter_does_not_reject_image_results() {
    let result = SearchResult {
        title: "site:wikipedia.org 唐玄奘 画像".to_string(),
        url: "https://example-travel.invalid/random-garden.jpg".to_string(),
        page_url: Some("https://example-travel.invalid/article".to_string()),
        snippet: "https://example-travel.invalid/article".to_string(),
        source: "bing_images_mediaurl".to_string(),
    };

    let filtered = filter_results(
        vec![result],
        &WebDiscoverArgs {
            kind: "image".to_string(),
            query: "site:wikipedia.org 唐玄奘 画像".to_string(),
            include_regex: None,
            exclude_regex: None,
            max_results: 5,
            download_dir: Some("media".to_string()),
            min_size: 1,
            max_size: DEFAULT_MAX_SIZE,
            format_selector: None,
        },
    )
    .expect("filter results");

    assert_eq!(filtered.len(), 1);
}

#[test]
fn image_query_treats_site_filter_as_keywords() {
    assert_eq!(
        site_filters_to_image_keywords("site: newjeans.kr Minji official profile"),
        "newjeans.kr Minji official profile"
    );
    assert_eq!(
        site_filters_to_image_keywords("site:newjeans.kr, Minji official profile"),
        "newjeans.kr Minji official profile"
    );
}

#[test]
fn bing_mediaurl_title_uses_real_context_not_query_page_title() {
    let context = r#"
        mediaurl=https%3a%2f%2fimages.example.invalid%2funrelated.jpg&amp;cdnurl=https%3a%2f%2fth.bing.com%2fthumb.jpg
        <img alt="Résultat d’images pour site:wikipedia.org 唐玄奘 画像" />
        <div class="lnkw"><a title="example.invalid" target="_blank" data-hookid="pgdom" href="https://example.invalid/source-page">example.invalid</a></div>
    "#;

    let page_url = extract_bing_image_page_url(context).expect("page url");
    let title = extract_bing_image_title(
        context,
        Some(&page_url),
        "https://images.example.invalid/unrelated.jpg",
    );

    assert_eq!(page_url, "https://example.invalid/source-page");
    assert_eq!(title, "example.invalid");
    assert!(!title.contains("唐玄奘"));
}
