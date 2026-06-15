use super::helpers::*;

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
