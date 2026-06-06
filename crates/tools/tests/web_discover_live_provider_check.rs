use code_tools::commands;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

struct EnvSnapshot {
    values: Vec<(&'static str, Option<String>)>,
}

impl EnvSnapshot {
    fn capture(keys: &[&'static str]) -> Self {
        Self {
            values: keys
                .iter()
                .map(|key| (*key, std::env::var(key).ok()))
                .collect(),
        }
    }

    fn restore(self) {
        for (key, value) in self.values {
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

fn configure_provider(provider: &str) {
    unsafe {
        std::env::remove_var("TURA_WEB_DISCOVER_ENDPOINT");
        std::env::remove_var("TURA_WEB_SEARCH_ENDPOINT");
        std::env::remove_var("TURA_IMAGE_SEARCH_ENDPOINT");
        std::env::remove_var("TURA_DUCKDUCKGO_SEARCH_ENDPOINT");
        std::env::remove_var("TURA_DUCKDUCKGO_HTML_ENDPOINT");
        std::env::remove_var("TURA_DUCKDUCKGO_IMAGE_PAGE_ENDPOINT");
        std::env::remove_var("TURA_DUCKDUCKGO_IMAGE_SEARCH_ENDPOINT");
        std::env::remove_var("TURA_DUCKDUCKGO_IMAGES_ENDPOINT");
        std::env::set_var("TURA_IMAGE_FALLBACK_PROVIDER", "duckduckgo");
        std::env::set_var("TURA_DUCKDUCKGO_IMAGE_BACKEND", "auto");
        match provider {
            "brave" => {
                std::env::remove_var("TURA_BRAVE_SEARCH_DISABLED");
                std::env::remove_var("TURA_EXA_SEARCH_DISABLED");
            }
            "exa" => {
                std::env::set_var("TURA_BRAVE_SEARCH_DISABLED", "1");
                std::env::remove_var("TURA_EXA_SEARCH_DISABLED");
            }
            "duckduckgo" => {
                std::env::set_var("TURA_BRAVE_SEARCH_DISABLED", "1");
                std::env::set_var("TURA_EXA_SEARCH_DISABLED", "1");
            }
            other => panic!("unknown provider {other}"),
        }
    }
}

fn run_case(session_dir: &Path, provider: &str, name: &str, command_line: String) -> Value {
    configure_provider(provider);
    let started = std::time::Instant::now();
    let response = commands::execute("web_discover", &command_line, session_dir, 90);
    json!({
        "provider": provider,
        "case": name,
        "success": response.success,
        "exit_code": response.exit_code,
        "elapsed_ms": started.elapsed().as_millis(),
        "stderr": response.stderr,
        "stdout": response.stdout,
        "output": response.output,
    })
}

#[test]
#[ignore]
fn live_provider_matrix_downloads_docs_and_images() {
    let env = EnvSnapshot::capture(&[
        "TURA_WEB_DISCOVER_ENDPOINT",
        "TURA_WEB_SEARCH_ENDPOINT",
        "TURA_IMAGE_SEARCH_ENDPOINT",
        "TURA_BRAVE_SEARCH_DISABLED",
        "TURA_EXA_SEARCH_DISABLED",
        "TURA_IMAGE_FALLBACK_PROVIDER",
        "TURA_DUCKDUCKGO_IMAGE_BACKEND",
        "TURA_DUCKDUCKGO_SEARCH_ENDPOINT",
        "TURA_DUCKDUCKGO_HTML_ENDPOINT",
        "TURA_DUCKDUCKGO_IMAGE_PAGE_ENDPOINT",
        "TURA_DUCKDUCKGO_IMAGE_SEARCH_ENDPOINT",
        "TURA_DUCKDUCKGO_IMAGES_ENDPOINT",
    ]);

    let session_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("web-discover-provider-checks");
    let _ = std::fs::remove_dir_all(&session_dir);
    std::fs::create_dir_all(&session_dir).expect("create session dir");

    let mut results = Vec::new();
    for provider in ["brave", "exa", "duckduckgo"] {
        let docs_dir = format!("docs/{provider}/replicate-z-image-turbo");
        results.push(run_case(
            &session_dir,
            provider,
            "replicate_z_image_turbo_api_docs",
            format!(
                "web_discover website \"replicate z image turbo api documentation\" --max-results=3 --download-dir={docs_dir}"
            ),
        ));

        let image_dir = format!("media/{provider}/xuanzang-portrait");
        results.push(run_case(
            &session_dir,
            provider,
            "xuanzang_portrait_image",
            format!(
                "web_discover image \"唐玄奘 画像 portrait Xuanzang\" --max-results=3 --download-dir={image_dir} --min-size=10000 --max-size=10000000"
            ),
        ));
    }

    let summary = json!({
        "session_dir": session_dir.display().to_string(),
        "results": results,
    });
    std::fs::write(
        session_dir.join("summary.json"),
        serde_json::to_string_pretty(&summary).expect("summary json"),
    )
    .expect("write summary");
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).expect("summary should serialize")
    );

    env.restore();
}

#[test]
#[ignore]
fn live_newjeans_profile_fetch_preserves_media_links() {
    let session_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("web-discover-newjeans-profile");
    let _ = std::fs::remove_dir_all(&session_dir);
    std::fs::create_dir_all(&session_dir).expect("create session dir");

    let response = commands::execute(
        "web_discover",
        "web_discover website \"https://www.newjeans.jp/profile/MINJI\" --download-dir docs/newjeans",
        &session_dir,
        90,
    );
    assert!(response.success, "{}", response.stderr);

    let downloaded = response.output["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert_eq!(downloaded.len(), 1);
    let relative = downloaded[0]["path"].as_str().expect("path");
    let markdown = std::fs::read_to_string(session_dir.join(relative)).expect("read markdown");

    assert!(markdown.contains("Media links"), "{markdown}");
    assert!(
        markdown.contains("profile_member") || markdown.contains(".webp"),
        "{markdown}"
    );
    assert!(!markdown.contains("window.__reactRouterContext"));
    println!("{}", session_dir.display());
    println!("{}", relative);
}

#[test]
#[ignore]
fn live_direct_image_and_youtube_url_download() {
    let session_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("web-discover-direct-media");
    let _ = std::fs::remove_dir_all(&session_dir);
    std::fs::create_dir_all(&session_dir).expect("create session dir");

    let page_response = commands::execute(
        "web_discover",
        "web_discover website \"https://www.newjeans.jp/profile/MINJI\" --download-dir docs/newjeans",
        &session_dir,
        90,
    );
    assert!(page_response.success, "{}", page_response.stderr);
    let page_path = page_response.output["downloaded_files"][0]["path"]
        .as_str()
        .expect("page path");
    let markdown = std::fs::read_to_string(session_dir.join(page_path)).expect("read page md");
    assert!(markdown.contains("profile_member"), "{markdown}");

    let profile_image_url =
        "https://officialsite.cds-jp.online/prod/profile_member/105/158/2c38bd5497b94e38aba150b784a7de87.webp";
    let image_command = format!(
        "web_discover image \"{profile_image_url}\" --download-dir media/newjeans --min-size=10000"
    );
    let image_response = commands::execute("web_discover", &image_command, &session_dir, 90);
    assert!(image_response.success, "{}", image_response.stderr);
    let image_files = image_response.output["downloaded_files"]
        .as_array()
        .expect("image downloads");
    assert_eq!(image_files.len(), 1, "{}", image_response.output);

    let video_response = commands::execute(
        "web_discover",
        "web_discover video \"https://www.youtube.com/watch?v=7Bu7KfFtGsQ\" --download-dir media/video --format \"best[height<=360][ext=mp4]/best[height<=360]/best\" --max-size=80000000",
        &session_dir,
        180,
    );
    assert!(video_response.success, "{}", video_response.stderr);
    let video_files = video_response.output["downloaded_files"]
        .as_array()
        .expect("video downloads");
    assert_eq!(video_files.len(), 1, "{}", video_response.output);

    println!("{}", session_dir.display());
    println!(
        "{}",
        serde_json::to_string_pretty(&image_response.output)
            .expect("image output should serialize")
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&video_response.output)
            .expect("video output should serialize")
    );
}
