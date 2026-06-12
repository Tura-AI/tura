use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use tura_command_web_discover::{access, execute};

#[test]
fn web_discover_business_flow_fetches_loopback_page_and_saves_markdown() {
    let dir = tempfile::tempdir().expect("tempdir");
    let server = spawn_page_server("Tura Local Business Page");
    let command_line = format!(
        "web_discover website {} --download-dir discoveries --max-results 1",
        server.url
    );

    let access = access(&command_line, dir.path());
    assert!(access.read_paths.is_empty());
    assert_eq!(access.write_paths.len(), 1);
    assert!(
        normalize_path(&access.write_paths[0]).starts_with("discoveries/.web_discover-website-")
    );
    assert!(!access.workspace_write);

    let response = execute(&command_line, dir.path(), 10);
    assert!(response.success);
    assert_eq!(response.exit_code, 0);
    assert!(response.stderr.is_empty());
    assert!(response.changes.is_empty());
    assert!(response.stdout.contains("Tura Local Business Page"));

    assert_eq!(response.output["direct_fetch"], true);
    assert_eq!(response.output["saved"], true);
    assert_eq!(
        response.output["result_count"], 1,
        "unexpected website output: {} stderr: {}",
        response.output, response.stderr
    );
    assert_eq!(
        response.output["results"][0]["title"],
        "Tura Local Business Page"
    );
    assert_eq!(response.output["results"][0]["source"], "direct_url");
    assert_eq!(
        response.output["downloaded_files"]
            .as_array()
            .unwrap_or(&Vec::new())
            .len(),
        1
    );
    let local_path = response.output["downloaded_files"][0]["path"]
        .as_str()
        .expect("downloaded local path");
    let saved = dir.path().join(local_path);
    assert!(
        saved.exists(),
        "downloaded markdown should exist at {}",
        saved.display()
    );
    let saved_text = std::fs::read_to_string(&saved).expect("saved markdown");
    assert!(saved_text.contains("# Tura Local Business Page"));
    assert!(saved_text.contains("business sentinel paragraph"));

    let request = server.join();
    assert!(request.starts_with("GET /article "));
}

#[test]
fn web_discover_business_protocol_binary_accepts_json_arguments_and_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let server = spawn_page_server("Protocol Local Page");
    let response = run_protocol(json!({
        "kind": "execute",
        "payload": {
            "session_dir": dir.path().display().to_string(),
            "arguments": {
                "type": "website",
                "query": server.url,
                "download_dir": "protocol-pages",
                "max_results": 1
            }
        }
    }));

    assert_eq!(response["ok"], true);
    assert_eq!(response["success"], true);
    assert_eq!(response["exit_code"], 0);
    assert_eq!(response["output"]["direct_fetch"], true);
    assert_eq!(
        response["output"]["results"][0]["title"],
        "Protocol Local Page"
    );
    let saved_path = response["output"]["downloaded_files"][0]["path"]
        .as_str()
        .expect("saved path");
    assert!(dir.path().join(saved_path).exists());
    assert!(server.join().starts_with("GET /article "));

    let unsupported = run_protocol(json!({
        "kind": "execute",
        "payload": {
            "session_dir": dir.path().display().to_string(),
            "arguments": {"type": "archive", "query": "anything"}
        }
    }));
    assert_eq!(unsupported["ok"], true);
    assert_eq!(unsupported["success"], false);
    assert_eq!(unsupported["exit_code"], 1);
    assert!(unsupported["stderr"]
        .as_str()
        .is_some_and(|stderr| stderr.contains("unsupported web_discover type")));
}

#[test]
fn web_discover_business_protocol_health_capabilities_and_access_are_stable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let health = run_protocol(json!({
        "kind": "health_check",
        "payload": {}
    }));
    assert_eq!(health["ok"], true);
    assert_eq!(health["output"]["status"], "ok");

    let capabilities = run_protocol(json!({
        "kind": "capabilities",
        "payload": {}
    }));
    assert_eq!(capabilities["ok"], true);
    assert_eq!(capabilities["output"]["id"], "web_discover");
    assert_eq!(capabilities["output"]["supports_macro_command"], true);
    assert_eq!(capabilities["output"]["network"], true);

    let access = run_protocol(json!({
        "kind": "access",
        "payload": {
            "session_dir": dir.path().display().to_string(),
            "arguments": {
                "type": "website",
                "query": "https://example.invalid/docs",
                "downloadDir": "saved/pages"
            }
        }
    }));
    assert_eq!(access["ok"], true);
    assert!(access["output"]["read_paths"]
        .as_array()
        .is_some_and(|paths| paths.is_empty()));
    assert_eq!(access["output"]["workspace_write"], false);
    assert!(
        access["output"]["write_paths"][0].as_str().is_some_and(
            |path| normalize_path(path).starts_with("saved/pages/.web_discover-website-")
        )
    );
}

#[test]
fn web_discover_business_flow_downloads_direct_image_and_applies_size_limits() {
    let dir = tempfile::tempdir().expect("tempdir");
    let image_bytes = tiny_png_bytes();
    let image = spawn_binary_response_server(200, "image/png", image_bytes.to_vec(), "/asset.png");
    let command_line = format!(
        "web_discover image {} --download-dir media-out --min-size 1 --max-size 100000",
        image.url
    );

    let access = access(&command_line, dir.path());
    assert!(access.read_paths.is_empty());
    assert_eq!(access.write_paths.len(), 1);
    assert!(normalize_path(&access.write_paths[0]).starts_with("media-out/.web_discover-image-"));

    let response = execute(&command_line, dir.path(), 10);
    assert!(
        response.success,
        "direct image download failed: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.output["type"], "image");
    assert_eq!(response.output["saved"], true);
    assert_eq!(
        response.output["result_count"], 1,
        "unexpected image download output: {} stderr: {}",
        response.output, response.stderr
    );
    assert_eq!(response.output["results"][0]["file_type"], "image");
    assert_eq!(response.output["results"][0]["source"], "direct_image_url");
    assert_eq!(response.output["downloaded_files"][0]["file_type"], "image");
    assert_eq!(
        response.output["downloaded_files"][0]["content_type"],
        "image/png"
    );
    assert_eq!(
        response.output["downloaded_files"][0]["size"],
        image_bytes.len() as u64
    );
    let downloaded_path = response.output["downloaded_files"][0]["path"]
        .as_str()
        .expect("download path");
    let downloaded_bytes = std::fs::read(dir.path().join(downloaded_path)).expect("saved image");
    assert_eq!(downloaded_bytes, image_bytes);
    assert!(response.stdout.contains("downloaded:"));
    assert!(image.join().starts_with("GET /asset.png "));

    let filtered =
        spawn_binary_response_server(200, "image/png", image_bytes.to_vec(), "/small.png");
    let command_line = format!(
        "web_discover image {} --download-dir filtered --min-size {} --max-size 100000",
        filtered.url,
        image_bytes.len() + 1
    );
    let response = execute(&command_line, dir.path(), 10);
    assert!(
        response.success,
        "size-filtered image flow should be successful: {}",
        response.stderr
    );
    assert_eq!(response.output["result_count"], 0);
    assert_eq!(response.output["downloaded_files"], json!([]));
    let filtered_dir = dir.path().join("filtered");
    assert!(filtered_dir.exists());
    assert!(
        std::fs::read_dir(&filtered_dir)
            .expect("filtered dir")
            .next()
            .is_none(),
        "size-filtered download directory should remain empty"
    );
    assert!(filtered.join().starts_with("GET /small.png "));
}

#[test]
fn web_discover_business_flow_downloads_multiple_direct_images_concurrently_and_keeps_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let server = spawn_multi_asset_server(vec![
        AssetResponse::ok("/first.png", "image/png", b"first image bytes".to_vec()),
        AssetResponse::status("/missing.png", 404, "image/png", b"missing".to_vec()),
        AssetResponse::ok("/second.jpg", "image/jpeg", b"second image bytes".to_vec()),
        AssetResponse::ok("/third.webp", "image/webp", b"third image bytes".to_vec()),
    ]);
    let query = format!(
        "{} {} {} {}",
        server.url_for("/first.png"),
        server.url_for("/missing.png"),
        server.url_for("/second.jpg"),
        server.url_for("/third.webp")
    );
    let command_line = json!({
        "type": "image",
        "query": query,
        "download_dir": "multi-image-out",
        "min_size": 1,
        "max_size": 100000,
        "max_results": 4
    })
    .to_string();

    let response = execute(&command_line, dir.path(), 10);

    assert!(
        response.success,
        "multi-image direct download should tolerate one failed image: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.output["type"], "image");
    assert_eq!(response.output["saved"], true);
    assert_eq!(
        response.output["result_count"], 3,
        "failed image fetches should be skipped without failing the whole command: {}",
        response.output
    );
    let records = response.output["results"]
        .as_array()
        .expect("image records");
    assert_eq!(records.len(), 3);
    assert!(records[0]["url"]
        .as_str()
        .is_some_and(|url| url.ends_with("/first.png")));
    assert!(records[1]["url"]
        .as_str()
        .is_some_and(|url| url.ends_with("/second.jpg")));
    assert!(records[2]["url"]
        .as_str()
        .is_some_and(|url| url.ends_with("/third.webp")));
    assert_eq!(records[0]["source"], "direct_image_url");
    assert_eq!(records[1]["source"], "direct_image_url");
    assert_eq!(records[2]["source"], "direct_image_url");

    let downloaded = response.output["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert_eq!(downloaded.len(), 3);
    let downloaded_paths = downloaded
        .iter()
        .map(|item| item["path"].as_str().expect("download path").to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        downloaded_paths
            .iter()
            .map(|path| {
                Path::new(path)
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_string()
            })
            .collect::<Vec<_>>(),
        vec!["png", "jpg", "webp"]
    );
    assert_eq!(
        std::fs::read(dir.path().join(&downloaded_paths[0])).expect("first image"),
        b"first image bytes"
    );
    assert_eq!(
        std::fs::read(dir.path().join(&downloaded_paths[1])).expect("second image"),
        b"second image bytes"
    );
    assert_eq!(
        std::fs::read(dir.path().join(&downloaded_paths[2])).expect("third image"),
        b"third image bytes"
    );
    assert!(!downloaded_paths
        .iter()
        .any(|path| normalize_path(path).contains("missing")));
    assert!(response.stdout.contains("downloaded:"));

    let requests = server.join();
    assert_eq!(requests.len(), 4);
    assert!(requests
        .iter()
        .any(|request| request.starts_with("GET /first.png ")));
    assert!(requests
        .iter()
        .any(|request| request.starts_with("GET /missing.png ")));
    assert!(requests
        .iter()
        .any(|request| request.starts_with("GET /second.jpg ")));
    assert!(requests
        .iter()
        .any(|request| request.starts_with("GET /third.webp ")));
}

#[test]
fn web_discover_business_flow_invalid_filter_returns_structured_error_without_download_side_effects(
) {
    let dir = tempfile::tempdir().expect("tempdir");
    let command_line = r#"{
        "type": "website",
        "query": "local invalid regex",
        "download_dir": "should-not-exist",
        "include_regex": "[unterminated"
    }"#;

    let response = execute(command_line, dir.path(), 10);

    assert!(!response.success);
    assert_eq!(response.exit_code, 1);
    assert!(response.stdout.is_empty());
    assert!(
        response.stderr.contains("invalid include_regex:"),
        "unexpected stderr: {}",
        response.stderr
    );
    assert_eq!(response.output["error"], response.stderr);
    assert!(
        !dir.path().join("should-not-exist").exists(),
        "filter validation must fail before creating the download directory"
    );
}

#[test]
fn web_discover_business_flow_downloads_direct_audio_with_local_downloader() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let fake_ytdlp = write_fake_ytdlp(dir.path());
    let previous_ytdlp = std::env::var_os("TURA_WEB_DISCOVER_YTDLP");
    std::env::set_var("TURA_WEB_DISCOVER_YTDLP", &fake_ytdlp);

    let command_line = r#"{"type":"audio","query":"\"https://www.bilibili.com/video/BV1xx411c7mD\"","download_dir":"audio-out","min_size":1,"max_size":100000}"#;
    let access = access(command_line, dir.path());
    assert!(access.read_paths.is_empty());
    assert_eq!(access.write_paths.len(), 1);
    assert!(normalize_path(&access.write_paths[0]).starts_with("audio-out/.web_discover-audio-"));

    let response = execute(command_line, dir.path(), 10);

    restore_env("TURA_WEB_DISCOVER_YTDLP", previous_ytdlp);

    assert!(
        response.success,
        "direct audio download should use local fake yt-dlp: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.output["type"], "audio");
    assert_eq!(response.output["saved"], true);
    assert_eq!(
        response.output["result_count"], 1,
        "unexpected audio download output: {} stderr: {}",
        response.output, response.stderr
    );
    assert_eq!(response.output["results"][0]["file_type"], "audio");
    assert_eq!(response.output["results"][0]["source"], "direct_audio_url");
    assert_eq!(response.output["downloaded_files"][0]["file_type"], "audio");
    assert_eq!(
        response.output["downloaded_files"][0]["content_type"],
        "audio/mpeg"
    );
    let downloaded_path = response.output["downloaded_files"][0]["path"]
        .as_str()
        .expect("download path");
    let downloaded_bytes = std::fs::read(dir.path().join(downloaded_path)).expect("saved audio");
    assert_eq!(downloaded_bytes, b"fake local audio bytes");
    assert!(response.stdout.contains("downloaded:"));
}

#[test]
fn web_discover_business_flow_downloads_direct_video_and_uses_unique_names_on_repeated_runs() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let fake_ytdlp = write_fake_ytdlp(dir.path());
    let previous_ytdlp = std::env::var_os("TURA_WEB_DISCOVER_YTDLP");
    std::env::set_var("TURA_WEB_DISCOVER_YTDLP", &fake_ytdlp);

    let command_line = r#"{"type":"video","query":"\"https://www.youtube.com/watch?v=localBusinessVideo\"","download_dir":"video out","min_size":1,"max_size":100000}"#;
    let access = access(command_line, dir.path());
    assert!(access.read_paths.is_empty());
    assert_eq!(access.write_paths.len(), 1);
    assert!(normalize_path(&access.write_paths[0]).starts_with("video out/.web_discover-video-"));

    let first = execute(command_line, dir.path(), 10);
    let second = execute(command_line, dir.path(), 10);

    restore_env("TURA_WEB_DISCOVER_YTDLP", previous_ytdlp);

    assert!(
        first.success,
        "first direct video download should use fake yt-dlp: {}",
        first.stderr
    );
    assert!(
        second.success,
        "second direct video download should use fake yt-dlp: {}",
        second.stderr
    );
    assert_eq!(first.output["type"], "video");
    assert_eq!(second.output["type"], "video");
    assert_eq!(first.output["downloaded_files"][0]["file_type"], "video");
    assert_eq!(second.output["downloaded_files"][0]["file_type"], "video");
    assert_eq!(
        first.output["downloaded_files"][0]["content_type"],
        "video/mp4"
    );
    assert_eq!(
        second.output["downloaded_files"][0]["content_type"],
        "video/mp4"
    );

    let first_path = first.output["downloaded_files"][0]["path"]
        .as_str()
        .expect("first video path");
    let second_path = second.output["downloaded_files"][0]["path"]
        .as_str()
        .expect("second video path");
    assert_ne!(
        normalize_path(first_path),
        normalize_path(second_path),
        "repeated downloads with the same title/id must not overwrite prior media"
    );
    assert!(normalize_path(second_path).ends_with("-1.mp4"));
    assert_eq!(
        std::fs::read(dir.path().join(first_path)).expect("first saved video"),
        b"fake local video bytes"
    );
    assert_eq!(
        std::fs::read(dir.path().join(second_path)).expect("second saved video"),
        b"fake local video bytes"
    );
    assert!(first.stdout.contains("downloaded:"));
    assert!(second.stdout.contains("downloaded:"));
}

#[test]
fn web_discover_business_flow_fetches_plain_text_and_records_failed_fetch_without_public_fallback()
{
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let previous = std::env::var_os("TURA_WEB_READER_DISABLED");
    std::env::set_var("TURA_WEB_READER_DISABLED", "true");

    let dir = tempfile::tempdir().expect("tempdir");
    let plain = spawn_response_server(
        200,
        "text/plain; charset=utf-8",
        "Plain Local Page\n\nplain business content ".repeat(80),
        "/plain",
    );
    let command_line = format!(
        "web_discover website {} --download-dir plain-pages --max-results 1",
        plain.url
    );
    let response = execute(&command_line, dir.path(), 10);
    assert!(
        response.success,
        "plain text fetch failed: {}",
        response.stderr
    );
    assert_eq!(response.output["direct_fetch"], true);
    assert_eq!(
        response.output["results"][0]["content_type"],
        "text/plain; charset=utf-8"
    );
    assert_eq!(response.output["results"][0]["fetch_mode"], "primary");
    let plain_path = response.output["downloaded_files"][0]["path"]
        .as_str()
        .expect("plain markdown path");
    let plain_saved = std::fs::read_to_string(dir.path().join(plain_path)).expect("plain saved");
    assert!(plain_saved.contains("Plain Local Page"));
    assert!(plain.join().starts_with("GET /plain "));

    let failing = spawn_response_server(
        500,
        "text/html; charset=utf-8",
        "<html><head><title>Broken</title></head><body>broken</body></html>".to_string(),
        "/broken",
    );
    let command_line = format!(
        "web_discover website {} --download-dir failed-pages --max-results 1",
        failing.url
    );
    let response = execute(&command_line, dir.path(), 10);
    assert!(
        response.success,
        "failed fetch should still produce a saved record"
    );
    assert_eq!(response.output["results"][0]["fetch_mode"], "failed");
    assert_eq!(response.output["results"][0]["content_type"], "");
    let failed_path = response.output["downloaded_files"][0]["path"]
        .as_str()
        .expect("failed markdown path");
    let failed_saved = std::fs::read_to_string(dir.path().join(failed_path)).expect("failed saved");
    assert!(failed_saved.contains("Source:"));
    assert!(failing.join().starts_with("GET /broken "));

    restore_env("TURA_WEB_READER_DISABLED", previous);
}

#[test]
fn web_discover_business_flow_uses_local_reader_fallback_when_primary_content_is_too_short() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let previous_disabled = std::env::var_os("TURA_WEB_READER_DISABLED");
    let previous_endpoint = std::env::var_os("TURA_WEB_READER_ENDPOINT");
    let previous_min = std::env::var_os("TURA_WEB_READER_MIN_TEXT_CHARS");

    let dir = tempfile::tempdir().expect("tempdir");
    let server = spawn_reader_fallback_server();
    std::env::remove_var("TURA_WEB_READER_DISABLED");
    std::env::set_var(
        "TURA_WEB_READER_ENDPOINT",
        format!("http://{}/reader?source=", server.addr),
    );
    std::env::set_var("TURA_WEB_READER_MIN_TEXT_CHARS", "200");

    let command_line = format!(
        "web_discover website {} --download-dir reader-pages --max-results 1",
        server.url
    );
    let response = execute(&command_line, dir.path(), 10);

    restore_env("TURA_WEB_READER_DISABLED", previous_disabled);
    restore_env("TURA_WEB_READER_ENDPOINT", previous_endpoint);
    restore_env("TURA_WEB_READER_MIN_TEXT_CHARS", previous_min);

    assert!(
        response.success,
        "reader fallback flow should succeed: {}",
        response.stderr
    );
    assert_eq!(response.output["direct_fetch"], true);
    assert_eq!(
        response.output["results"][0]["fetch_mode"],
        "reader_fallback"
    );
    assert_eq!(
        response.output["results"][0]["title"],
        "Reader Fallback Business Page"
    );
    assert_eq!(
        response.output["results"][0]["content_type"],
        "text/markdown; charset=utf-8"
    );

    let saved_path = response.output["downloaded_files"][0]["path"]
        .as_str()
        .expect("reader fallback markdown path");
    let saved = std::fs::read_to_string(dir.path().join(saved_path)).expect("reader saved");
    assert!(saved.contains("# Reader Fallback Business Page"));
    assert!(saved.contains("reader fallback business body"));
    assert!(!saved.contains("tiny primary"));

    let requests = server.join();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /short "));
    assert!(requests[1].starts_with("GET /reader?source=http://"));
}

#[test]
fn web_discover_business_flow_records_truncated_response_body_without_hanging_or_public_fallback() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let previous_disabled = std::env::var_os("TURA_WEB_READER_DISABLED");
    std::env::set_var("TURA_WEB_READER_DISABLED", "true");

    let dir = tempfile::tempdir().expect("tempdir");
    let server = spawn_truncated_response_server();
    let command_line = format!(
        "web_discover website {} --download-dir malformed-pages --max-results 1",
        server.url
    );
    let response = execute(&command_line, dir.path(), 10);

    restore_env("TURA_WEB_READER_DISABLED", previous_disabled);

    assert!(
        response.success,
        "truncated response should produce a failed website record instead of failing the command: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.output["direct_fetch"], true);
    assert_eq!(response.output["results"][0]["fetch_mode"], "failed");
    assert_eq!(response.output["results"][0]["content_type"], "");
    assert_eq!(
        response.output["downloaded_files"][0]["content_type"],
        "text/markdown"
    );

    let saved_path = response.output["downloaded_files"][0]["path"]
        .as_str()
        .expect("malformed response markdown path");
    let saved = std::fs::read_to_string(dir.path().join(saved_path)).expect("malformed saved");
    assert!(saved.contains("Source:"));
    assert!(!saved.contains("partial body that should not be trusted"));
    assert!(server.join().starts_with("GET /truncated "));
}

#[test]
fn web_discover_business_flow_retries_local_cf_challenge_without_public_reader() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let previous_disabled = std::env::var_os("TURA_WEB_READER_DISABLED");
    std::env::set_var("TURA_WEB_READER_DISABLED", "true");

    let dir = tempfile::tempdir().expect("tempdir");
    let server = spawn_cf_retry_page_server();
    let command_line = format!(
        "web_discover website {} --download-dir retry-pages --max-results 1",
        server.url
    );

    let response = execute(&command_line, dir.path(), 10);

    restore_env("TURA_WEB_READER_DISABLED", previous_disabled);

    assert!(
        response.success,
        "challenge retry should recover locally: {}",
        response.stderr
    );
    assert_eq!(response.output["direct_fetch"], true);
    assert_eq!(response.output["results"][0]["fetch_mode"], "primary");
    assert_eq!(response.output["results"][0]["title"], "Retry Local Page");
    let saved_path = response.output["downloaded_files"][0]["path"]
        .as_str()
        .expect("retry markdown path");
    let saved = std::fs::read_to_string(dir.path().join(saved_path)).expect("retry saved");
    assert!(saved.contains("# Retry Local Page"));
    assert!(saved.contains("local retry success body"));

    let requests = server.join();
    assert_eq!(
        requests.len(),
        2,
        "challenge response should be retried exactly once by the primary fetcher"
    );
    assert!(requests[0].starts_with("GET /challenge "));
    assert!(requests[1].starts_with("GET /challenge "));
}

#[test]
fn web_discover_business_flow_falls_back_from_failed_brave_route_to_local_duckduckgo_results() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let result_page = spawn_page_server("Search Fallback Local Page");
    let search = spawn_search_route_fallback_server(&result_page.url);
    let previous_brave_key = std::env::var_os("TURA_BRAVE_SEARCH_API_KEY");
    let previous_brave_endpoint = std::env::var_os("TURA_BRAVE_WEB_SEARCH_ENDPOINT");
    let previous_exa_disabled = std::env::var_os("TURA_EXA_SEARCH_DISABLED");
    let previous_duck_endpoint = std::env::var_os("TURA_DUCKDUCKGO_SEARCH_ENDPOINT");
    let previous_reader_disabled = std::env::var_os("TURA_WEB_READER_DISABLED");
    std::env::set_var("TURA_BRAVE_SEARCH_API_KEY", "local-business-key");
    std::env::set_var("TURA_BRAVE_WEB_SEARCH_ENDPOINT", &search.brave_url);
    std::env::set_var("TURA_EXA_SEARCH_DISABLED", "true");
    std::env::set_var("TURA_DUCKDUCKGO_SEARCH_ENDPOINT", &search.duckduckgo_url);
    std::env::set_var("TURA_WEB_READER_DISABLED", "true");

    let command_line =
        "web_discover website tura local search fallback --download-dir search-pages --max-results 1";
    let response = execute(command_line, dir.path(), 10);

    restore_env("TURA_BRAVE_SEARCH_API_KEY", previous_brave_key);
    restore_env("TURA_BRAVE_WEB_SEARCH_ENDPOINT", previous_brave_endpoint);
    restore_env("TURA_EXA_SEARCH_DISABLED", previous_exa_disabled);
    restore_env("TURA_DUCKDUCKGO_SEARCH_ENDPOINT", previous_duck_endpoint);
    restore_env("TURA_WEB_READER_DISABLED", previous_reader_disabled);

    assert!(
        response.success,
        "search route fallback should succeed through local DuckDuckGo endpoint: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.output["direct_fetch"], Value::Null);
    assert_eq!(response.output["results"][0]["source"], "duckduckgo_html");
    assert_eq!(
        response.output["results"][0]["title"],
        "Search Fallback Local Page"
    );
    assert_eq!(response.output["results"][0]["fetch_mode"], "primary");
    assert_eq!(response.output["result_count"], 1);
    let saved_path = response.output["downloaded_files"][0]["path"]
        .as_str()
        .expect("fallback search saved path");
    let saved = std::fs::read_to_string(dir.path().join(saved_path)).expect("fallback saved");
    assert!(saved.contains("# Search Fallback Local Page"));
    assert!(saved.contains("business sentinel paragraph"));

    let search_requests = search.join();
    assert_eq!(search_requests.len(), 2);
    assert!(
        search_requests[0].starts_with("get /brave?"),
        "first route should try local Brave endpoint: {:?}",
        search_requests
    );
    assert!(
        search_requests[0].contains("x-subscription-token: local-business-key"),
        "Brave request should include configured local key: {}",
        search_requests[0]
    );
    assert!(
        search_requests[1].starts_with("get /duck?"),
        "fallback route should query local DuckDuckGo endpoint: {:?}",
        search_requests
    );
    assert!(result_page.join().starts_with("GET /article "));
}

#[test]
fn web_discover_business_flow_uses_custom_search_endpoint_and_fetches_only_usable_results() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let server = spawn_custom_search_with_pages_server();
    let previous_custom = std::env::var_os("TURA_WEB_DISCOVER_ENDPOINT");
    let previous_reader_disabled = std::env::var_os("TURA_WEB_READER_DISABLED");
    std::env::set_var("TURA_WEB_DISCOVER_ENDPOINT", &server.search_url);
    std::env::set_var("TURA_WEB_READER_DISABLED", "true");

    let command_line =
        "web_discover website custom local endpoint --download-dir custom-pages --max-results 3";
    let response = execute(command_line, dir.path(), 10);

    restore_env("TURA_WEB_DISCOVER_ENDPOINT", previous_custom);
    restore_env("TURA_WEB_READER_DISABLED", previous_reader_disabled);

    assert!(
        response.success,
        "custom endpoint business flow should succeed locally: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.output["type"], "website");
    assert_eq!(response.output["saved"], true);
    assert_eq!(
        response.output["result_count"], 2,
        "custom endpoint results without usable URLs must be skipped: {}",
        response.output
    );
    let records = response.output["results"]
        .as_array()
        .expect("custom endpoint records");
    assert_eq!(records[0]["source"], "custom_endpoint");
    assert_eq!(records[0]["title"], "Custom One");
    assert_eq!(records[0]["fetch_mode"], "primary");
    assert_eq!(records[1]["source"], "custom_endpoint");
    assert_eq!(records[1]["title"], "Custom Two");
    assert_eq!(records[1]["fetch_mode"], "primary");

    let downloaded = response.output["downloaded_files"]
        .as_array()
        .expect("downloaded custom pages");
    assert_eq!(downloaded.len(), 2);
    let first_path = downloaded[0]["path"].as_str().expect("first custom path");
    let second_path = downloaded[1]["path"].as_str().expect("second custom path");
    let first_saved = std::fs::read_to_string(dir.path().join(first_path)).expect("first saved");
    let second_saved = std::fs::read_to_string(dir.path().join(second_path)).expect("second saved");
    assert!(first_saved.contains("# Custom One"));
    assert!(first_saved.contains("custom one business body"));
    assert!(second_saved.contains("# Custom Two"));
    assert!(second_saved.contains("custom two business body"));
    assert!(response.stdout.contains("Custom One"));
    assert!(response.stdout.contains("Custom Two"));

    let requests = server.join();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].starts_with("POST /search "));
    assert!(
        requests[0].contains("\"query\":\"custom local endpoint\""),
        "custom search request should post normalized query body: {}",
        requests[0]
    );
    assert!(
        requests[0].contains("\"max_results\":3"),
        "custom search request should post configured result cap: {}",
        requests[0]
    );
    assert!(requests[1].starts_with("GET /custom-one "));
    assert!(requests[2].starts_with("GET /custom-two "));
}

struct PageServer {
    url: String,
    join: thread::JoinHandle<String>,
}

impl PageServer {
    fn join(self) -> String {
        self.join.join().expect("server joins")
    }
}

struct MultiAssetServer {
    addr: std::net::SocketAddr,
    join: thread::JoinHandle<Vec<String>>,
}

impl MultiAssetServer {
    fn url_for(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    fn join(self) -> Vec<String> {
        self.join.join().expect("multi asset server joins")
    }
}

struct AssetResponse {
    path: String,
    status: u16,
    content_type: String,
    body: Vec<u8>,
}

impl AssetResponse {
    fn ok(path: &str, content_type: &str, body: Vec<u8>) -> Self {
        Self::status(path, 200, content_type, body)
    }

    fn status(path: &str, status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            path: path.to_string(),
            status,
            content_type: content_type.to_string(),
            body,
        }
    }
}

fn spawn_multi_asset_server(mut assets: Vec<AssetResponse>) -> MultiAssetServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind multi asset server");
    let addr = listener.local_addr().expect("multi asset server addr");
    let join = thread::spawn(move || {
        let mut requests = Vec::new();
        let expected = assets.len();
        for _ in 0..expected {
            let (mut stream, _) = listener.accept().expect("accept multi asset request");
            let request = read_request_head(&mut stream);
            let request_path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
            let index = assets
                .iter()
                .position(|asset| asset.path == request_path)
                .unwrap_or_else(|| {
                    panic!("unexpected asset request path {request_path}; request was {request}")
                });
            let asset = assets.remove(index);
            let reason = if asset.status == 200 { "OK" } else { "ERROR" };
            let headers = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                asset.status,
                reason,
                asset.content_type,
                asset.body.len()
            );
            stream
                .write_all(headers.as_bytes())
                .expect("write multi asset headers");
            stream
                .write_all(&asset.body)
                .expect("write multi asset body");
            requests.push(request);
        }
        requests
    });
    MultiAssetServer { addr, join }
}

fn spawn_page_server(title: &str) -> PageServer {
    spawn_response_server(
        200,
        "text/html; charset=utf-8",
        html_page(title),
        "/article",
    )
}

fn spawn_cf_retry_page_server() -> RetryPageServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local retry server");
    let addr = listener.local_addr().expect("retry server addr");
    let join = thread::spawn(move || {
        let mut requests = Vec::new();
        let (mut first, _) = listener.accept().expect("accept challenge request");
        requests.push(read_request_head(&mut first));
        first
            .write_all(
                concat!(
                    "HTTP/1.1 403 Forbidden\r\n",
                    "Content-Type: text/html; charset=utf-8\r\n",
                    "cf-mitigated: challenge\r\n",
                    "Connection: close\r\n",
                    "Content-Length: 22\r\n",
                    "\r\n",
                    "<html>challenge</html>"
                )
                .as_bytes(),
            )
            .expect("write challenge response");
        drop(first);

        let (mut second, _) = listener.accept().expect("accept retry request");
        requests.push(read_request_head(&mut second));
        let body = html_page("Retry Local Page").replace(
            "business sentinel paragraph",
            "local retry success body paragraph",
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        second
            .write_all(response.as_bytes())
            .expect("write retry response");
        requests
    });
    RetryPageServer {
        url: format!("http://{addr}/challenge"),
        join,
    }
}

struct RetryPageServer {
    url: String,
    join: thread::JoinHandle<Vec<String>>,
}

impl RetryPageServer {
    fn join(self) -> Vec<String> {
        self.join.join().expect("retry server joins")
    }
}

struct ReaderFallbackServer {
    addr: std::net::SocketAddr,
    url: String,
    join: thread::JoinHandle<Vec<String>>,
}

impl ReaderFallbackServer {
    fn join(self) -> Vec<String> {
        self.join.join().expect("reader fallback server joins")
    }
}

struct SearchRouteFallbackServer {
    brave_url: String,
    duckduckgo_url: String,
    join: thread::JoinHandle<Vec<String>>,
}

impl SearchRouteFallbackServer {
    fn join(self) -> Vec<String> {
        self.join.join().expect("search fallback server joins")
    }
}

struct CustomSearchWithPagesServer {
    search_url: String,
    join: thread::JoinHandle<Vec<String>>,
}

impl CustomSearchWithPagesServer {
    fn join(self) -> Vec<String> {
        self.join.join().expect("custom search server joins")
    }
}

fn spawn_custom_search_with_pages_server() -> CustomSearchWithPagesServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind custom search server");
    let addr = listener.local_addr().expect("custom search addr");
    let join = thread::spawn(move || {
        let mut requests = Vec::new();

        let (mut search, _) = listener.accept().expect("accept custom search request");
        let search_request = read_http_request(&mut search);
        let search_body = json!({
            "results": [
                {
                    "title": "Custom One",
                    "url": format!("http://{addr}/custom-one"),
                    "snippet": "first custom endpoint result"
                },
                {
                    "name": "Custom Two",
                    "link": format!("http://{addr}/custom-two"),
                    "description": "second custom endpoint result",
                    "sourceUrl": format!("http://{addr}/source-two")
                },
                {
                    "title": "Missing URL from custom endpoint"
                }
            ]
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            search_body.len(),
            search_body
        );
        search
            .write_all(response.as_bytes())
            .expect("write custom search response");
        requests.push(search_request);
        drop(search);

        for (path, title, marker) in [
            (
                "/custom-one",
                "Custom One",
                "custom one business body paragraph",
            ),
            (
                "/custom-two",
                "Custom Two",
                "custom two business body paragraph",
            ),
        ] {
            let (mut page, _) = listener.accept().expect("accept custom result page");
            let page_request = read_request_head(&mut page);
            assert!(
                page_request.starts_with(&format!("GET {path} ")),
                "unexpected custom result page request: {page_request}"
            );
            let body = html_page(title).replace("business sentinel paragraph", marker);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            page.write_all(response.as_bytes())
                .expect("write custom result page");
            requests.push(page_request);
        }
        requests
    });
    CustomSearchWithPagesServer {
        search_url: format!("http://{addr}/search"),
        join,
    }
}

fn spawn_search_route_fallback_server(result_url: &str) -> SearchRouteFallbackServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind search fallback server");
    let addr = listener.local_addr().expect("search fallback addr");
    let result_url = result_url.to_string();
    let join = thread::spawn(move || {
        let mut requests = Vec::new();

        let (mut brave, _) = listener.accept().expect("accept brave search request");
        requests.push(read_request_head(&mut brave).to_ascii_lowercase());
        brave
            .write_all(
                concat!(
                    "HTTP/1.1 500 Internal Server Error\r\n",
                    "Content-Type: application/json\r\n",
                    "Connection: close\r\n",
                    "Content-Length: 26\r\n",
                    "\r\n",
                    "{\"error\":\"local failure\"}"
                )
                .as_bytes(),
            )
            .expect("write brave failure");
        drop(brave);

        let (mut duck, _) = listener.accept().expect("accept duckduckgo search request");
        requests.push(read_request_head(&mut duck).to_ascii_lowercase());
        let body = format!(
            r#"<!doctype html>
<html>
  <body>
    <a class="result__a" href="{result_url}">Search Fallback Local Page</a>
    <a class="result__snippet" href="{result_url}">local search fallback snippet</a>
  </body>
</html>"#
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        duck.write_all(response.as_bytes())
            .expect("write duckduckgo results");
        requests
    });
    SearchRouteFallbackServer {
        brave_url: format!("http://{addr}/brave"),
        duckduckgo_url: format!("http://{addr}/duck"),
        join,
    }
}

fn spawn_reader_fallback_server() -> ReaderFallbackServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local reader fallback server");
    let addr = listener.local_addr().expect("reader fallback server addr");
    let join = thread::spawn(move || {
        let mut requests = Vec::new();

        let (mut primary, _) = listener.accept().expect("accept primary request");
        requests.push(read_request_head(&mut primary));
        let primary_body =
            "<html><head><title>Tiny Primary</title></head><body>tiny primary</body></html>";
        let primary_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            primary_body.len(),
            primary_body
        );
        primary
            .write_all(primary_response.as_bytes())
            .expect("write primary response");
        drop(primary);

        let (mut reader, _) = listener.accept().expect("accept reader request");
        requests.push(read_request_head(&mut reader));
        let reader_body = format!(
            "Title: Reader Fallback Business Page\n\n{}",
            "reader fallback business body ".repeat(40)
        );
        let reader_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/markdown; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            reader_body.len(),
            reader_body
        );
        reader
            .write_all(reader_response.as_bytes())
            .expect("write reader response");
        requests
    });
    ReaderFallbackServer {
        addr,
        url: format!("http://{addr}/short"),
        join,
    }
}

fn spawn_truncated_response_server() -> PageServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind truncated response server");
    let addr = listener
        .local_addr()
        .expect("truncated response server addr");
    let join = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("accept truncated response request");
        let request = read_request_head(&mut stream);
        stream
            .write_all(
                concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Type: text/html; charset=utf-8\r\n",
                    "Connection: close\r\n",
                    "Content-Length: 8192\r\n",
                    "\r\n",
                    "<html><head><title>Truncated</title></head><body>partial body that should not be trusted"
                )
                .as_bytes(),
            )
            .expect("write truncated response");
        request
    });
    PageServer {
        url: format!("http://{addr}/truncated"),
        join,
    }
}

fn spawn_response_server(status: u16, content_type: &str, body: String, path: &str) -> PageServer {
    spawn_binary_response_server(status, content_type, body.into_bytes(), path)
}

fn spawn_binary_response_server(
    status: u16,
    content_type: &str,
    body: Vec<u8>,
    path: &str,
) -> PageServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local server");
    let addr = listener.local_addr().expect("server addr");
    let content_type = content_type.to_string();
    let reason = if status == 200 { "OK" } else { "ERROR" }.to_string();
    let path = path.to_string();
    let join = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let request = read_request_head(&mut stream);
        let headers = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream
            .write_all(headers.as_bytes())
            .expect("write response headers");
        stream.write_all(&body).expect("write response body");
        request
    });
    PageServer {
        url: format!("http://{addr}{path}"),
        join,
    }
}

fn read_request_head(stream: &mut std::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 512];
    loop {
        let read = stream.read(&mut chunk).expect("read request");
        assert!(read > 0, "client closed before request headers");
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&buffer).to_string()
}

fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 512];
    let mut header_end = None;
    loop {
        let read = stream.read(&mut chunk).expect("read http request");
        assert!(read > 0, "client closed before request");
        buffer.extend_from_slice(&chunk[..read]);
        if header_end.is_none() {
            header_end = buffer
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|index| index + 4);
        }
        if let Some(end) = header_end {
            let header = String::from_utf8_lossy(&buffer[..end]).to_string();
            let content_length = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if name.eq_ignore_ascii_case("content-length") {
                        value.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if buffer.len() >= end + content_length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&buffer).to_string()
}

fn html_page(title: &str) -> String {
    let long_body = (0..80)
        .map(|index| {
            format!(
                "business sentinel paragraph {index}: local loopback content proves the command can fetch and save a webpage without public internet."
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        r#"<!doctype html>
<html>
  <head><title>{title}</title></head>
  <body>
    <main>
      <h1>{title}</h1>
      <p>{long_body}</p>
    </main>
  </body>
</html>"#
    )
}

fn tiny_png_bytes() -> &'static [u8] {
    &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60,
        0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xe2, 0x21, 0xbc, 0x33, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ]
}

fn write_fake_ytdlp(dir: &Path) -> std::path::PathBuf {
    #[cfg(windows)]
    {
        let script = dir.join("fake-ytdlp.cmd");
        let ps1 = dir.join("fake-ytdlp.ps1");
        std::fs::write(
            &ps1,
            r#"$template = $null
for ($index = 0; $index -lt $args.Count; $index++) {
  if ($args[$index] -eq '-o' -and ($index + 1) -lt $args.Count) {
    $template = $args[$index + 1]
    $index++
  }
}
if ([string]::IsNullOrWhiteSpace($template)) {
  exit 2
}
$path = $template.Replace('%(title).80s', 'business-audio').Replace('%(id)s', 'local').Replace('%(ext)s', 'mp3')
$isVideo = $args -contains 'best[height<=540][ext=mp4]/best[height<=540]/best'
if ($isVideo) {
  $path = $template.Replace('%(title).80s', 'business-video').Replace('%(id)s', 'local').Replace('%(ext)s', 'mp4')
}
New-Item -ItemType Directory -Force -Path (Split-Path -LiteralPath $path) | Out-Null
if ($isVideo) {
  [System.IO.File]::WriteAllText($path, 'fake local video bytes', [System.Text.Encoding]::ASCII)
} else {
  [System.IO.File]::WriteAllText($path, 'fake local audio bytes', [System.Text.Encoding]::ASCII)
}
"#,
        )
        .expect("write fake yt-dlp ps1");
        std::fs::write(
            &script,
            r#"@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0fake-ytdlp.ps1" %*
exit /b %ERRORLEVEL%
"#,
        )
        .expect("write fake yt-dlp cmd");
        script
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("fake-ytdlp.sh");
        std::fs::write(
            &script,
            r#"#!/usr/bin/env sh
set -eu
template=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then
    shift
    template="$1"
  fi
  shift || true
done
if [ -z "$template" ]; then
  exit 2
fi
is_video=0
for arg in "$@"; do
  if [ "$arg" = "best[height<=540][ext=mp4]/best[height<=540]/best" ]; then
    is_video=1
  fi
done
if [ "$is_video" -eq 1 ]; then
  path=$(printf '%s' "$template" | sed 's/%(title).80s/business-video/g; s/%(id)s/local/g; s/%(ext)s/mp4/g')
else
  path=$(printf '%s' "$template" | sed 's/%(title).80s/business-audio/g; s/%(id)s/local/g; s/%(ext)s/mp3/g')
fi
mkdir -p "$(dirname "$path")"
if [ "$is_video" -eq 1 ]; then
  printf 'fake local video bytes' > "$path"
else
  printf 'fake local audio bytes' > "$path"
fi
"#,
        )
        .expect("write fake yt-dlp sh");
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script)
            .expect("fake yt-dlp metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).expect("chmod fake yt-dlp");
        script
    }
}

fn run_protocol(request: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tura-command-web-discover"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn web_discover binary");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(request.to_string().as_bytes())
        .expect("write request");
    let output = child.wait_with_output().expect("protocol output");
    assert!(
        output.status.success(),
        "web_discover protocol process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("protocol json response")
}

fn normalize_path(path: &str) -> String {
    Path::new(path).to_string_lossy().replace('\\', "/")
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn restore_env(name: &str, value: Option<std::ffi::OsString>) {
    if let Some(value) = value {
        std::env::set_var(name, value);
    } else {
        std::env::remove_var(name);
    }
}
