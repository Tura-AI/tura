use serde_json::json;

#[allow(dead_code, unused_imports)]
#[path = "business/helpers/web_discover_local.rs"]
mod helpers;
use helpers::*;

#[test]
fn web_discover_asset_cli_e2e_downloads_five_asset_styles() {
    let dir = tempfile::tempdir().expect("tempdir");
    let server = spawn_multi_asset_server(vec![
        AssetResponse::ok(
            "/noir-hud.wgsl",
            "text/plain",
            b"@fragment fn fs_main() -> @location(0) vec4f { return vec4f(0.1, 0.9, 0.6, 1.0); }"
                .to_vec(),
        ),
        AssetResponse::ok(
            "/desert-rock.jpg",
            "image/jpeg",
            b"desert rock texture bytes".to_vec(),
        ),
        AssetResponse::ok("/pixel-panel.png", "image/png", tiny_png_bytes().to_vec()),
        AssetResponse::ok(
            "/compact-ship.glb",
            "model/gltf-binary",
            b"glTF compact ship bytes".to_vec(),
        ),
        AssetResponse::ok(
            "/retro-click.ogg",
            "audio/ogg",
            b"OggS retro click bytes".to_vec(),
        ),
    ]);

    for (asset_type, path, expected_content_type, style_label) in [
        ("shader", "/noir-hud.wgsl", "text/plain", "noir HUD shader"),
        (
            "texture",
            "/desert-rock.jpg",
            "image/jpeg",
            "desert rock texture",
        ),
        ("2d", "/pixel-panel.png", "image/png", "pixel UI sprite"),
        (
            "3d",
            "/compact-ship.glb",
            "model/gltf-binary",
            "compact ship model",
        ),
        (
            "audio",
            "/retro-click.ogg",
            "audio/ogg",
            "retro click sound",
        ),
    ] {
        let response = run_protocol(json!({
            "kind": "execute",
            "payload": {
                "session_dir": dir.path().display().to_string(),
                "arguments": format!(
                    "web_discover asset {asset_type} {} --download-dir cli-assets --min-size 1 --max-size 1000000",
                    server.url_for(path)
                )
            }
        }));

        assert_eq!(response["ok"], true, "{style_label}");
        assert_eq!(response["success"], true, "{style_label}: {response}");
        assert_eq!(response["exit_code"], 0, "{style_label}");
        assert_eq!(response["output"]["type"], "asset", "{style_label}");
        assert_eq!(
            response["output"]["asset_type"], asset_type,
            "{style_label}"
        );
        assert_eq!(response["output"]["result_count"], 1, "{style_label}");
        assert_eq!(
            response["output"]["searched_sources"][0], "direct_asset_url",
            "{style_label}"
        );
        let downloaded = response["output"]["downloaded_files"]
            .as_array()
            .expect("downloaded files");
        assert_eq!(downloaded.len(), 1, "{style_label}");
        assert_eq!(
            downloaded[0]["content_type"], expected_content_type,
            "{style_label}"
        );
        let local_path = downloaded[0]["path"].as_str().expect("local path");
        assert!(
            normalize_path(local_path).starts_with(&format!("cli-assets/{asset_type}/")),
            "{style_label} should be saved below its typed asset dir: {local_path}"
        );
        assert!(
            dir.path().join(local_path).exists(),
            "{style_label} should exist on disk"
        );
    }

    let requests = server.join();
    assert_eq!(requests.len(), 5);
    for path in [
        "/noir-hud.wgsl",
        "/desert-rock.jpg",
        "/pixel-panel.png",
        "/compact-ship.glb",
        "/retro-click.ogg",
    ] {
        assert!(
            requests
                .iter()
                .any(|request| request.starts_with(&format!("GET {path} "))),
            "missing e2e request for {path}: {requests:?}"
        );
    }
}
