use super::args::*;
use super::asset::*;
use super::download::*;
use super::files::*;
use super::filter::*;
use super::html::*;
use super::output::*;
use super::search::*;
use super::types::{
    SearchResult, WebDiscoverArgs, DEFAULT_IMAGE_MIN_SIZE, DEFAULT_MAX_RESULTS, DEFAULT_MAX_SIZE,
    DEFAULT_MIN_SIZE,
};
use super::util::*;
use super::{access, execute, handle_envelope, Envelope};
use reqwest::blocking::Client;
use serde_json::json;
use std::fs;
use std::io::{Read, Write};
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
            asset_type: None,
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

#[test]
fn parse_cli_args_accepts_command_name_aliases_and_bounds() {
    let args = parse_args_text(
        r#"web-search --kind=photos --query "Minji official profile" --include-regex=newjeans --exclude_regex=fanmade --limit 999 --download-dir "media/images" --min-size 123 --max_size 456 --format "best""#,
    )
    .expect("parse aliased CLI");

    assert_eq!(args.kind, "image");
    assert_eq!(args.query, "Minji official profile");
    assert_eq!(args.include_regex.as_deref(), Some("newjeans"));
    assert_eq!(args.exclude_regex.as_deref(), Some("fanmade"));
    assert_eq!(args.max_results, 20);
    assert_eq!(args.download_dir.as_deref(), Some("media/images"));
    assert_eq!(args.min_size, 123);
    assert_eq!(args.max_size, 456);
    assert_eq!(args.format_selector.as_deref(), Some("best"));
}

#[test]
fn parse_cli_args_treats_first_media_kind_as_type() {
    let args = parse_args_text(r#"video "launch keynote" -n 0"#).expect("parse media kind");

    assert_eq!(args.kind, "video");
    assert_eq!(args.query, "launch keynote");
    assert_eq!(args.max_results, 1);
    assert_eq!(args.min_size, DEFAULT_MIN_SIZE);
}

#[test]
fn parse_cli_args_ignores_unknown_options_without_stealing_query() {
    let args = parse_args_text("--unused ignored website rust docs")
        .expect("unknown option should not make the command invalid");

    assert_eq!(args.kind, "website");
    assert_eq!(args.query, "rust docs");
    assert_eq!(args.max_results, DEFAULT_MAX_RESULTS);
}

#[test]
fn parse_cli_args_rejects_empty_and_unsupported_kind() {
    assert_eq!(
        parse_args_text("   ").expect_err("empty query is invalid"),
        "web_discover query cannot be empty"
    );
    assert_eq!(
        parse_args_text("--type archive rust docs").expect_err("archive is unsupported"),
        "unsupported web_discover type: archive"
    );
}

#[test]
fn parse_json_args_accepts_schema_aliases_and_string_numbers() {
    let args = parse_args_value(json!({
        "mediaType": "web-page",
        "keywords": "Rust async cancellation",
        "includeRegex": "tokio",
        "exclude": "sponsored",
        "n": "12",
        "outDir": "web",
        "minSize": "2",
        "maxSize": "1000",
        "ytDlpFormat": "  "
    }))
    .expect("parse JSON aliases");

    assert_eq!(args.kind, "website");
    assert_eq!(args.query, "Rust async cancellation");
    assert_eq!(args.include_regex.as_deref(), Some("tokio"));
    assert_eq!(args.exclude_regex.as_deref(), Some("sponsored"));
    assert_eq!(args.max_results, 12);
    assert_eq!(args.download_dir.as_deref(), Some("web"));
    assert_eq!(args.min_size, 2);
    assert_eq!(args.max_size, 1000);
    assert_eq!(args.format_selector, None);
}

#[test]
fn parse_asset_args_accepts_asset_type_and_aliases() {
    let args = parse_args_text(r#"web_discover asset 3d "space cruiser" -o assets --limit 2"#)
        .expect("parse asset cli");

    assert_eq!(args.kind, "asset");
    assert_eq!(args.asset_type.as_deref(), Some("3d"));
    assert_eq!(args.query, "space cruiser");
    assert_eq!(args.download_dir.as_deref(), Some("assets"));
    assert_eq!(args.max_results, 2);

    let audio_args = parse_args_text(r#"asset audio "laser click" --download-dir assets"#)
        .expect("parse asset audio");
    assert_eq!(audio_args.kind, "asset");
    assert_eq!(audio_args.asset_type.as_deref(), Some("audio"));
    assert_eq!(audio_args.query, "laser click");

    for (asset_type, query) in [
        ("shader", "scanline wgsl"),
        ("texture", "brushed metal"),
        ("2d", "pixel cockpit"),
        ("3d", "patrol ship"),
        ("audio", "laser click"),
    ] {
        let parsed = parse_args_text(&format!("asset {asset_type} \"{query}\" -o assets"))
            .expect("parse asset subtype");
        assert_eq!(parsed.kind, "asset", "{asset_type}");
        assert_eq!(
            parsed.asset_type.as_deref(),
            Some(asset_type),
            "{asset_type}"
        );
        assert_eq!(parsed.query, query, "{asset_type}");
        assert_eq!(
            parsed.download_dir.as_deref(),
            Some("assets"),
            "{asset_type}"
        );
    }

    let json_args = parse_args_value(json!({
        "kind": "assets",
        "assetType": "materials",
        "query": "brushed metal"
    }))
    .expect("parse asset JSON");
    assert_eq!(json_args.kind, "asset");
    assert_eq!(json_args.asset_type.as_deref(), Some("texture"));

    assert_eq!(normalize_kind("game-assets"), "asset");
    assert_eq!(normalize_asset_type("sfx"), "audio");
}

#[test]
fn parse_json_args_accepts_cli_string_and_rejects_arrays() {
    let args = parse_args_value(json!("image newjeans minji --limit=3")).expect("string payload");
    assert_eq!(args.kind, "image");
    assert_eq!(args.query, "newjeans minji");
    assert_eq!(args.max_results, 3);
    assert_eq!(args.min_size, DEFAULT_IMAGE_MIN_SIZE);

    assert_eq!(
        parse_args_value(json!(["image", "minji"])).expect_err("arrays are not supported"),
        "web_discover input must be object or CLI text"
    );
}

#[test]
fn parse_args_text_reports_invalid_json_before_cli_fallback() {
    let error = parse_args_text(r#"{"query":"rust docs", "kind":"website""#)
        .expect_err("malformed JSON object should fail as JSON");

    assert!(error.starts_with("invalid web_discover JSON:"));
}

#[test]
fn parse_json_args_accepts_wrapped_cli_aliases_before_object_fields() {
    for key in [
        "cli",
        "command_line",
        "commandLine",
        "input",
        "args",
        "payload",
    ] {
        let args = parse_args_value(json!({
            key: "web_discover --type image --query \"NewJeans profile\" --limit 2",
            "query": "this object query should be ignored"
        }))
        .expect("wrapped cli alias");

        assert_eq!(args.kind, "image", "{key}");
        assert_eq!(args.query, "NewJeans profile", "{key}");
        assert_eq!(args.max_results, 2, "{key}");
    }
}

#[test]
fn parse_cli_args_reports_missing_values_for_value_options() {
    for input in [
        "--query",
        "--type",
        "--include-regex",
        "--exclude_regex",
        "--download-dir",
        "--min-size",
        "--max_size",
        "--format",
        "--asset-type",
    ] {
        let error = parse_args_text(input).expect_err("missing value should fail");
        assert!(error.contains("requires a value"), "{input}: {error}");
    }
}

#[test]
fn parse_cli_args_clamps_bad_numeric_options_to_safe_defaults() {
    let args = parse_args_text(
        r#"audio "interview clip" --limit nope --min-size nope --max-size nope --format "  ""#,
    )
    .expect("parse numeric fallbacks");

    assert_eq!(args.kind, "audio");
    assert_eq!(args.query, "interview clip");
    assert_eq!(args.max_results, DEFAULT_MAX_RESULTS);
    assert_eq!(args.min_size, DEFAULT_MIN_SIZE);
    assert_eq!(args.max_size, DEFAULT_MAX_SIZE);
    assert_eq!(args.format_selector, None);

    let clamped = parse_args_text(r#"website rust --limit -5 --max-size 0"#)
        .expect("negative limit is treated as parse fallback");
    assert_eq!(clamped.max_results, DEFAULT_MAX_RESULTS);
    assert_eq!(clamped.max_size, 1);
}

#[test]
fn normalize_kind_covers_internal_canonical_values_only() {
    assert_eq!(normalize_kind("web"), "website");
    assert_eq!(normalize_kind("webpages"), "website");
    assert_eq!(normalize_kind("img"), "image");
    assert_eq!(normalize_kind("photos"), "image");
    assert_eq!(normalize_kind("movies"), "video");
    assert_eq!(normalize_kind("music"), "audio");
    assert_eq!(normalize_kind("assets"), "asset");
    assert_eq!(normalize_kind("custom_type"), "custom_type");
}

#[test]
fn asset_source_queries_cover_prompted_asset_sources() {
    let sources = asset_source_queries("auto", "space cruiser")
        .into_iter()
        .map(|source| source.source)
        .collect::<Vec<_>>();

    for expected in [
        "polydown_poly_pizza",
        "magic_ui",
        "shadcn_ui",
        "objaverse",
        "ambientcg_api",
        "sketchfab_download_api",
        "freesound_api",
        "internet_archive",
    ] {
        assert!(sources.contains(&expected), "{expected} not searched");
    }

    let model_sources = asset_source_queries("3d", "fighter")
        .into_iter()
        .map(|source| source.source)
        .collect::<Vec<_>>();
    assert!(model_sources.contains(&"polydown_poly_pizza"));
    assert!(model_sources.contains(&"objaverse"));
    assert!(model_sources.contains(&"sketchfab_download_api"));
    assert!(model_sources.contains(&"internet_archive"));
    assert!(!model_sources.contains(&"magic_ui"));
}

#[test]
fn split_cli_assignment_only_handles_option_assignments() {
    assert_eq!(
        split_cli_assignment("--limit=3"),
        ("--limit".to_string(), Some("3".to_string()))
    );
    assert_eq!(
        split_cli_assignment("query=value"),
        ("query=value".to_string(), None)
    );
}

#[test]
fn query_requirements_support_positive_and_negative_terms() {
    let requirements = parse_query_requirements("rust docs, not nightly, 排除: draft, 不要 mirror");

    assert_eq!(requirements.0, vec!["rust docs"]);
    assert_eq!(
        requirements.1,
        vec!["nightly", "draft", "mirror"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        build_search_query(&requirements),
        "rust docs -nightly -draft -mirror"
    );
}

#[test]
fn query_requirements_keep_negative_only_query_searchable() {
    let requirements = parse_query_requirements("not beta, exclude old");

    assert_eq!(requirements.0, vec!["not beta, exclude old"]);
    assert_eq!(requirements.1, vec!["beta", "old"]);
    assert_eq!(
        build_search_query(&requirements),
        "not beta, exclude old -beta -old"
    );
}

#[test]
fn filter_results_applies_include_exclude_site_and_limit() {
    let args = WebDiscoverArgs {
        kind: "website".to_string(),
        asset_type: None,
        query: "site:example.com rust".to_string(),
        include_regex: Some("Rust|Tokio".to_string()),
        exclude_regex: Some("draft".to_string()),
        max_results: 1,
        download_dir: None,
        min_size: DEFAULT_MIN_SIZE,
        max_size: DEFAULT_MAX_SIZE,
        format_selector: None,
    };
    let results = vec![
        result(
            "Rust Guide",
            "https://docs.example.com/rust",
            "Tokio runtime",
            "web",
        ),
        result(
            "Rust Draft",
            "https://docs.example.com/draft",
            "draft only",
            "web",
        ),
        result(
            "Rust Mirror",
            "https://other.example.net/rust",
            "Tokio runtime",
            "web",
        ),
    ];

    let filtered = filter_results(results, &args).expect("filter");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].url, "https://docs.example.com/rust");
}

#[test]
fn filter_results_returns_regex_errors_with_field_context() {
    let args = WebDiscoverArgs {
        kind: "website".to_string(),
        asset_type: None,
        query: "rust".to_string(),
        include_regex: Some("(".to_string()),
        exclude_regex: None,
        max_results: 5,
        download_dir: None,
        min_size: DEFAULT_MIN_SIZE,
        max_size: DEFAULT_MAX_SIZE,
        format_selector: None,
    };

    let error = filter_results(Vec::new(), &args).expect_err("invalid regex must fail");

    assert!(error.starts_with("invalid include_regex:"));
}

#[test]
fn filter_results_reports_exclude_regex_errors_with_field_context() {
    let args = WebDiscoverArgs {
        kind: "website".to_string(),
        asset_type: None,
        query: "rust".to_string(),
        include_regex: None,
        exclude_regex: Some("[".to_string()),
        max_results: 5,
        download_dir: None,
        min_size: DEFAULT_MIN_SIZE,
        max_size: DEFAULT_MAX_SIZE,
        format_selector: None,
    };

    let error = filter_results(Vec::new(), &args).expect_err("invalid exclude regex must fail");

    assert!(error.starts_with("invalid exclude_regex:"));
}

#[test]
fn filter_results_applies_site_filter_only_to_website_results() {
    let image_args = WebDiscoverArgs {
        kind: "image".to_string(),
        asset_type: None,
        query: "site:official.example profile".to_string(),
        include_regex: None,
        exclude_regex: None,
        max_results: 5,
        download_dir: None,
        min_size: DEFAULT_IMAGE_MIN_SIZE,
        max_size: DEFAULT_MAX_SIZE,
        format_selector: None,
    };
    let video_args = WebDiscoverArgs {
        kind: "video".to_string(),
        asset_type: None,
        query: "site:official.example concert".to_string(),
        include_regex: None,
        exclude_regex: None,
        max_results: 5,
        download_dir: None,
        min_size: DEFAULT_MIN_SIZE,
        max_size: DEFAULT_MAX_SIZE,
        format_selector: None,
    };
    let offsite = result(
        "Profile",
        "https://cdn.other.example/profile.jpg",
        "offsite media",
        "direct_image_url",
    );

    assert_eq!(
        filter_results(vec![offsite.clone()], &image_args)
            .expect("image filter")
            .len(),
        1
    );
    assert_eq!(
        filter_results(vec![offsite], &video_args)
            .expect("video filter")
            .len(),
        1
    );
}

#[test]
fn bing_image_include_regex_uses_url_context_not_query_title() {
    let args = WebDiscoverArgs {
        kind: "image".to_string(),
        asset_type: None,
        query: "site:official.example profile".to_string(),
        include_regex: Some("official\\.example".to_string()),
        exclude_regex: None,
        max_results: 5,
        download_dir: None,
        min_size: DEFAULT_IMAGE_MIN_SIZE,
        max_size: DEFAULT_MAX_SIZE,
        format_selector: None,
    };
    let results = vec![
        SearchResult {
            title: "site:official.example generated query title".to_string(),
            url: "https://cdn.other.invalid/photo.jpg".to_string(),
            page_url: Some("https://source.other.invalid/profile".to_string()),
            snippet: "source page".to_string(),
            source: "bing_images_mediaurl".to_string(),
        },
        SearchResult {
            title: "profile".to_string(),
            url: "https://cdn.official.example/photo.jpg".to_string(),
            page_url: None,
            snippet: "source page".to_string(),
            source: "bing_images_mediaurl".to_string(),
        },
    ];

    let filtered = filter_results(results, &args).expect("filter");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].url, "https://cdn.official.example/photo.jpg");
}

#[test]
fn site_filter_helpers_normalize_hosts_and_subdomains() {
    assert_eq!(
        site_filters("site:https://www.Example.COM/docs rust site:sub.example.net, test"),
        vec!["example.com", "sub.example.net"]
    );
    assert_eq!(
        strip_site_filters_from_query("site:example.com rust docs"),
        "rust docs"
    );
    assert!(url_host_matches(
        "https://docs.example.com:443/path?x=1",
        "example.com"
    ));
    assert!(url_host_matches(
        "https://user:pass@example.com/path",
        "example.com"
    ));
    assert!(!url_host_matches(
        "https://badexample.com/path",
        "example.com"
    ));
    assert_eq!(
        url_host("https://www.Example.com:8443/path"),
        Some("example.com".to_string())
    );
}

#[test]
fn site_filter_helpers_handle_userinfo_ports_paths_and_invalid_urls() {
    assert_eq!(
        site_filters("rust site:http://user:pass@www.Example.com:8443/docs?q=1"),
        vec!["example.com"]
    );
    assert!(url_host_matches(
        "https://user:pass@docs.example.com:8443/path",
        "example.com"
    ));
    assert!(url_host_matches("docs.example.com/path", "example.com"));
    assert!(!url_host_matches("", "example.com"));
    assert_eq!(url_host("https:///missing-host"), None);
}

#[test]
fn duckduckgo_parser_decodes_redirects_and_snippets() {
    let html = r#"
        <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Frust&amp;rut=abc">Rust &amp; Async</a>
        <a class="result__snippet">Learn <b>Rust</b> async patterns.</a>
        <a class="result__a" href="https://example.net/second">Second</a>
        <a class="result__snippet">Second snippet</a>
    "#;

    let results = parse_duckduckgo_html_results(html, 1);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Rust & Async");
    assert_eq!(results[0].url, "https://example.com/rust");
    assert_eq!(results[0].snippet, "Learn Rust async patterns.");
    assert_eq!(
        normalize_duckduckgo_url("https://example.com?a=1&amp;b=2"),
        "https://example.com?a=1&b=2"
    );
}

#[test]
fn media_link_extraction_resolves_relative_srcset_json_and_protocol_urls() {
    let html = r#"
        <img srcset="/small.jpg 1x, /large.webp 2x">
        <video poster="//cdn.example.com/poster.png"></video>
        <script>{"url":"https:\/\/cdn.example.com\/clip.mp4"}</script>
        <img src="data:image/png;base64,abc">
        <a href="/docs/not-media"></a>
    "#;

    let links = extract_media_links(html, "https://example.com/articles/page");

    assert_eq!(
        links,
        vec![
            "https://example.com/small.jpg",
            "https://example.com/large.webp",
            "https://cdn.example.com/poster.png",
            "https://cdn.example.com/clip.mp4",
        ]
    );
    assert_eq!(
        normalize_media_url("../media/photo.avif?x=1", "https://example.com/a/b/page").as_deref(),
        Some("https://example.com/a/media/photo.avif?x=1")
    );
    assert!(normalize_media_url("blob:https://example.com/id", "https://example.com").is_none());
}

#[test]
fn html_helpers_extract_titles_and_normalize_markdown() {
    let html = r#"
        <title>Rust &amp; Async</title>
        <style>body { color: red; }</style>
        <script>alert(1)</script>
        <h1>Heading</h1>
        <p>One&nbsp;two</p>
    "#;

    assert_eq!(extract_title(html).as_deref(), Some("Rust & Async"));
    assert!(!remove_html_noise(html).contains("alert(1)"));
    assert_eq!(
        normalize_markdown("One&nbsp;two\n\n\nThree\n"),
        "One two\n\nThree"
    );
    assert_eq!(title_from_url("https://example.com/docs/rust"), "rust");
    assert_eq!(title_from_url("not a url"), "Webpage");
}

#[test]
fn bing_image_page_url_ignores_bing_and_media_links() {
    let context = r#"
        <a href="https://www.bing.com/images/search?q=x">bing</a>
        <a href="https://cdn.example.com/image.jpg">media</a>
        <a href="https://source.example.com/article">source</a>
    "#;

    assert_eq!(
        extract_bing_image_page_url(context).as_deref(),
        Some("https://source.example.com/article")
    );
}

#[test]
fn util_string_parsers_and_decoders_cover_edge_values() {
    let value = json!({
        "name": "  Alice  ",
        "empty": " ",
        "nested": { "title": "  Nested  " },
        "count": "42",
        "bad": "nope"
    });

    assert_eq!(
        split_cli_words(r#"one "two words" 'three words'"#),
        vec!["one", "two words", "three words"]
    );
    assert_eq!(
        string_field(&value, &["missing", "name"]).as_deref(),
        Some("Alice")
    );
    assert_eq!(string_field(&value, &["empty"]), None);
    assert_eq!(
        string_field_at(&value, &[&["nested", "title"]]).as_deref(),
        Some("Nested")
    );
    assert_eq!(u64_field(&value, &["count"]), Some(42));
    assert_eq!(u64_field(&value, &["bad"]), None);
    assert_eq!(
        clean_text("<b>Rust</b>&nbsp;&amp;&nbsp;Tura"),
        "Rust & Tura"
    );
    assert_eq!(
        json_unescape(r#"https:\/\/example.com\/a"#),
        "https://example.com/a"
    );
    assert_eq!(percent_decode("a+b%20c%ZZ"), "a b c%ZZ");
}

#[test]
fn filename_content_type_and_truncation_helpers_are_stable() {
    assert_eq!(
        safe_filename(" NewJeans: Minji/Profile! "),
        "NewJeans-Minji-Profile"
    );
    assert_eq!(safe_filename("!!!"), "result");
    assert_eq!(
        extension_from_url("https://example.com/a.JPEG?x=1"),
        Some("jpg")
    );
    assert_eq!(
        extension_from_url("https://cdn.example.com/model.GLB?download=1"),
        Some("glb")
    );
    assert_eq!(
        extension_from_url("https://cdn.example.com/effect.WGSL"),
        Some("wgsl")
    );
    assert_eq!(
        extension_from_url("https://cdn.example.com/hit.OGG"),
        Some("ogg")
    );
    assert_eq!(extension_from_url("https://example.com/a.txt"), None);
    assert_eq!(
        content_type_for_path(&PathBuf::from("photo.webp"), "image"),
        "image/webp"
    );
    assert_eq!(
        content_type_for_path(&PathBuf::from("ship.glb"), "3d"),
        "model/gltf-binary"
    );
    assert_eq!(
        content_type_for_path(&PathBuf::from("shader.wgsl"), "shader"),
        "text/plain"
    );
    assert_eq!(
        content_type_for_path(&PathBuf::from("click.ogg"), "audio"),
        "audio/ogg"
    );
    assert_eq!(
        content_type_for_path(&PathBuf::from("page.md"), "website"),
        "text/markdown"
    );
    assert_eq!(truncate_chars("abcdef", 3), "abc");
    assert_eq!(middle_truncate_chars("abcdef", 3), "abc");
    assert_eq!(middle_truncate_chars("abcdef", 100), "abcdef");
    assert!(middle_truncate_chars("abcdefghijklmnopqrstuvwxyz", 20).contains("[truncated]"));
}

#[test]
fn file_helpers_resolve_relative_absolute_and_unique_downloads() {
    let temp = tempfile::tempdir().expect("tempdir");
    let session_dir = temp.path();
    let args = WebDiscoverArgs {
        kind: "image".to_string(),
        asset_type: None,
        query: "minji".to_string(),
        include_regex: None,
        exclude_regex: None,
        max_results: 5,
        download_dir: Some("media/image".to_string()),
        min_size: DEFAULT_IMAGE_MIN_SIZE,
        max_size: DEFAULT_MAX_SIZE,
        format_selector: None,
    };
    let output_dir = resolve_download_dir(&args, session_dir).expect("download dir");
    fs::create_dir_all(&output_dir).expect("create output dir");

    assert_eq!(output_dir, session_dir.join("media/image"));
    assert_eq!(
        workspace_relative_path("media/image", session_dir),
        Some(PathBuf::from("media/image"))
    );
    assert!(workspace_relative_path(
        temp.path()
            .parent()
            .unwrap_or(temp.path())
            .to_string_lossy()
            .as_ref(),
        session_dir
    )
    .is_none());

    let first = write_unique_download(&output_dir, "profile", "jpg", b"one").expect("write first");
    let second =
        write_unique_download(&output_dir, "profile", "jpg", b"two").expect("write second");
    assert_eq!(
        first.file_name().and_then(|v| v.to_str()),
        Some("profile.jpg")
    );
    assert_eq!(
        second.file_name().and_then(|v| v.to_str()),
        Some("profile-1.jpg")
    );

    let downloaded = downloaded_file_value(
        &first,
        session_dir,
        "https://example.com/profile.jpg",
        Some("https://example.com/profile"),
        "image",
    );
    assert_eq!(
        downloaded["path"]
            .as_str()
            .map(|value| value.replace('\\', "/")),
        Some("media/image/profile.jpg".to_string())
    );
    assert_eq!(downloaded["content_type"], "image/jpeg");
    assert_eq!(downloaded["size"], 3);

    let scope = web_discover_write_scope(&args, &PathBuf::from("media/image"));
    assert!(scope.starts_with("media/image/.web_discover-image-"));
    assert_eq!(stable_hash("minji"), stable_hash("minji"));
}

#[test]
fn move_unique_download_preserves_existing_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output_dir = temp.path().join("out");
    fs::create_dir_all(&output_dir).expect("create out");
    fs::write(output_dir.join("clip.mp4"), b"existing").expect("existing");
    let temp_source_dir = temp.path().join("tmp");
    fs::create_dir_all(&temp_source_dir).expect("create tmp");
    let source = temp_source_dir.join("download.mp4");
    fs::write(&source, b"new").expect("source");

    let moved = move_unique_download(&source, &output_dir, "clip", "mp4").expect("move");

    assert_eq!(
        moved.file_name().and_then(|v| v.to_str()),
        Some("clip-1.mp4")
    );
    assert_eq!(
        fs::read(output_dir.join("clip.mp4")).expect("read original"),
        b"existing"
    );
    assert_eq!(fs::read(moved).expect("read moved"), b"new");
}

#[test]
fn access_rules_are_empty_without_download_and_scoped_with_download() {
    let temp = tempfile::tempdir().expect("tempdir");

    let empty = access("image minji", temp.path());
    assert!(empty.write_paths.is_empty());

    let scoped = access("image minji --download-dir media/image", temp.path());
    assert_eq!(scoped.read_paths.len(), 0);
    assert_eq!(scoped.write_paths.len(), 1);
    assert!(scoped.write_paths[0].starts_with("media/image/.web_discover-image-"));

    let value_scoped = super::access::access_for_value(
        json!({"kind":"website","query":"rust","downloadDir":"web"}),
        temp.path(),
    );
    assert_eq!(value_scoped.write_paths.len(), 1);
    assert!(value_scoped.write_paths[0].starts_with("web/.web_discover-website-"));
}

#[test]
fn output_summary_handles_strings_records_paths_and_downloads() {
    let records = vec![
        json!("plain text record\nwith newline"),
        json!({"title":"Rust","url":"https://example.com/rust"}),
        json!({"title":"Image","url":"https://example.com/img.jpg","local_path":"media/img.jpg"}),
    ];
    let downloaded = vec![json!({"path":"media/img.jpg","size":123})];

    let summary = summarize_records(&records, &downloaded);

    assert!(summary.contains("1. plain text record with newline"));
    assert!(summary.contains("2. [Rust](https://example.com/rust)"));
    assert!(summary.contains("3. [Image](https://example.com/img.jpg) -> media/img.jpg"));
    assert!(summary.contains("- media/img.jpg (123 bytes)"));
    assert_eq!(summary_text(&json!({"summary_markdown":"ok"})), "ok");
    assert_eq!(summary_text(&json!({})), "");
}

#[test]
fn protocol_health_capabilities_access_and_unknown_are_local() {
    let temp = tempfile::tempdir().expect("tempdir");
    let health = handle_envelope(Envelope {
        kind: "health_check".to_string(),
        payload: json!({}),
    });
    assert!(health.ok);
    assert_eq!(health.output["status"], "ok");

    let capabilities = handle_envelope(Envelope {
        kind: "capabilities".to_string(),
        payload: json!({}),
    });
    assert!(capabilities.ok);
    assert_eq!(capabilities.output["id"], "web_discover");
    assert_eq!(capabilities.output["network"], true);

    let access = handle_envelope(Envelope {
        kind: "access".to_string(),
        payload: json!({
            "session_dir": temp.path(),
            "arguments": "image minji --download-dir media/image"
        }),
    });
    assert!(access.ok);
    assert_eq!(
        access.output["write_paths"].as_array().map(Vec::len),
        Some(1)
    );

    let unknown = handle_envelope(Envelope {
        kind: "unknown".to_string(),
        payload: json!({}),
    });
    assert!(!unknown.ok);
    assert!(unknown.output["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unsupported protocol kind"));
}

#[test]
fn execute_invalid_input_returns_structured_error_without_network() {
    let temp = tempfile::tempdir().expect("tempdir");

    let response = execute("--type archive rust", temp.path(), 0);

    assert!(!response.success);
    assert_eq!(response.exit_code, 1);
    assert!(response.stderr.contains("unsupported web_discover type"));
    assert_eq!(response.output["error"], response.stderr);
    assert!(response.stdout.is_empty());
}

#[test]
fn execute_asset_direct_zip_downloads_and_extracts_into_type_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let zip_bytes = make_test_zip_bytes();
    let (asset_url, server) = spawn_binary_endpoint("application/zip", zip_bytes, "/asset.zip");

    let response = execute(
        &format!("asset 3d {asset_url} --download-dir assets --min-size 1 --max-size 1000000"),
        temp.path(),
        0,
    );
    server.join().expect("asset server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["type"], "asset");
    assert_eq!(response.output["asset_type"], "3d");
    assert_eq!(response.output["searched_sources"][0], "direct_asset_url");
    assert_eq!(response.output["result_count"], 1);
    assert!(response.output["download_dir"]
        .as_str()
        .unwrap_or_default()
        .ends_with("assets"));

    let downloaded = response.output["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert_eq!(downloaded.len(), 2);
    assert!(downloaded.iter().any(|item| {
        item["absolute_path"]
            .as_str()
            .is_some_and(|path| path.ends_with("asset-zip.zip") && PathBuf::from(path).exists())
    }));
    assert!(downloaded.iter().any(|item| {
        item["absolute_path"].as_str().is_some_and(|path| {
            path.ends_with("models\\ship.glb") || path.ends_with("models/ship.glb")
        }) && item["content_type"] == "model/gltf-binary"
    }));
    assert!(temp.path().join("assets").join("3d").exists());
}

#[test]
fn exa_web_results_dedupe_limit_and_skip_metadata_lines() {
    let raw = json!({
        "result": {
            "content": [{
                "type": "text",
                "text": "Title: First Result\nURL: https://example.com/one\nPublished: yesterday\nAuthor: nobody\nHighlights:\nImportant <b>snippet</b>\n---\nTitle: Duplicate\nURL: https://example.com/one\nDuplicate snippet\n---\nTitle: Second Result\nURL: https://example.com/two\nSecond snippet\n---\nTitle: Ignored FTP\nURL: ftp://example.com/file\nNope"
            }]
        }
    })
    .to_string();

    let results = parse_exa_web_results(&raw, 2).expect("exa results");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "First Result");
    assert_eq!(results[0].url, "https://example.com/one");
    assert_eq!(results[0].snippet, "Important snippet");
    assert_eq!(results[0].source, "exa_web");
    assert_eq!(results[1].title, "Second Result");
}

#[test]
fn exa_web_results_report_unparsable_and_empty_payloads() {
    let unparsable = parse_exa_web_results("event: done\ndata: {not-json}", 5)
        .expect_err("unparsable exa payload");
    assert_eq!(unparsable, "exa web search returned no parseable content");

    let empty = parse_exa_web_results(
        &json!({"result": {"content": [{"type": "text", "text": "Title: no url"}]}}).to_string(),
        5,
    )
    .expect_err("empty exa result set");
    assert_eq!(empty, "exa web search returned no usable results");
}

#[test]
fn custom_search_endpoint_posts_query_and_filters_missing_urls() {
    let body = json!({
        "results": [
            {"title": "One", "url": "https://example.com/one", "snippet": "first"},
            {"name": "Two", "link": "https://example.com/two", "description": "second", "sourceUrl": "https://source.example/two"},
            {"title": "Missing URL"},
            {"title": "Over Limit", "url": "https://example.com/three"}
        ]
    })
    .to_string();
    let (endpoint, server) = spawn_json_endpoint(body);
    let client = Client::builder().build().expect("client");

    let results = search_custom_endpoint(&client, &endpoint, "rust query", 3)
        .expect("custom endpoint results");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "One");
    assert_eq!(results[0].url, "https://example.com/one");
    assert_eq!(results[0].snippet, "first");
    assert_eq!(results[0].source, "custom_endpoint");
    assert_eq!(results[1].title, "Two");
    assert_eq!(
        results[1].page_url.as_deref(),
        Some("https://source.example/two")
    );
    server.join().expect("custom endpoint server");
}

#[test]
fn direct_media_results_use_url_titles_and_kind_specific_sources() {
    let results = direct_media_results(
        "audio",
        vec![
            "https://example.com/media/song.mp3".to_string(),
            "https://example.com/".to_string(),
        ],
    );

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "song.mp3");
    assert_eq!(results[0].source, "direct_audio_url");
    assert_eq!(results[1].title, "example.com");
    assert_eq!(results[1].url, "https://example.com/");
}

#[test]
fn duckduckgo_vqd_extraction_accepts_script_json_and_query_shapes() {
    assert_eq!(
        extract_duckduckgo_vqd(r#"var vqd = 'abc-123';"#).as_deref(),
        Some("abc-123")
    );
    assert_eq!(
        extract_duckduckgo_vqd(r#"{"vqd":"json-token"}"#).as_deref(),
        Some("json-token")
    );
    assert_eq!(
        extract_duckduckgo_vqd(r#"/i.js?q=x&vqd=encoded%2Btoken&x=1"#).as_deref(),
        Some("encoded+token")
    );
    assert_eq!(extract_duckduckgo_vqd("no token"), None);
}

fn spawn_json_endpoint(response_body: String) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test endpoint");
    let addr = listener.local_addr().expect("test endpoint addr");
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer).expect("read request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if http_request_complete(&request) {
                break;
            }
        }
        let request_text = String::from_utf8_lossy(&request);
        assert!(request_text.starts_with("POST "), "{request_text}");
        assert!(
            request_text.contains("\"query\":\"rust query\""),
            "{request_text}"
        );
        assert!(request_text.contains("\"max_results\":3"), "{request_text}");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });
    (format!("http://{addr}/search"), handle)
}

fn spawn_binary_endpoint(
    content_type: &'static str,
    body: Vec<u8>,
    path: &'static str,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind binary endpoint");
    let addr = listener.local_addr().expect("binary endpoint addr");
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer).expect("read request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if http_request_complete(&request) {
                break;
            }
        }
        let request_text = String::from_utf8_lossy(&request);
        assert!(
            request_text.starts_with(&format!("GET {path} ")),
            "{request_text}"
        );
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(headers.as_bytes()).expect("write headers");
        stream.write_all(&body).expect("write body");
    });
    (format!("http://{addr}{path}"), handle)
}

fn make_test_zip_bytes() -> Vec<u8> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default();
    writer
        .start_file("models/ship.glb", options)
        .expect("start zip file");
    writer
        .write_all(b"glTF test model")
        .expect("write zip file");
    writer.finish().expect("finish zip").into_inner()
}

fn http_request_complete(data: &[u8]) -> bool {
    let text = String::from_utf8_lossy(data);
    let Some((headers, body)) = text.split_once("\r\n\r\n") else {
        return false;
    };
    let content_length = headers
        .lines()
        .find_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    body.len() >= content_length
}

#[test]
fn duckduckgo_html_parser_normalizes_redirect_urls_and_pairs_snippets() {
    let html = r#"
        <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fdocs%3Fx%3D1&amp;rut=abc">
            <b>Example</b> Docs
        </a>
        <a class="result__snippet">First &amp; useful <em>snippet</em></a>
        <a class="result__a" href="https://second.example/path">Second</a>
    "#;

    let results = parse_duckduckgo_html_results(html, 5);

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Example Docs");
    assert_eq!(results[0].url, "https://example.com/docs?x=1");
    assert_eq!(results[0].snippet, "First & useful snippet");
    assert_eq!(results[0].source, "duckduckgo_html");
    assert_eq!(results[1].snippet, "");
}

#[test]
fn duckduckgo_html_parser_filters_empty_titles_and_non_http_links() {
    let html = r#"
        <a class="result__a" href="javascript:void(0)">Ignored</a>
        <a class="result__a" href="https://example.com/empty"><span></span></a>
        <a class="result__a" href="https://example.com/ok">Accepted</a>
    "#;

    let results = parse_duckduckgo_html_results(html, 10);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Accepted");
    assert_eq!(results[0].url, "https://example.com/ok");
}

#[test]
fn bing_image_page_url_prefers_purl_and_skips_bing_or_direct_media_anchors() {
    let from_param =
        r#"<a href="/images/search?purl=https%3A%2F%2Fsource.example%2Farticle%3Fid%3D1">x</a>"#;
    assert_eq!(
        extract_bing_image_page_url(from_param).as_deref(),
        Some("https://source.example/article?id=1")
    );

    let from_anchor = r#"
        <a href="https://th.bing.com/th/id/example.jpg">thumb</a>
        <a href="https://cdn.example.com/photo.webp">direct media</a>
        <a href="https://source.example.com/article">source</a>
    "#;
    assert_eq!(
        extract_bing_image_page_url(from_anchor).as_deref(),
        Some("https://source.example.com/article")
    );
}

#[test]
fn bing_image_title_uses_valid_alt_then_page_host_then_media_host() {
    assert_eq!(
        extract_bing_image_title(
            r#"<img alt="  Sample &amp; Image  ">"#,
            Some("https://page.example/article"),
            "https://media.example/photo.jpg",
        ),
        "Sample & Image"
    );
    assert_eq!(
        extract_bing_image_title(
            r#"<img alt="Image result for sample">"#,
            Some("https://page.example/article"),
            "https://media.example/photo.jpg",
        ),
        "page.example"
    );
    assert_eq!(
        extract_bing_image_title("", None, "https://media.example/photo.jpg"),
        "media.example"
    );
}

#[test]
fn util_helpers_cover_text_decoding_filename_content_type_and_truncation() {
    assert_eq!(
        split_cli_words(r#"cmd "two words" 'three words'"#),
        vec!["cmd", "two words", "three words"]
    );
    assert_eq!(
        clean_text("<b>Hello</b>&nbsp;&amp; <i>world</i>"),
        "Hello & world"
    );
    assert_eq!(
        html_unescape("&lt;a&gt;&quot;x&#39;&quot;&lt;/a&gt;"),
        "<a>\"x'\"</a>"
    );
    assert_eq!(
        json_unescape(r#"https:\/\/example.com\/a.jpg"#),
        "https://example.com/a.jpg"
    );
    assert_eq!(percent_decode("a%2Fb+c%ZZ"), "a/b c%ZZ");
    assert_eq!(
        safe_filename("  A/B:C*D?E F G H I J K  "),
        "A-B-C-D-E-F-G-H"
    );
    assert_eq!(safe_filename("!!!"), "result");
    assert_eq!(
        extension_from_url("https://x.test/a.JPEG?size=1"),
        Some("jpg")
    );
    assert_eq!(
        content_type_for_path(std::path::Path::new("clip.webm"), "video"),
        "video/webm"
    );
    assert_eq!(
        content_type_for_path(std::path::Path::new("page.unknown"), "website"),
        "text/markdown"
    );
    assert_eq!(truncate_chars("ab😀cd", 3), "ab😀");
    assert_eq!(middle_truncate_chars("abcdef", 20), "abcdef");
    assert!(middle_truncate_chars("abcdefghijklmnopqrstuvwxyz", 12).len() <= 12);
}

#[test]
fn file_helpers_resolve_download_scopes_and_unique_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    let args = WebDiscoverArgs {
        kind: "image".to_string(),
        asset_type: None,
        query: "sample query".to_string(),
        include_regex: None,
        exclude_regex: None,
        max_results: 2,
        download_dir: None,
        min_size: 1,
        max_size: 100,
        format_selector: None,
    };

    let resolved = resolve_download_dir(&args, dir.path()).expect("download dir");
    assert!(resolved.ends_with("media/image"));
    let scope = web_discover_write_scope(&args, std::path::Path::new("media/image"));
    assert!(scope.starts_with("media/image/.web_discover-image-"));
    assert_eq!(stable_hash("same"), stable_hash("same"));
    assert_ne!(stable_hash("same"), stable_hash("different"));

    fs::create_dir_all(&resolved).expect("create output dir");
    let first = write_unique_download(&resolved, "sample", "jpg", b"one").expect("first write");
    let second = write_unique_download(&resolved, "sample", "jpg", b"two").expect("second write");
    assert!(first.ends_with("sample.jpg"));
    assert!(second.ends_with("sample-1.jpg"));

    let value = downloaded_file_value(
        &first,
        dir.path(),
        "https://source.example/image.jpg",
        Some("https://source.example/page"),
        "image",
    );
    let relative_path = value["path"].as_str().expect("relative path");
    assert!(relative_path.ends_with("sample.jpg"));
    assert!(
        relative_path == "media/image/sample.jpg" || relative_path == r"media/image\sample.jpg",
        "{relative_path}"
    );
    assert_eq!(value["name"], "sample.jpg");
    assert_eq!(value["source_page_url"], "https://source.example/page");
    assert_eq!(value["content_type"], "image/jpeg");
}

#[test]
fn move_unique_download_keeps_existing_files_and_reports_missing_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = dir.path().join("out");
    fs::create_dir_all(&output).expect("create output");
    fs::write(output.join("clip.mp4"), b"existing").expect("existing file");
    let source = dir.path().join("clip.tmp");
    fs::write(&source, b"new").expect("source file");

    let moved = move_unique_download(&source, &output, "clip", "mp4").expect("move unique");
    assert!(moved.ends_with("clip-1.mp4"));
    assert!(!source.exists());
    assert_eq!(fs::read(&moved).expect("read moved"), b"new");

    let missing = dir.path().join("missing.tmp");
    let error = move_unique_download(&missing, &output, "missing", "mp4")
        .expect_err("missing source should fail");
    assert!(error.contains("failed to move downloaded media"));
}

#[test]
fn ytdlp_defaults_and_candidate_rank_prefer_expected_media() {
    let dir = tempfile::tempdir().expect("tempdir");
    let small_audio = dir.path().join("small.mp3");
    let large_audio = dir.path().join("large.mp3");
    let video = dir.path().join("clip.mp4");
    let unknown = dir.path().join("file.bin");
    fs::write(&small_audio, vec![0_u8; 10]).expect("small audio");
    fs::write(&large_audio, vec![0_u8; 20]).expect("large audio");
    fs::write(&video, vec![0_u8; 15]).expect("video");
    fs::write(&unknown, vec![0_u8; 30]).expect("unknown");

    assert_eq!(default_ytdlp_format("audio"), "bestaudio/best");
    assert!(default_ytdlp_format("video").contains("height<=540"));
    assert!(
        ytdlp_download_candidate_rank(&large_audio, "audio")
            < ytdlp_download_candidate_rank(&small_audio, "audio")
    );
    assert!(
        ytdlp_download_candidate_rank(&video, "video")
            < ytdlp_download_candidate_rank(&large_audio, "video")
    );
    assert!(
        ytdlp_download_candidate_rank(&video, "video")
            < ytdlp_download_candidate_rank(&unknown, "video")
    );
}

#[test]
fn media_url_normalization_rejects_inline_and_non_media_values() {
    assert!(normalize_media_url("", "https://example.com/base/").is_none());
    assert!(
        normalize_media_url("data:image/png;base64,abc", "https://example.com/base/").is_none()
    );
    assert!(
        normalize_media_url("blob:https://example.com/id", "https://example.com/base/").is_none()
    );
    assert!(normalize_media_url("/docs/page.html", "https://example.com/base/").is_none());
    assert_eq!(
        normalize_media_url(
            "//cdn.example.com/photo.jpg?x=1",
            "https://example.com/base/"
        )
        .as_deref(),
        Some("https://cdn.example.com/photo.jpg?x=1")
    );
    assert_eq!(
        normalize_media_url("../media/photo.webp#hero", "https://example.com/docs/page/")
            .as_deref(),
        Some("https://example.com/docs/media/photo.webp#hero")
    );
}

#[test]
fn extract_media_links_deduplicates_srcset_meta_and_escaped_urls() {
    let html = r#"
        <img src="/media/a.jpg">
        <img data-src="/media/a.jpg">
        <source srcset="/media/b.webp 1x, /media/c.png 2x">
        <meta property="og:video" content="https:\/\/cdn.example.com\/clip.mp4">
        <a href="/download/file.txt">ignored</a>
        <script>var poster = "https:\/\/cdn.example.com\/clip.mp4";</script>
    "#;

    let links = extract_media_links(html, "https://example.com/page/");

    assert_eq!(
        links,
        vec![
            "https://example.com/media/a.jpg",
            "https://example.com/media/b.webp",
            "https://example.com/media/c.png",
            "https://cdn.example.com/clip.mp4",
        ]
    );
}

#[test]
fn markdown_normalization_and_title_helpers_trim_noise() {
    assert_eq!(
        normalize_markdown("Title &amp; Value\n\n\nBody&nbsp;text\n"),
        "Title & Value\n\nBody text"
    );
    assert_eq!(
        extract_title("<title>  <b>Docs</b> &amp; API </title>").as_deref(),
        Some("Docs & API")
    );
    assert_eq!(extract_title("<title>   </title>"), None);
    assert_eq!(
        title_from_url("https://example.com/docs/guide.html?x=1"),
        "guide.html"
    );
    assert_eq!(title_from_url("not a url"), "Webpage");
}

fn result(title: &str, url: &str, snippet: &str, source: &str) -> SearchResult {
    SearchResult {
        title: title.to_string(),
        url: url.to_string(),
        page_url: None,
        snippet: snippet.to_string(),
        source: source.to_string(),
    }
}
