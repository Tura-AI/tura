use serde_json::{json, Value};
use tura_command_web_discover::{access, execute};

#[path = "helpers/web_discover_local.rs"]
mod helpers;
use helpers::*;
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
fn web_discover_business_flow_downloads_assets_into_typed_dirs_and_extracts_zip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let shader_bytes =
        b"void mainImage(out vec4 color, in vec2 uv) { color = vec4(uv, 0.0, 1.0); }";
    let texture_bytes = b"business texture jpg bytes";
    let sprite_bytes = tiny_png_bytes();
    let model_bytes = b"glTF business binary model";
    let audio_bytes = b"ID3 business audio bytes";
    let bundle_bytes = test_zip_bytes(&[
        ("models/fighter.glb", b"glTF extracted fighter"),
        ("textures/fighter-hull.png", tiny_png_bytes()),
    ]);
    let server = spawn_multi_asset_server(vec![
        AssetResponse::ok("/hud.glsl", "text/plain", shader_bytes.to_vec()),
        AssetResponse::ok("/brushed-metal.jpg", "image/jpeg", texture_bytes.to_vec()),
        AssetResponse::ok("/particle.png", "image/png", sprite_bytes.to_vec()),
        AssetResponse::ok("/patrol.glb", "model/gltf-binary", model_bytes.to_vec()),
        AssetResponse::ok("/impact.mp3", "audio/mpeg", audio_bytes.to_vec()),
        AssetResponse::ok("/fighter-pack.zip", "application/zip", bundle_bytes.clone()),
    ]);

    for (asset_type, path, expected_content_type, expected_size) in [
        (
            "shader",
            "/hud.glsl",
            "text/plain",
            shader_bytes.len() as u64,
        ),
        (
            "texture",
            "/brushed-metal.jpg",
            "image/jpeg",
            texture_bytes.len() as u64,
        ),
        (
            "2d",
            "/particle.png",
            "image/png",
            sprite_bytes.len() as u64,
        ),
        (
            "3d",
            "/patrol.glb",
            "model/gltf-binary",
            model_bytes.len() as u64,
        ),
        (
            "audio",
            "/impact.mp3",
            "audio/mpeg",
            audio_bytes.len() as u64,
        ),
    ] {
        let command_line = format!(
            "web_discover asset {asset_type} {} --download-dir assets --min-size 1 --max-size 1000000",
            server.url_for(path)
        );
        let response = execute(&command_line, dir.path(), 10);

        assert!(
            response.success,
            "asset {asset_type} download should succeed: {}",
            response.stderr
        );
        assert_eq!(response.output["type"], "asset");
        assert_eq!(response.output["asset_type"], asset_type);
        assert_eq!(response.output["searched_sources"][0], "direct_asset_url");
        assert_eq!(response.output["result_count"], 1);
        let downloaded = response.output["downloaded_files"]
            .as_array()
            .expect("downloaded asset files");
        assert_eq!(downloaded.len(), 1);
        assert_eq!(downloaded[0]["content_type"], expected_content_type);
        assert_eq!(downloaded[0]["size"], expected_size);
        let local_path = downloaded[0]["path"].as_str().expect("asset path");
        assert!(
            normalize_path(local_path).starts_with(&format!("assets/{asset_type}/")),
            "asset should be placed under typed dir: {local_path}"
        );
        assert!(dir.path().join(local_path).exists());
    }

    let zip_command = format!(
        "web_discover asset 3d {} --download-dir assets --min-size 1 --max-size 1000000",
        server.url_for("/fighter-pack.zip")
    );
    let zip_response = execute(&zip_command, dir.path(), 10);
    assert!(
        zip_response.success,
        "zip asset download should succeed: {}",
        zip_response.stderr
    );
    assert_eq!(zip_response.output["type"], "asset");
    assert_eq!(zip_response.output["asset_type"], "3d");
    let zip_downloaded = zip_response.output["downloaded_files"]
        .as_array()
        .expect("zip downloaded files");
    assert_eq!(zip_downloaded.len(), 3);
    assert!(zip_downloaded.iter().any(|item| {
        item["path"]
            .as_str()
            .is_some_and(|path| normalize_path(path).contains("assets/3d/archives/"))
            && item["content_type"] == "application/zip"
            && item["size"] == bundle_bytes.len() as u64
    }));
    assert!(zip_downloaded.iter().any(|item| {
        item["path"]
            .as_str()
            .is_some_and(|path| normalize_path(path).ends_with("models/fighter.glb"))
            && item["content_type"] == "model/gltf-binary"
    }));
    assert!(zip_downloaded.iter().any(|item| {
        item["path"]
            .as_str()
            .is_some_and(|path| normalize_path(path).ends_with("textures/fighter-hull.png"))
            && item["content_type"] == "image/png"
    }));

    let requests = server.join();
    assert_eq!(requests.len(), 6);
    for path in [
        "/hud.glsl",
        "/brushed-metal.jpg",
        "/particle.png",
        "/patrol.glb",
        "/impact.mp3",
        "/fighter-pack.zip",
    ] {
        assert!(
            requests
                .iter()
                .any(|request| request.starts_with(&format!("GET {path} "))),
            "missing request for {path}: {requests:?}"
        );
    }
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
