use code_tools::command_run;
use code_tools::commands;
use code_tools::runtime::file_locks::{self, Access};
use code_tools::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolPayload, ToolRouter, ToolRuntimeEvent,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

async fn env_lock() -> tokio::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().await
}

fn env_lock_blocking() -> tokio::sync::MutexGuard<'static, ()> {
    ENV_LOCK.blocking_lock()
}

fn temp_workspace(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "tura-command-run-current-flow-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp workspace");
    path
}

fn find_ffmpeg() -> Option<String> {
    if let Ok(path) = std::env::var("FFMPEG_PATH") {
        if !path.trim().is_empty() && PathBuf::from(&path).exists() {
            return Some(path);
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(if cfg!(windows) {
                "ffmpeg.exe"
            } else {
                "ffmpeg"
            });
            if candidate.exists() {
                return Some(candidate.display().to_string());
            }
        }
    }
    let output = std::process::Command::new("python")
        .arg("-c")
        .arg("import imageio_ffmpeg; print(imageio_ffmpeg.get_ffmpeg_exe())")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !path.is_empty() && PathBuf::from(&path).exists() {
        Some(path)
    } else {
        None
    }
}

#[test]
fn pass_current_style_command_run_output_shape() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("shape");

    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "shell_command", "command_line": "{\"command\":\"Write-Output ok\",\"timeout_ms\":5000}" }
            ]
        }),
        &root,
    );

    assert!(output.get("results").is_some());
    assert!(
        output.get("ok").is_none(),
        "current command_run does not expose top-level ok"
    );
    assert!(output.get("output_policy").is_none());
    assert!(output["results"][0].get("command").is_none());
    assert_eq!(output["results"][0]["command_type"], "shell_command");
    assert_eq!(output["results"][0]["success"], true);
}

#[tokio::test]
async fn pass_internal_command_rebuilds_tool_call_and_dispatches_router_handler() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("router");
    let router = ToolRouter::new();
    let call = ToolCall {
        tool_name: "shell_command".to_string(),
        call_id: "call_test".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "command": "Write-Output router-ok", "timeout_ms": 5000 }),
        },
    };

    let result = router
        .dispatch(call, ToolContext::new(root), false)
        .await
        .expect("router dispatch should succeed");

    assert_eq!(result.call_id, "call_test");
    assert_eq!(result.result.success, Some(true));
    assert!(result.result.code_mode_result()["stdout"]
        .as_str()
        .unwrap_or_default()
        .contains("router-ok"));
}

#[test]
fn fail_empty_command_run_returns_current_style_failure_result() {
    let root = temp_workspace("empty");
    let output = command_run::execute(&json!({ "commands": [] }), &root);

    assert!(output["results"][0].get("command").is_none());
    assert_eq!(output["results"][0]["command_type"], "command_run");
    assert_eq!(output["results"][0]["success"], false);
    assert_eq!(
        output["results"][0]["error"],
        "command_run commands must not be empty"
    );
}

#[test]
fn fail_unsupported_internal_command_returns_model_visible_result() {
    let root = temp_workspace("unsupported");
    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "read_file", "command_line": "{}" }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false);
    assert!(output["results"][0]["error"]
        .as_str()
        .expect("error should be a string")
        .contains("unsupported command_run command"));
}

#[test]
fn pass_read_media_image_returns_compact_visual_preview() {
    let root = temp_workspace("read-media-image");
    let image_path = root.join("sample.png");
    let mut image = image::RgbImage::new(64, 64);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = if x > y {
            image::Rgb([220, 20, 20])
        } else {
            image::Rgb([20, 60, 220])
        };
    }
    image.save(&image_path).expect("save image fixture");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "{\"paths\":[\"sample.png\"],\"max_visuals\":1,\"max_side\":256}",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "read_media");
    assert_eq!(output["results"][0]["success"], true);
    let media = &output["results"][0]["output"]["media_results"][0];
    assert_eq!(media["media_type"], "image");
    assert_eq!(media["visual_preview_count"], 1);
    let url = media["visual_previews"][0]["image_url"]["url"]
        .as_str()
        .expect("image data url");
    assert!(url.starts_with("data:image/jpeg;base64,"));
    assert!(
        url.len() < 80_000,
        "preview should be compact, got {}",
        url.len()
    );
}

#[test]
fn pass_read_media_pdf_text_fallback_is_context_sized() {
    let root = temp_workspace("read-media-pdf");
    fs::write(
        root.join("brief.pdf"),
        b"%PDF-1.4\n1 0 obj <<>> stream\nBT /F1 12 Tf 72 720 Td (Quarterly media brief: blue chart and revenue table.) Tj ET\nendstream endobj\n%%EOF",
    )
    .expect("write simple pdf fixture");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "{\"paths\":[\"brief.pdf\"],\"include_text\":true,\"max_text_chars\":2000,\"max_visuals\":1}",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "read_media");
    assert_eq!(output["results"][0]["success"], true);
    let media = &output["results"][0]["output"]["media_results"][0];
    assert_eq!(media["media_type"], "pdf");
    assert!(media["extracted_text"]
        .as_str()
        .unwrap_or_default()
        .contains("Quarterly media brief"));
    assert!(
        serde_json::to_string(&output).expect("serialize").len() < 120_000,
        "read_media output should stay reasonably small"
    );
}

#[test]
fn pass_read_media_video_uses_available_frame_decoder() {
    let root = temp_workspace("read-media-video");
    let video_path = root.join("clip.mp4");
    let status = std::process::Command::new("python")
        .arg("-c")
        .arg(
            r#"
import cv2, sys
import numpy as np
out = cv2.VideoWriter(sys.argv[1], cv2.VideoWriter_fourcc(*"mp4v"), 2.0, (64, 64))
for i in range(6):
    frame = np.zeros((64,64,3), dtype=np.uint8)
    frame[:,:] = (i*30, 40, 220-i*20)
    cv2.putText(frame, str(i), (20,40), cv2.FONT_HERSHEY_SIMPLEX, 1, (255,255,255), 2)
    out.write(frame)
out.release()
"#,
        )
        .arg(&video_path)
        .status()
        .expect("run python");
    if !status.success() {
        eprintln!("python cv2 unavailable; skipping video decode fixture");
        return;
    }

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "{\"paths\":[\"clip.mp4\"],\"max_visuals\":3,\"max_side\":128}",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "read_media");
    assert_eq!(output["results"][0]["success"], true);
    let media = &output["results"][0]["output"]["media_results"][0];
    assert_eq!(media["media_type"], "video");
    assert!(media["visual_preview_count"].as_u64().unwrap_or(0) >= 1);
}

#[test]
fn pass_read_media_audio_returns_compact_audio_preview() {
    let Some(ffmpeg) = find_ffmpeg() else {
        eprintln!("ffmpeg unavailable; skipping audio preview fixture");
        return;
    };
    let root = temp_workspace("read-media-audio");
    let audio_path = root.join("tone.wav");
    let status = std::process::Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=440:duration=1")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg("-y")
        .arg(&audio_path)
        .status()
        .expect("run ffmpeg");
    if !status.success() {
        eprintln!("ffmpeg sine fixture failed; skipping audio preview fixture");
        return;
    }

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "tone.wav",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "read_media");
    assert_eq!(output["results"][0]["success"], true);
    let media = &output["results"][0]["output"]["media_results"][0];
    assert_eq!(media["media_type"], "audio");
    assert_eq!(media["audio_preview_count"], 1);
    let url = media["audio_previews"][0]["audio_url"]["url"]
        .as_str()
        .expect("audio data url");
    assert!(url.starts_with("data:audio/mpeg;base64,"));
    assert!(
        url.len() < 80_000,
        "audio preview should be compact, got {}",
        url.len()
    );
}

#[test]
fn pass_read_media_directory_reads_newest_limited_files() {
    let root = temp_workspace("read-media-directory");
    let dir = root.join("media");
    fs::create_dir_all(&dir).expect("create media dir");
    fs::write(dir.join("old.txt"), "old file").expect("write old");
    std::thread::sleep(Duration::from_millis(20));
    fs::write(dir.join("newer.txt"), "newer file").expect("write newer");
    std::thread::sleep(Duration::from_millis(20));
    fs::write(dir.join("newest.txt"), "newest file").expect("write newest");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "read_media media --max-files=2",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    let tool_output = &output["results"][0]["output"];
    assert_eq!(tool_output["visual_contact_sheet"], true);
    let media_results = tool_output["media_results"]
        .as_array()
        .expect("media results");
    assert_eq!(media_results.len(), 2);
    let serialized = serde_json::to_string(media_results).expect("serialize");
    assert!(serialized.contains("newest.txt"));
    assert!(serialized.contains("newer.txt"));
    assert!(!serialized.contains("old file"));
    assert!(!serialized.contains("newest file"));
    assert!(!serialized.contains("newer file"));
}

#[test]
fn pass_read_media_small_unknown_binary_document_returns_file_attachment() {
    let root = temp_workspace("read-media-small-docx");
    fs::write(
        root.join("sample.docx"),
        [0x50, 0x4b, 0x03, 0x04, 0xff, 0x00],
    )
    .expect("write docx bytes");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "read_media sample.docx",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    let media = &output["results"][0]["output"]["media_results"][0];
    assert_eq!(media["media_type"], "document");
    assert_eq!(media["file_name"], "sample.docx");
    assert_eq!(media["file_attachment_count"], 1);
    assert_eq!(
        media["file_attachments"][0]["mime_type"],
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );
    assert!(media["file_attachments"][0]["data_base64"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
}

#[test]
fn pass_read_media_large_unknown_binary_document_returns_metadata_only() {
    let root = temp_workspace("read-media-large-doc");
    fs::write(root.join("large.doc"), vec![0xff; 1_000_001]).expect("write large doc");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "read_media large.doc",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    let media = &output["results"][0]["output"]["media_results"][0];
    assert_eq!(media["media_type"], "document");
    assert_eq!(media["file_name"], "large.doc");
    assert_eq!(media["size_bytes"], 1_000_001);
    assert!(media["modified_unix_ms"].is_number());
    assert_eq!(media["file_attachment_count"], 0);
    assert_eq!(media["extracted_text"], "");
}

#[test]
fn pass_read_media_directory_images_are_compacted_into_contact_sheet() {
    let root = temp_workspace("read-media-directory-sheet");
    let dir = root.join("media");
    fs::create_dir_all(&dir).expect("create media dir");
    for index in 0..3 {
        let image_path = dir.join(format!("sample-{index}.png"));
        let mut image = image::RgbImage::new(80, 60);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Rgb([(index * 70) as u8, (x % 255) as u8, (y * 3 % 255) as u8]);
        }
        image.save(&image_path).expect("save image fixture");
        std::thread::sleep(Duration::from_millis(10));
    }

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "media --max-files 3 --max-visuals 3",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    let tool_output = &output["results"][0]["output"];
    assert_eq!(tool_output["visual_contact_sheet"], true);
    assert_eq!(tool_output["visual_preview_count"], 1);
    let sheet_url = tool_output["visual_previews"][0]["image_url"]["url"]
        .as_str()
        .expect("sheet image data url");
    assert!(sheet_url.starts_with("data:image/jpeg;base64,"));
    for media in tool_output["media_results"]
        .as_array()
        .expect("media results")
    {
        assert_eq!(media["visual_preview_count"], 0);
        assert_eq!(
            media["visual_previews_compacted_into"],
            "top_level_contact_sheet"
        );
    }
}

#[test]
fn pass_read_media_inline_arguments_are_accepted() {
    let root = temp_workspace("read-media-inline");
    fs::write(root.join("note.txt"), "inline args worked").expect("write note");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "path": "note.txt",
                    "max_text_chars": "2000",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    let serialized = serde_json::to_string(&output).expect("serialize");
    assert!(serialized.contains("inline args worked"));
}

#[test]
fn pass_web_discover_image_download_writes_image() {
    let _guard = env_lock_blocking();
    let root = temp_workspace("web-discover-image");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let image_url = format!("http://{addr}/image.jpg");
    let endpoint = format!("http://{addr}/images");
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("GET /images") {
                let html = format!(
                    r#"<html><body><a href="/images/detail?mediaurl={}&purl={}"><img alt="Official fixture photo"></a></body></html>"#,
                    image_url.replace(":", "%3a").replace("/", "%2f"),
                    "https%3a%2f%2fofficial.example%2fsource"
                );
                write_http_response(&mut stream, "text/html", &html);
            } else {
                let mut image = image::RgbImage::new(48, 48);
                for (_, _, pixel) in image.enumerate_pixels_mut() {
                    *pixel = image::Rgb([20, 120, 220]);
                }
                let mut bytes = Vec::new();
                image::DynamicImage::ImageRgb8(image)
                    .write_to(
                        &mut std::io::Cursor::new(&mut bytes),
                        image::ImageFormat::Jpeg,
                    )
                    .expect("encode jpeg");
                write_http_response_bytes(&mut stream, "image/jpeg", &bytes);
            }
        }
    });

    std::env::set_var("TURA_IMAGE_SEARCH_ENDPOINT", endpoint);
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": "--type image --query fixture --max-results 1 --download-dir media/image --min-size 100 --max-size 1000000",
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_IMAGE_SEARCH_ENDPOINT");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["command_type"], "web_discover");
    assert_eq!(output["results"][0]["success"], true);
    let downloaded = output["results"][0]["output"]["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert_eq!(downloaded.len(), 1);
    let relative = downloaded[0]["path"].as_str().expect("relative path");
    assert!(root.join(relative).exists());
    assert_eq!(
        downloaded[0]["source_page_url"],
        "https://official.example/source"
    );
}

#[test]
fn pass_web_discover_image_uses_brave_endpoint_when_key_is_set() {
    let _guard = env_lock_blocking();
    let root = temp_workspace("web-discover-brave-image");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let image_url = format!("http://{addr}/brave-image.jpg");
    let endpoint = format!("http://{addr}/brave-images");
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("GET /brave-images") {
                assert!(request.contains("q=fixture"));
                let body = json!({
                    "type": "images",
                    "results": [
                        {
                            "type": "image_result",
                            "title": "Official fixture from Brave",
                            "source": "https://official.example/brave-source",
                            "properties": {
                                "url": image_url,
                                "width": 48,
                                "height": 48
                            },
                            "thumbnail": {
                                "src": "https://imgs.search.brave.com/thumb"
                            },
                            "meta_url": {
                                "hostname": "official.example"
                            }
                        }
                    ]
                })
                .to_string();
                write_http_response(&mut stream, "application/json", &body);
            } else {
                let mut image = image::RgbImage::new(48, 48);
                for (_, _, pixel) in image.enumerate_pixels_mut() {
                    *pixel = image::Rgb([80, 180, 40]);
                }
                let mut bytes = Vec::new();
                image::DynamicImage::ImageRgb8(image)
                    .write_to(
                        &mut std::io::Cursor::new(&mut bytes),
                        image::ImageFormat::Jpeg,
                    )
                    .expect("encode jpeg");
                write_http_response_bytes(&mut stream, "image/jpeg", &bytes);
            }
        }
    });

    std::env::set_var("TURA_BRAVE_SEARCH_API_KEY", "test-key");
    std::env::set_var("TURA_BRAVE_IMAGE_SEARCH_ENDPOINT", endpoint);
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": "--type image --query fixture --max-results 1 --download-dir media/brave --min-size 100 --max-size 1000000",
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_BRAVE_SEARCH_API_KEY");
    std::env::remove_var("TURA_BRAVE_IMAGE_SEARCH_ENDPOINT");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["command_type"], "web_discover");
    assert_eq!(output["results"][0]["success"], true);
    let result = &output["results"][0]["output"]["results"][0];
    assert_eq!(result["source"], "brave_images");
    assert_eq!(result["page_url"], "https://official.example/brave-source");
    let downloaded = output["results"][0]["output"]["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert_eq!(downloaded.len(), 1);
    assert_eq!(
        downloaded[0]["source_page_url"],
        "https://official.example/brave-source"
    );
    let relative = downloaded[0]["path"].as_str().expect("relative path");
    assert!(root.join(relative).exists());
}

#[test]
fn pass_web_discover_image_reads_brave_key_from_tura_config() {
    let _guard = env_lock_blocking();
    let root = temp_workspace("web-discover-brave-config");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let image_url = format!("http://{addr}/brave-config-image.jpg");
    let endpoint = format!("http://{addr}/brave-config-images");
    let env_path = root.join(".env");
    fs::write(
        &env_path,
        format!(
            "TURA_BRAVE_SEARCH_API_KEY=config-test-key\nTURA_BRAVE_IMAGE_SEARCH_ENDPOINT={endpoint}\n"
        ),
    )
    .expect("write tura env");
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("GET /brave-config-images") {
                assert!(request.contains("q=fixture"));
                let body = json!({
                    "type": "images",
                    "results": [
                        {
                            "type": "image_result",
                            "title": "Config Brave fixture",
                            "source": "https://official.example/config-source",
                            "properties": {
                                "url": image_url
                            }
                        }
                    ]
                })
                .to_string();
                write_http_response(&mut stream, "application/json", &body);
            } else {
                let mut image = image::RgbImage::new(48, 48);
                for (_, _, pixel) in image.enumerate_pixels_mut() {
                    *pixel = image::Rgb([120, 40, 190]);
                }
                let mut bytes = Vec::new();
                image::DynamicImage::ImageRgb8(image)
                    .write_to(
                        &mut std::io::Cursor::new(&mut bytes),
                        image::ImageFormat::Jpeg,
                    )
                    .expect("encode jpeg");
                write_http_response_bytes(&mut stream, "image/jpeg", &bytes);
            }
        }
    });

    std::env::remove_var("TURA_BRAVE_SEARCH_API_KEY");
    std::env::remove_var("BRAVE_API_KEY");
    std::env::remove_var("TURA_BRAVE_IMAGE_SEARCH_ENDPOINT");
    std::env::set_var("TURA_ENV_PATH", &env_path);
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": "--type image --query fixture --max-results 1 --download-dir media/config-brave --min-size 100 --max-size 1000000",
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_ENV_PATH");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["command_type"], "web_discover");
    assert_eq!(output["results"][0]["success"], true);
    let result = &output["results"][0]["output"]["results"][0];
    assert_eq!(result["source"], "brave_images");
    assert_eq!(result["page_url"], "https://official.example/config-source");
}

#[test]
fn pass_web_discover_image_uses_duckduckgo_fallback_without_brave() {
    let _guard = env_lock_blocking();
    let root = temp_workspace("web-discover-ddg-image");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let image_url = format!("http://{addr}/ddg-image.jpg");
    let page_endpoint = format!("http://{addr}/ddg");
    let search_endpoint = format!("http://{addr}/i.js");
    let server = thread::spawn(move || {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("GET /ddg") {
                write_http_response(&mut stream, "text/html", "vqd='fixture-vqd';");
            } else if request.starts_with("GET /i.js") {
                assert!(request.contains("q=fixture"));
                assert!(request.contains("vqd=fixture-vqd"));
                let body = json!({
                    "results": [
                        {
                            "title": "Official fixture from DuckDuckGo",
                            "image": image_url,
                            "url": "https://official.example/ddg-source",
                            "source": "official.example"
                        }
                    ]
                })
                .to_string();
                write_http_response(&mut stream, "application/json", &body);
            } else {
                let mut image = image::RgbImage::new(96, 96);
                for (_, _, pixel) in image.enumerate_pixels_mut() {
                    *pixel = image::Rgb([220, 90, 40]);
                }
                let mut bytes = Vec::new();
                image::DynamicImage::ImageRgb8(image)
                    .write_to(
                        &mut std::io::Cursor::new(&mut bytes),
                        image::ImageFormat::Jpeg,
                    )
                    .expect("encode jpeg");
                write_http_response_bytes(&mut stream, "image/jpeg", &bytes);
            }
        }
    });

    std::env::remove_var("TURA_IMAGE_SEARCH_ENDPOINT");
    std::env::remove_var("TURA_BRAVE_SEARCH_API_KEY");
    std::env::remove_var("BRAVE_API_KEY");
    std::env::set_var("TURA_BRAVE_SEARCH_DISABLED", "1");
    std::env::set_var("TURA_EXA_SEARCH_DISABLED", "1");
    std::env::set_var("TURA_DUCKDUCKGO_IMAGE_PAGE_ENDPOINT", page_endpoint);
    std::env::set_var("TURA_DUCKDUCKGO_IMAGE_SEARCH_ENDPOINT", search_endpoint);
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": "--type image --query fixture --max-results 1 --download-dir media/ddg --min-size 1 --max-size 1000000",
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_BRAVE_SEARCH_DISABLED");
    std::env::remove_var("TURA_EXA_SEARCH_DISABLED");
    std::env::remove_var("TURA_DUCKDUCKGO_IMAGE_PAGE_ENDPOINT");
    std::env::remove_var("TURA_DUCKDUCKGO_IMAGE_SEARCH_ENDPOINT");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["command_type"], "web_discover");
    assert_eq!(output["results"][0]["success"], true);
    let result = &output["results"][0]["output"]["results"][0];
    assert_eq!(result["source"], "duckduckgo_images");
    assert_eq!(result["page_url"], "https://official.example/ddg-source");
    let downloaded = output["results"][0]["output"]["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert_eq!(downloaded.len(), 1);
    let relative = downloaded[0]["path"].as_str().expect("relative path");
    assert!(root.join(relative).exists());
}

#[test]
fn pass_web_discover_image_min_size_filters_small_downloads() {
    let _guard = env_lock_blocking();
    let root = temp_workspace("web-discover-ddg-min-size");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let image_url = format!("http://{addr}/tiny.jpg");
    let page_endpoint = format!("http://{addr}/ddg");
    let search_endpoint = format!("http://{addr}/i.js");
    let server = thread::spawn(move || {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("GET /ddg") {
                write_http_response(&mut stream, "text/html", "vqd='tiny-vqd';");
            } else if request.starts_with("GET /i.js") {
                let body = json!({
                    "results": [
                        {
                            "title": "Tiny fixture",
                            "image": image_url,
                            "url": "https://official.example/tiny-source",
                            "source": "official.example"
                        }
                    ]
                })
                .to_string();
                write_http_response(&mut stream, "application/json", &body);
            } else {
                write_http_response_bytes(&mut stream, "image/jpeg", &[1, 2, 3, 4, 5]);
            }
        }
    });

    std::env::remove_var("TURA_IMAGE_SEARCH_ENDPOINT");
    std::env::remove_var("TURA_BRAVE_SEARCH_API_KEY");
    std::env::remove_var("BRAVE_API_KEY");
    std::env::set_var("TURA_BRAVE_SEARCH_DISABLED", "1");
    std::env::set_var("TURA_EXA_SEARCH_DISABLED", "1");
    std::env::set_var("TURA_DUCKDUCKGO_IMAGE_PAGE_ENDPOINT", page_endpoint);
    std::env::set_var("TURA_DUCKDUCKGO_IMAGE_SEARCH_ENDPOINT", search_endpoint);
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": "--type image --query fixture --max-results 1 --download-dir media/ddg --min-size 100 --max-size 1000000",
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_BRAVE_SEARCH_DISABLED");
    std::env::remove_var("TURA_EXA_SEARCH_DISABLED");
    std::env::remove_var("TURA_DUCKDUCKGO_IMAGE_PAGE_ENDPOINT");
    std::env::remove_var("TURA_DUCKDUCKGO_IMAGE_SEARCH_ENDPOINT");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["command_type"], "web_discover");
    assert_eq!(output["results"][0]["success"], true);
    let downloaded = output["results"][0]["output"]["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert!(downloaded.is_empty());
    assert!(root
        .join("media/ddg")
        .read_dir()
        .expect("read dir")
        .next()
        .is_none());
}

#[test]
fn pass_web_discover_website_download_writes_markdown() {
    let root = temp_workspace("web-discover-website");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let page_url = format!("http://{addr}/page");
    let endpoint = format!("http://{addr}/search");
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]);
            if request.starts_with("POST /search") {
                let body = json!({
                    "results": [
                        {
                            "title": "Fixture Page",
                            "url": page_url,
                            "snippet": "A fixture page"
                        }
                    ]
                })
                .to_string();
                write_http_response(&mut stream, "application/json", &body);
            } else {
                write_http_response(
                    &mut stream,
                    "text/html",
                    "<html><head><title>Fixture Page</title><script>hidden()</script></head><body><h1>Hello Web</h1><p>Clean visible text.</p></body></html>",
                );
            }
        }
    });

    std::env::set_var("TURA_WEB_DISCOVER_ENDPOINT", endpoint);
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": "web_discover website fixture --max-results=1 --download-dir=docs",
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_WEB_DISCOVER_ENDPOINT");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["command_type"], "web_discover");
    assert_eq!(output["results"][0]["success"], true);
    let downloaded = output["results"][0]["output"]["downloaded_files"]
        .as_array()
        .expect("downloaded files");
    assert_eq!(downloaded.len(), 1);
    let relative = downloaded[0]["path"].as_str().expect("relative path");
    assert!(relative.starts_with("docs"));
    let markdown = fs::read_to_string(root.join(relative)).expect("read markdown");
    assert!(markdown.contains("Clean visible text"));
    assert!(!markdown.contains("hidden()"));
}

#[test]
fn pass_web_discover_direct_website_without_download_returns_clean_text_only() {
    let root = temp_workspace("web-discover-direct-text");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let page_url = format!("http://{addr}/page");
    let long_body = format!("{}{}", "A".repeat(900), "B".repeat(900));
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0u8; 4096];
        let _ = stream.read(&mut buffer).expect("read request");
        let body = format!(
            "<html><head><title>Fixture Page</title><script>hidden()</script></head><body><h1>Hello Web</h1><p>{long_body}</p></body></html>"
        );
        write_http_response(&mut stream, "text/html", &body);
    });

    std::env::set_var("TURA_WEB_READER_DISABLED", "1");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "web_discover",
                    "command_line": format!("web_discover website \"{page_url}\""),
                    "step": 1
                }
            ]
        }),
        &root,
    );
    std::env::remove_var("TURA_WEB_READER_DISABLED");
    server.join().expect("server joins");

    assert_eq!(output["results"][0]["success"], true);
    let results = output["results"][0]["output"]["results"]
        .as_array()
        .expect("results");
    assert_eq!(results.len(), 1);
    let text = results[0]
        .as_str()
        .expect("website result should be text only");
    assert!(text.contains("Hello Web"));
    assert!(text.contains("[truncated]"));
    assert!(!text.contains("hidden()"));
    assert!(text.chars().count() <= 1_000);
}

fn write_http_response(stream: &mut std::net::TcpStream, content_type: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .expect("write response");
}

fn write_http_response_bytes(stream: &mut std::net::TcpStream, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).expect("write header");
    stream.write_all(body).expect("write body");
}

#[test]
fn pass_missing_steps_default_to_original_order() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("steps");
    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "shell_command", "command_line": "{\"command\":\"Write-Output one\"}" },
                { "command": "shell_command", "command_line": "{\"command\":\"Write-Output two\"}" }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["step"], 1);
    assert_eq!(output["results"][1]["step"], 2);
}

#[test]
fn pass_top_level_task_status_argument_is_not_model_visible() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("top-level-task-status");

    let output = command_run::execute(
        &json!({
            "task_status": { "status": "done" },
            "commands": [
                { "command": "shell_command", "command_line": json!({ "command": "Write-Output ok" }).to_string() }
            ]
        }),
        &root,
    );

    assert!(output.get("task_status").is_none());
}

#[test]
fn pass_planning_command_routes_through_command_run() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_FORCE_EXECUTE_TOOLS_PLANNING", "1");
    let root = temp_workspace("planning");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "planning",
                    "command_line": "[{\"step\":1,\"task_summary\":\"Inspect files\"},{\"step\":1,\"task_summary\":\"Apply changes\"}]"
                }
            ]
        }),
        &root,
    );

    std::env::remove_var("TURA_FORCE_EXECUTE_TOOLS_PLANNING");

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][0]["command_type"], "planning");
    assert_eq!(
        output["results"][0]["output"]["steps"][0]["task_summary"],
        "Inspect files"
    );
    assert_eq!(output["results"][0]["output"]["steps"][0]["step"], 1);
    assert!(output["results"][0]["output"]["steps"][0]
        .get("deliverable")
        .is_none());
    assert!(output["results"][0]["output"]["steps"][0]
        .get("task_id")
        .is_none());
    assert_eq!(output["results"][0]["output"]["steps"][1]["step"], 2);
}

#[test]
fn pass_task_status_command_inside_command_run_is_not_shell_executed() {
    let root = temp_workspace("task-status");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 1,
                    "command_type": "task_status",
                    "command_line": "{\"status\":\"done\",\"task_summary\":\"Patch code\"}"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "task_status");
    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["output"],
        json!({ "task_status": { "status": "done", "task_summary": "Patch code" } })
    );
}

#[test]
fn pass_task_status_payload_in_command_field_is_recovered() {
    let root = temp_workspace("task-status-command-field");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 1,
                    "command_type": "task_status",
                    "command": "{\"status\":\"done\",\"task_summary\":\"Smoke confirmation\"}"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "task_status");
    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["output"],
        json!({ "task_status": { "status": "done", "task_summary": "Smoke confirmation" } })
    );
}

#[test]
fn pass_task_status_accepts_no_required_arguments() {
    let root = temp_workspace("task-status-empty");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 1,
                    "command_type": "task_status",
                    "command_line": "{}"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "task_status");
    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][0]["output"], json!({ "task_status": {} }));
}

#[test]
fn fail_task_status_rejects_status_outside_question_or_done() {
    let root = temp_workspace("task-status-invalid");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 1,
                    "command_type": "task_status",
                    "command_line": "{\"status\":\"doing\"}"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "task_status");
    assert_eq!(output["results"][0]["success"], false);
    assert_eq!(
        output["results"][0]["error"],
        "task_status status must be question or done"
    );
}

#[test]
fn fail_planning_command_is_unavailable_by_default() {
    let _guard = env_lock_blocking();
    std::env::remove_var("TURA_FORCE_PLANNING");
    std::env::remove_var("TURA_FORCE_EXECUTE_TOOLS_PLANNING");
    let root = temp_workspace("planning-disabled");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "planning",
                    "command_line": "[{\"task_summary\":\"Inspect files\"},{\"task_summary\":\"Apply changes\"}]"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false);
    assert_eq!(
        output["results"][0]["error"],
        "unsupported command_run command"
    );
}

#[tokio::test]
async fn fail_command_run_rejects_commands_outside_agent_capabilities() {
    let root = temp_workspace("allowed-commands");
    let allowed = BTreeSet::from(["shell_command".to_string()]);
    let output = command_run::execute_async_value_with_allowed(
        json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "read_media sample.png"
                }
            ]
        }),
        root,
        Some(allowed),
    )
    .await;

    assert_eq!(output["results"][0]["command_type"], "read_media");
    assert_eq!(output["results"][0]["success"], false);
    assert_eq!(
        output["results"][0]["error"],
        "unsupported command_run command"
    );
}

#[test]
fn pass_compact_context_command_routes_and_outputs_summary() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("compact-context");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 1,
                    "command_type": "shell_command",
                    "command_line": json!({ "command": "Write-Output before-compact" }).to_string()
                },
                {
                    "step": 2,
                    "command_type": "compact_context",
                    "command_line": "{\"summary\":\"Goal done partly. Next read src/lib.rs.\"}"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][1]["command_type"], "compact_context");
    assert_eq!(output["results"][1]["success"], true);
    assert_eq!(
        output["results"][1]["output"]["compact_context"],
        "Goal done partly. Next read src/lib.rs."
    );
}

#[test]
fn fail_compact_context_must_be_final_highest_step() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("compact-context-position");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 2,
                    "command_type": "compact_context",
                    "command_line": "summary"
                },
                {
                    "step": 3,
                    "command_type": "shell_command",
                    "command_line": json!({ "command": "Write-Output after" }).to_string()
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false);
    assert_eq!(
        output["results"][0]["error"],
        "compact_context must be the final command in the highest step of command_run"
    );
}

#[test]
fn pass_shell_command_output_matches_current_code_mode_string() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("shell-output");
    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "shell_command", "command_line": json!({ "command": "Write-Output current-backfill-ok" }).to_string() }
            ]
        }),
        &root,
    );

    let text = output["results"][0]["output"]
        .as_str()
        .expect("shell command_run output should be current-style text");
    assert!(text.starts_with("Exit code: 0\nWall time: "));
    assert!(text.contains("\nOutput:\n"));
    assert!(text.contains("current-backfill-ok"));
    assert!(!text.contains("\"metadata\""));
    assert!(!text.contains("\"stdout\""));
    assert!(!text.contains("\"stderr\""));
}

#[test]
fn pass_model_backfill_matches_current_shape_except_command_type_key() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("model-backfill");
    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "shell_command", "command_line": json!({ "command": "Write-Output command-type-diff-only" }).to_string() }
            ]
        }),
        &root,
    );
    let result = output["results"][0].as_object().expect("result object");
    let mut keys = result.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    assert_eq!(keys, vec!["command_type", "output", "step", "success"]);

    let mut current_equivalent = output.clone();
    let result = current_equivalent["results"][0]
        .as_object_mut()
        .expect("result object");
    let command_type = result.remove("command_type").expect("command_type");
    result.insert("command".to_string(), command_type);

    let expected = json!({
        "results": [
            {
                "step": 1,
                "command": commands::active_shell_command_name(),
                "success": true,
                "output": current_equivalent["results"][0]["output"].clone()
            }
        ]
    });
    assert_eq!(current_equivalent, expected);
}

#[test]
fn pass_command_only_shell_text_is_mapped_to_active_shell_command() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("command-only-shell");
    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "Write-Output ok", "step": 1 }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["command_type"],
        commands::active_shell_command_name()
    );
}

#[test]
fn pass_top_level_workdir_is_accepted_for_current_style_shell_items() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("top-level-workdir");
    let output = command_run::execute(
        &json!({
            "workdir": ".",
            "commands": [
                { "command": "Write-Output ok", "step": 1 }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
}

#[test]
fn pass_unknown_command_with_shell_payload_is_mapped_to_active_shell_command() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("unknown-command-payload");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "Get-Content src/app.py",
                    "command_line": json!({ "command": "Write-Output mapped-ok" }).to_string(),
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["command_type"],
        commands::active_shell_command_name()
    );
}

#[test]
fn pass_unknown_command_without_payload_runs_command_text_as_shell() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("unknown-command-no-payload");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "Write-Output raw-command-ok",
                    "command_line": "",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["command_type"],
        commands::active_shell_command_name()
    );
}

#[test]
fn pass_command_line_without_command_defaults_to_active_shell_command() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("command-line-only");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_line": json!({ "command": "Write-Output command-line-only-ok" }).to_string(),
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["command_type"],
        commands::active_shell_command_name()
    );
}

#[test]
fn pass_command_line_without_command_type_accepts_workdir_and_timeout() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("default-shell-workdir");
    let subdir = root.join("subdir");
    fs::create_dir_all(&subdir).expect("temp subdir");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_line": json!({ "command": "Get-Location", "timeout_ms": 5000 }).to_string(),
                    "workdir": "subdir",
                    "timeout_ms": 5000,
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["command_type"],
        commands::active_shell_command_name()
    );
    assert!(output["results"][0]["output"]
        .as_str()
        .is_some_and(|text| text.replace('\\', "/").contains("/subdir")));
}

#[test]
fn pass_legacy_steps_shape_is_accepted() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("legacy-steps");
    let output = command_run::execute(
        &json!({
            "steps": [
                {
                    "tool_name": "shell_command",
                    "command_code": json!({ "command": "Write-Output legacy-steps-ok" }).to_string(),
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["command_type"],
        commands::active_shell_command_name()
    );
}

#[test]
fn pass_command_run_arguments_accept_requests_wrapper_and_json_fence() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("json-fence");
    let output = command_run::execute(
        &Value::String(
            "```json\n{\"requests\":{\"commands\":[{\"command\":\"shell_command\",\"command_line\":\"{\\\"command\\\":\\\"Write-Output fenced-ok\\\",\\\"timeout_ms\\\":5000}\",\"step\":1}]}}\n```"
                .to_string(),
        ),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
}

#[test]
fn pass_apply_patch_success_and_fail_context_mismatch() {
    let root = temp_workspace("patch");
    fs::write(root.join("app.txt"), "old\n").expect("fixture should be written");

    let pass = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "apply_patch",
                    "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n"
                }
            ]
        }),
        &root,
    );
    assert_eq!(pass["results"][0]["success"], true);
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("patched file should be readable"),
        "new\n"
    );

    let fail = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "apply_patch",
                    "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-missing\n+value\n*** End Patch\n"
                }
            ]
        }),
        &root,
    );
    assert_eq!(fail["results"][0]["success"], false);
}

#[test]
fn pass_apply_patch_add_delete_and_move_are_tracked_in_output() {
    let root = temp_workspace("patch-add-delete-move");
    fs::write(root.join("move-me.txt"), "alpha\n").expect("move fixture should be written");
    fs::write(root.join("delete-me.txt"), "gone\n").expect("delete fixture should be written");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 1,
                    "command": "apply_patch",
                    "command_line": "*** Begin Patch\n*** Add File: added.txt\n+hello\n*** Update File: move-me.txt\n*** Move to: moved.txt\n@@\n-alpha\n+beta\n*** Delete File: delete-me.txt\n*** End Patch\n"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        fs::read_to_string(root.join("added.txt")).expect("added file should be readable"),
        "hello\n"
    );
    assert!(!root.join("move-me.txt").exists());
    assert_eq!(
        fs::read_to_string(root.join("moved.txt")).expect("moved file should be readable"),
        "beta\n"
    );
    assert!(!root.join("delete-me.txt").exists());
    let changes = output["results"][0]["output"]["changes"]
        .as_array()
        .expect("changes should be an array");
    assert!(changes.iter().any(|change| change["kind"] == "add"));
    assert!(changes
        .iter()
        .any(|change| change["move_path"] == "moved.txt"));
    assert!(changes.iter().any(|change| change["kind"] == "delete"));
}

#[test]
fn fail_apply_patch_rejects_path_outside_workspace() {
    let root = temp_workspace("patch-outside");
    let outside = root
        .parent()
        .expect("temp workspace should have a parent")
        .join("outside-command-run-test.txt");
    let _ = fs::remove_file(&outside);

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "apply_patch",
                    "command_line": format!("*** Begin Patch\n*** Add File: {}\n+bad\n*** End Patch\n", outside.display())
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false);
    assert!(output["results"][0]["output"]["stderr"]
        .as_str()
        .unwrap_or_default()
        .contains("outside"));
    assert!(!outside.exists());
}

#[test]
fn pass_shell_embedded_apply_patch_is_intercepted_before_shell_execution() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("embedded-patch");
    fs::write(root.join("app.txt"), "old\n").expect("fixture should be written");
    let command_line = "@'\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n'@ | apply_patch";

    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "shell_command", "command_line": json!({ "command": command_line }).to_string() }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("patched file should be readable"),
        "new\n"
    );
}

#[test]
fn pass_command_line_wrapped_apply_patch_routes_to_apply_patch() {
    let root = temp_workspace("patch-payload-route");
    fs::write(root.join("app.txt"), "old\n").expect("fixture");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "shell_command",
                    "command_line": "apply_patch <<'PATCH'\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\nPATCH",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][0]["command_type"], "apply_patch");
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("patched file should be readable"),
        "new\n"
    );
}

#[test]
fn pass_aliases_cmd_and_command_line_are_accepted() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("aliases");
    let output = command_run::execute(
        &json!({
            "commands": [
                { "cmd": "shell_command", "commandLine": json!({ "command": "Write-Output ok" }).to_string(), "step": 1 }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][0]["command_type"], "shell_command");
}

#[test]
fn pass_single_shell_object_without_commands_is_wrapped() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("single-shell-object");
    let output = command_run::execute(
        &json!({
            "command": json!({ "command": "Write-Output ok", "timeout_ms": 5000 }).to_string(),
            "timeoutMs": 120000
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
}

#[test]
fn pass_command_only_here_string_patch_is_routed_to_apply_patch() {
    let root = temp_workspace("patch-route");
    fs::write(root.join("app.txt"), "old\n").expect("fixture");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "@'\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n'@",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][0]["command_type"], "apply_patch");
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("patched file should be readable"),
        "new\n"
    );
}

#[test]
fn fail_later_batch_commands_stop_after_apply_patch_failure() {
    let root = temp_workspace("patch-failure-stop");
    fs::write(root.join("app.txt"), "actual\n").expect("fixture");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "apply_patch",
                    "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-missing\n+new\n*** End Patch\n",
                    "step": 1
                },
                {
                    "command": "shell_command",
                    "command_line": "echo after",
                    "step": 1
                },
                {
                    "command": "shell_command",
                    "command_line": "echo next-step",
                    "step": 2
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["cancelled"], true);
    assert!(output["cancel_reason"]
        .as_str()
        .is_some_and(|text| text.contains("apply_patch failed")));
    assert_eq!(output["results"].as_array().expect("results").len(), 1);
    assert_eq!(output["results"][0]["success"], false);
    assert_eq!(
        output["results"][0]["output"]["output"]["error_type"],
        "ContextMismatch"
    );
}

#[test]
fn pass_streaming_executor_returns_apply_patch_result_without_finish() {
    let root = temp_workspace("streaming-immediate");
    fs::write(root.join("app.txt"), "old\n").expect("fixture");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut executor = command_run::StreamingCommandRunExecutor::new(root.clone());

    let immediate = runtime.block_on(executor.push_command_value(json!({
        "command": "apply_patch",
        "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n",
        "step": 1
    })));

    assert_eq!(immediate.len(), 1);
    assert_eq!(immediate[0]["command_type"], "apply_patch");
    assert_eq!(immediate[0]["success"], true);
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("patched file should be readable"),
        "new\n"
    );
    let final_results = runtime.block_on(executor.finish());
    assert!(final_results.is_empty());
}

#[test]
fn pass_streaming_executor_strips_apply_patch_tool_prefix() {
    let root = temp_workspace("streaming-prefixed-patch");
    fs::write(root.join("app.txt"), "old\n").expect("fixture");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut executor = command_run::StreamingCommandRunExecutor::new(root.clone());

    let immediate = runtime.block_on(executor.push_command_value(json!({
        "command_type": "apply_patch",
        "command_line": "apply_patch\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n",
        "step": 1
    })));

    assert_eq!(immediate.len(), 1);
    assert_eq!(immediate[0]["command_type"], "apply_patch");
    assert_eq!(immediate[0]["success"], true);
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("patched file should be readable"),
        "new\n"
    );
}

#[test]
fn fail_streaming_executor_ignores_commands_after_failed_apply_patch() {
    let root = temp_workspace("streaming-patch-stop");
    fs::write(root.join("app.txt"), "actual\n").expect("fixture");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut executor = command_run::StreamingCommandRunExecutor::new(root.clone());

    let failed = runtime.block_on(executor.push_command_value(json!({
        "command": "apply_patch",
        "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-missing\n+new\n*** End Patch\n",
        "step": 1
    })));
    let ignored = runtime.block_on(executor.push_command_value(json!({
        "command": "shell_command",
        "command_line": "echo after",
        "step": 1
    })));
    let final_results = runtime.block_on(executor.finish());

    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["command_type"], "apply_patch");
    assert_eq!(failed[0]["success"], false);
    assert!(ignored.is_empty());
    assert!(final_results.is_empty());
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("fixture file should be readable"),
        "actual\n"
    );
}

#[test]
fn pass_streaming_executor_exposes_output_deltas_before_command_finishes() {
    let root = temp_workspace("streaming-output-deltas");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut executor = command_run::StreamingCommandRunExecutor::new(root);
    let event_ctx = executor.event_context();
    let command_line = if cfg!(windows) {
        "Write-Output 'stream-live-1'; Start-Sleep -Seconds 2; Write-Output 'stream-live-2'"
    } else {
        "printf 'stream-live-1\\n'; sleep 2; printf 'stream-live-2\\n'"
    };
    let handle = thread::spawn(move || {
        runtime.block_on(executor.push_command_value(json!({
            "command_type": commands::active_shell_command_name(),
            "command_line": command_line,
            "timeout_ms": 8000,
            "step": 1
        })))
    });

    let deadline = Instant::now() + Duration::from_secs(1);
    let mut saw_delta_before_finish = false;
    while Instant::now() < deadline {
        saw_delta_before_finish = event_ctx.events().iter().any(|event| {
            matches!(
                event,
                ToolRuntimeEvent::OutputDelta { text, .. } if text.contains("stream-live-1")
            )
        });
        if saw_delta_before_finish {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    assert!(
        saw_delta_before_finish,
        "expected stdout delta before the shell command finished"
    );
    let results = handle.join().expect("streaming command thread");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["success"], true);
}

#[test]
fn pass_mutating_commands_are_barriers_between_read_batches() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("barrier");
    fs::write(root.join("state.txt"), "before\n").expect("state fixture should be written");

    let output = command_run::execute(
        &json!({
            "commands": [
                { "step": 1, "command": "shell_command", "command_line": json!({ "command": "Get-Content state.txt" }).to_string() },
                {
                    "step": 1,
                    "command": "apply_patch",
                    "command_line": "*** Begin Patch\n*** Update File: state.txt\n@@\n-before\n+after\n*** End Patch\n"
                },
                { "step": 1, "command": "shell_command", "command_line": json!({ "command": "Get-Content state.txt" }).to_string() }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][1]["success"], true);
    assert_eq!(output["results"][2]["success"], true);
    assert!(output["results"][0]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("before"));
    assert!(output["results"][2]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("after"));
}

#[test]
fn pass_same_step_commands_are_extended_to_unique_order() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("unique-read-step");
    fs::write(root.join("state.txt"), "ready\n").expect("state fixture should be written");
    let command_a = if cfg!(windows) {
        "Test-Path state.txt; Write-Output read-a"
    } else {
        "pwd; echo read-a"
    };
    let command_b = if cfg!(windows) {
        "Test-Path state.txt; Write-Output read-b"
    } else {
        "pwd; echo read-b"
    };

    let output = command_run::execute(
        &json!({
            "commands": [
                { "step": 1, "command": "shell_command", "command_line": json!({ "command": command_a, "timeout_ms": 5000 }).to_string() },
                { "step": 1, "command": "shell_command", "command_line": json!({ "command": command_b, "timeout_ms": 5000 }).to_string() }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][1]["success"], true);
    assert_eq!(output["results"][0]["step"], 1);
    assert_eq!(output["results"][1]["step"], 2);
}

#[test]
fn pass_file_lock_allows_parallel_reads_and_blocks_write() {
    let read_access = Access {
        read_paths: vec!["same.txt".to_string()],
        ..Access::default()
    };
    let write_access = Access {
        write_paths: vec!["same.txt".to_string()],
        ..Access::default()
    };
    let read_a = file_locks::acquire(&read_access);
    let read_b = file_locks::acquire(&read_access);
    let started = Instant::now();
    let writer = std::thread::spawn(move || {
        let _write = file_locks::acquire(&write_access);
        started.elapsed()
    });

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        !writer.is_finished(),
        "write lock must wait for active readers"
    );
    drop(read_a);
    assert!(
        !writer.is_finished(),
        "write lock must wait for all readers"
    );
    drop(read_b);
    let waited = writer.join().expect("writer thread should finish");
    assert!(waited >= Duration::from_millis(200));
}

#[test]
fn pass_timeout_returns_quick_failure() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("timeout");
    let command = if cfg!(windows) {
        "Start-Sleep -Seconds 10"
    } else {
        "sleep 10"
    };
    let started = Instant::now();
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "shell_command",
                    "command_line": json!({ "command": command, "timeout_ms": 500 }).to_string()
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false);
    assert!(output["results"][0]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("Timed out after"));
    assert!(
        output["results"][0].get("error").is_none(),
        "timeout must be returned by the shell runtime as model-visible tool output, not by dropping command_run dispatch"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn fail_timeout_kills_descendant_process_tree_quickly() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");
    let root = temp_workspace("descendant-timeout");
    let started = Instant::now();
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "bash",
                    "command_line": json!({ "command": "sleep 10", "timeout_ms": 500 }).to_string()
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false);
    assert!(output["results"][0]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("Timed out after"));
    assert!(
        output["results"][0].get("error").is_none(),
        "descendant timeout must be converted by the shell runtime instead of outer command_run timeout"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn pass_async_command_run_entry_does_not_start_nested_runtime() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("async-entry");
    let output = command_run::execute_async_value(
        json!({
            "commands": [
                { "command": "shell_command", "command_line": json!({ "command": "Write-Output async-ok" }).to_string() }
            ]
        }),
        root,
    )
    .await;

    assert_eq!(output["results"][0]["success"], true);
    assert!(output["results"][0]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("async-ok"));
}

#[test]
fn pass_bash_surface_runs_posix_script_without_exposing_shell_command() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");
    let root = temp_workspace("bash-script");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "bash",
                    "command_line": json!({ "command": "for x in one two; do echo $x; done", "timeout_ms": 5000 }).to_string()
                }
            ]
        }),
        &root,
    );

    assert_eq!(commands::canonical_command("shell_command"), "bash");
    assert!(output["results"][0].get("command").is_none());
    assert_eq!(output["results"][0]["command_type"], "bash");
    assert_eq!(output["results"][0]["success"], true);
    assert!(output["results"][0]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("one"));
}

#[test]
fn pass_shell_surface_isolation_canonicalizes_to_one_active_shell() {
    let _guard = env_lock_blocking();

    std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");
    assert_eq!(commands::canonical_command("shell_command"), "bash");
    assert_eq!(commands::canonical_command("shll"), "bash");
    assert_eq!(commands::canonical_command("bash"), "bash");

    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shll");
    assert_eq!(commands::canonical_command("bash"), "shell_command");
    assert_eq!(
        commands::canonical_command("shell_command"),
        "shell_command"
    );
}

#[tokio::test]
async fn fail_pre_tool_hook_blocks_tool_before_runtime() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("pre-hook");
    let ctx = ToolContext::new(root);
    ctx.set_pre_hook(|call| {
        Err(ToolError::RespondToModel(format!(
            "blocked by hook: {}",
            call.tool_name
        )))
    });
    let router = ToolRouter::new();
    let call = ToolCall {
        tool_name: "shell_command".to_string(),
        call_id: "call_pre_hook".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "command": "Write-Output should-not-run", "timeout_ms": 5000 }),
        },
    };

    let error = router
        .dispatch(call, ctx.clone(), false)
        .await
        .expect_err("pre hook should block dispatch");

    assert!(error.to_string().contains("blocked by hook"));
    assert!(
        ctx.events().is_empty(),
        "pre hook should run before tool-started events"
    );
}

#[tokio::test]
async fn pass_post_tool_hook_can_replace_model_visible_response() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("post-hook");
    let ctx = ToolContext::new(root);
    ctx.set_post_hook(|_call, output: &mut FunctionToolOutput| {
        output.body = json!({ "output": "replaced by post hook", "metadata": { "exit_code": 0 } });
        output.success = Some(true);
        Ok(())
    });
    let router = ToolRouter::new();
    let call = ToolCall {
        tool_name: "shell_command".to_string(),
        call_id: "call_post_hook".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "command": "Write-Output original", "timeout_ms": 5000 }),
        },
    };

    let result = router
        .dispatch(call, ctx.clone(), false)
        .await
        .expect("post hook should allow dispatch");

    assert_eq!(
        result.result.code_mode_result()["output"],
        "replaced by post hook"
    );
    assert!(ctx.events().iter().any(|event| matches!(
        event,
        ToolRuntimeEvent::ToolFinished {
            call_id,
            success: true,
            ..
        } if call_id == "call_post_hook"
    )));
}

#[tokio::test]
async fn pass_shell_runtime_records_stdout_stderr_delta_events() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("stream-delta");
    let command = if cfg!(windows) {
        "Write-Output out-delta; [Console]::Error.WriteLine('err-delta')"
    } else {
        "echo out-delta; echo err-delta >&2"
    };
    let ctx = ToolContext::new(root);
    let router = ToolRouter::new();
    let call = ToolCall {
        tool_name: "shell_command".to_string(),
        call_id: "call_delta".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "command": command, "timeout_ms": 5000 }),
        },
    };

    let result = router
        .dispatch(call, ctx.clone(), false)
        .await
        .expect("streaming command should succeed");

    assert_eq!(result.result.success, Some(true));
    let events = ctx.events();
    assert!(events.iter().any(|event| matches!(
        event,
        ToolRuntimeEvent::OutputDelta {
            call_id,
            stream,
            text,
        } if call_id == "call_delta" && stream == "stdout" && text.contains("out-delta")
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        ToolRuntimeEvent::OutputDelta {
            call_id,
            stream,
            text,
        } if call_id == "call_delta" && stream == "stderr" && text.contains("err-delta")
    )));
}

#[tokio::test]
async fn fail_turn_cancellation_aborts_running_shell_command() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("cancel");
    let command = if cfg!(windows) {
        "Start-Sleep -Seconds 10"
    } else {
        "sleep 10"
    };
    let ctx = ToolContext::new(root);
    let cancellation = ctx.cancellation.clone();
    let router = ToolRouter::new();
    let call = ToolCall {
        tool_name: "shell_command".to_string(),
        call_id: "call_cancel".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "command": command, "timeout_ms": 30000 }),
        },
    };
    let started = Instant::now();
    let task = tokio::spawn(async move { router.dispatch(call, ctx, false).await });

    tokio::time::sleep(Duration::from_millis(200)).await;
    cancellation.cancel();
    let result = task
        .await
        .expect("dispatch task should join")
        .expect("dispatch should return model-visible failure output");

    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(result.result.success, Some(false));
    assert!(result.result.code_mode_result()["stderr"]
        .as_str()
        .unwrap_or_default()
        .contains("tool task aborted"));
}

#[tokio::test]
async fn fail_timeout_aborts_reader_drain_for_pipe_holding_descendants() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");
    let root = temp_workspace("reader-drain");
    let router = ToolRouter::new();
    let call = ToolCall {
        tool_name: "bash".to_string(),
        call_id: "call_drain".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "command": "sh -c 'sleep 10 & wait'", "timeout_ms": 500 }),
        },
    };
    let started = Instant::now();

    let result = router
        .dispatch(call, ToolContext::new(root), false)
        .await
        .expect("timeout should be reported as tool output");

    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(result.result.success, Some(false));
    assert!(result.result.code_mode_result()["stderr"]
        .as_str()
        .unwrap_or_default()
        .contains("Timed out"));
}
