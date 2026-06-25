use serde_json::{json, Value};
use std::io::Cursor;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use tura_command_read_media::{access, execute};

#[test]
fn read_media_business_flow_expands_directory_reports_failures_and_access_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    let docs = dir.path().join("docs");
    std::fs::create_dir(&docs).expect("create docs");
    std::fs::write(docs.join("first.txt"), "first local document").expect("write first");
    std::fs::write(docs.join("second.md"), "second local document").expect("write second");

    let access = access("read_media docs missing.txt --max-files 10", dir.path());
    assert_eq!(
        access.read_paths,
        vec!["docs".to_string(), "missing.txt".to_string()]
    );
    assert!(access.write_paths.is_empty());
    assert!(!access.workspace_write);

    let response = execute("read_media docs missing.txt --max-files 10", dir.path());
    assert!(response.success);
    assert_eq!(response.exit_code, 0);
    assert!(response.stderr.is_empty());
    assert!(response.stdout.contains("missing.txt: failed"));
    assert!(response.stdout.contains("document"));
    assert!(response.changes.is_empty());

    let results = response.output["media_results"]
        .as_array()
        .expect("media results");
    assert_eq!(results.len(), 3);
    assert_result(results, "docs/first.txt", true);
    assert_result(results, "docs/second.md", true);
    let missing = result_by_path(results, "missing.txt");
    assert_eq!(missing["success"], false);
    assert!(missing["error"]
        .as_str()
        .is_some_and(|error| error.contains("media path does not exist")));
}

#[test]
fn read_media_business_protocol_binary_accepts_json_arguments_and_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("notes.txt"), "hello from protocol").expect("write notes");

    let response = run_protocol(json!({
        "kind": "execute",
        "payload": {
            "session_dir": dir.path().display().to_string(),
            "arguments": {
                "paths": ["notes.txt"],
                "include_text": true,
                "max_text_chars": 100
            }
        }
    }));

    assert_eq!(response["ok"], true);
    assert_eq!(response["success"], true);
    assert_eq!(response["exit_code"], 0);
    assert_eq!(response["output"]["media_results"][0]["path"], "notes.txt");
    assert_eq!(
        response["output"]["media_results"][0]["extracted_text"],
        "hello from protocol"
    );

    let unsupported = run_protocol(json!({
        "kind": "execute",
        "payload": {
            "session_dir": dir.path().display().to_string(),
            "arguments": "--not-a-real-option notes.txt"
        }
    }));
    assert_eq!(unsupported["ok"], true);
    assert_eq!(unsupported["success"], false);
    assert_eq!(unsupported["exit_code"], 1);
    assert!(unsupported["stderr"]
        .as_str()
        .is_some_and(|stderr| stderr.contains("unsupported read_media option")));
}

#[test]
fn read_media_business_flow_reads_image_preview_and_binary_document_attachment() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("sample.png"), png_bytes()).expect("write png");
    std::fs::write(dir.path().join("archive.zip"), b"PK\x03\x04fixture").expect("write archive");

    let image = execute(
        r#"{"paths":["sample.png"],"max_side":128,"max_visuals":1}"#,
        dir.path(),
    );
    assert!(image.success, "image read failed: {}", image.stderr);
    let image_result = &image.output["media_results"][0];
    assert_eq!(image_result["success"], true);
    assert_eq!(image_result["media_type"], "image");
    assert_eq!(image_result["visual_preview_count"], 1);
    let preview_url = image_result["visual_previews"][0]["image_url"]["url"]
        .as_str()
        .expect("image preview data url");
    assert!(preview_url.starts_with("data:image/jpeg;base64,"));
    assert!(image.stdout.contains("sample.png: image"));

    let archive = execute(
        r#"{"paths":["archive.zip"],"include_text":false,"document_attachment_bytes":100000}"#,
        dir.path(),
    );
    assert!(archive.success, "archive read failed: {}", archive.stderr);
    let archive_result = &archive.output["media_results"][0];
    assert_eq!(archive_result["success"], true);
    assert_eq!(archive_result["media_type"], "document");
    assert_eq!(archive_result["file_attachment_count"], 1);
    assert_eq!(
        archive_result["file_attachments"][0]["mime_type"],
        "application/zip"
    );
    assert!(archive_result["file_attachments"][0]["data_base64"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
}

#[test]
fn read_media_business_flow_reads_minimal_pdf_text_without_external_rendering_dependency() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("local.pdf"), minimal_pdf_bytes()).expect("write pdf");

    let response = execute(
        r#"{"paths":["local.pdf"],"include_text":true,"max_visuals":0,"pdf_pages":1,"max_text_chars":2000}"#,
        dir.path(),
    );

    assert!(
        response.success,
        "minimal pdf should be handled through the local text path: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    let result = &response.output["media_results"][0];
    assert_eq!(result["success"], true);
    assert_eq!(result["media_type"], "pdf");
    assert_eq!(result["visual_preview_count"], 0);
    assert_eq!(result["visual_previews"], json!([]));
    assert!(result["extracted_text"]
        .as_str()
        .is_some_and(|text| text.contains("Local PDF business fixture")));
    assert!(response.stdout.contains("local.pdf: pdf"));
}

#[test]
fn read_media_business_flow_handles_unicode_space_paths_aliases_and_repeated_protocol_calls() {
    let dir = tempfile::tempdir().expect("tempdir");
    let nested = dir.path().join("nested folder");
    std::fs::create_dir(&nested).expect("create nested folder");
    std::fs::write(dir.path().join("资料 space.txt"), "alpha unicode path")
        .expect("write unicode path");
    std::fs::write(nested.join("emoji-😀.md"), "beta nested unicode")
        .expect("write nested unicode path");

    let response = execute(
        r#"read-media --path "资料 space.txt" --path "nested folder/emoji-😀.md" --max-files 10"#,
        dir.path(),
    );
    assert!(
        response.success,
        "unicode path read failed: {}",
        response.stderr
    );
    let results = response.output["media_results"]
        .as_array()
        .expect("media results");
    assert_eq!(results.len(), 2);
    assert_eq!(
        normalize_path(results[0]["path"].as_str().expect("first path")),
        "资料 space.txt"
    );
    assert_eq!(
        normalize_path(results[1]["path"].as_str().expect("second path")),
        "nested folder/emoji-😀.md"
    );
    assert_eq!(results[0]["success"], true);
    assert_eq!(results[1]["success"], true);
    assert_eq!(results[0]["media_type"], "document");
    assert_eq!(results[1]["media_type"], "document");
    assert!(response.stdout.contains("资料 space.txt: document"));
    assert!(response.stdout.contains("emoji-😀.md: document"));

    for round in 0..3 {
        let protocol = run_protocol(json!({
            "kind": "execute",
            "payload": {
                "session_dir": dir.path().display().to_string(),
                "arguments": ["资料 space.txt"]
            }
        }));
        assert_eq!(protocol["ok"], true, "protocol round {round}");
        assert_eq!(protocol["success"], true, "protocol round {round}");
        assert_eq!(
            normalize_path(
                protocol["output"]["media_results"][0]["path"]
                    .as_str()
                    .expect("protocol first path")
            ),
            "资料 space.txt"
        );
        assert_eq!(
            protocol["output"]["media_results"][0]["extracted_text"],
            "alpha unicode path"
        );
    }
}

#[test]
fn read_media_business_flow_directory_expansion_respects_newest_first_max_files_limit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let media = dir.path().join("media");
    std::fs::create_dir(&media).expect("create media dir");
    std::fs::write(media.join("old.txt"), "old document").expect("write old");
    std::thread::sleep(std::time::Duration::from_millis(150));
    std::fs::write(media.join("middle.txt"), "middle document").expect("write middle");
    std::thread::sleep(std::time::Duration::from_millis(150));
    std::fs::write(media.join("new.txt"), "new document").expect("write new");

    let response = execute(
        r#"{"paths":["media"],"maxFiles":2,"includeText":true}"#,
        dir.path(),
    );

    assert!(
        response.success,
        "directory expansion should succeed: {}",
        response.stderr
    );
    let results = response.output["media_results"]
        .as_array()
        .expect("media results");
    assert_eq!(
        results.len(),
        2,
        "maxFiles should cap directory expansion before processing"
    );
    let paths = results
        .iter()
        .map(|item| normalize_path(item["path"].as_str().expect("result path")))
        .collect::<Vec<_>>();
    assert_eq!(paths, vec!["media/new.txt", "media/middle.txt"]);
    assert!(results.iter().all(|item| item["success"] == true));
    assert!(
        !response.stdout.contains("old.txt"),
        "oldest file should not be processed after maxFiles cap"
    );
}

#[test]
fn read_media_business_flow_directory_expansion_uses_path_tiebreaker_for_equal_timestamps() {
    let dir = tempfile::tempdir().expect("tempdir");
    let media = dir.path().join("media");
    std::fs::create_dir(&media).expect("create media dir");
    for (name, body) in [
        ("zeta.txt", "zeta document"),
        ("alpha.txt", "alpha document"),
        ("middle.txt", "middle document"),
    ] {
        let path = media.join(name);
        std::fs::write(&path, body).expect("write tied timestamp fixture");
        let tied = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        filetime::set_file_mtime(&path, tied).expect("set tied mtime");
    }

    let response = execute(
        r#"{"paths":["media"],"maxFiles":3,"includeText":true}"#,
        dir.path(),
    );

    assert!(
        response.success,
        "directory tie expansion should succeed: {}",
        response.stderr
    );
    let results = response.output["media_results"]
        .as_array()
        .expect("media results");
    let paths = results
        .iter()
        .map(|item| normalize_path(item["path"].as_str().expect("result path")))
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec!["media/alpha.txt", "media/middle.txt", "media/zeta.txt"],
        "equal mtimes should fall back to workspace-relative path ordering"
    );
    assert!(results.iter().all(|item| item["success"] == true));
}

#[test]
fn read_media_business_protocol_access_and_limits_are_stable_for_json_payloads() {
    let dir = tempfile::tempdir().expect("tempdir");
    let text = format!("{}{}", "a".repeat(1400), "z".repeat(1400));
    std::fs::write(dir.path().join("long.txt"), text).expect("write long text");
    std::fs::write(dir.path().join("sample.png"), png_bytes()).expect("write png");

    let access = run_protocol(json!({
        "kind": "access",
        "payload": {
            "session_dir": dir.path().display().to_string(),
            "arguments": {
                "paths": ["long.txt", "sample.png"],
                "maxTextChars": "1000",
                "maxVisuals": "1"
            }
        }
    }));
    assert_eq!(access["ok"], true);
    assert_eq!(
        access["output"]["read_paths"],
        json!(["long.txt", "sample.png"])
    );
    assert_eq!(access["output"]["write_paths"], json!([]));
    assert_eq!(access["output"]["workspace_write"], false);

    let response = run_protocol(json!({
        "kind": "execute",
        "payload": {
            "session_dir": dir.path().display().to_string(),
            "arguments": {
                "paths": ["long.txt"],
                "includeText": true,
                "maxTextChars": "1000"
            }
        }
    }));
    assert_eq!(response["ok"], true);
    assert_eq!(response["success"], true);
    let extracted = response["output"]["media_results"][0]["extracted_text"]
        .as_str()
        .expect("extracted text");
    assert!(extracted.contains("[read_media text truncated]"));
    assert!(extracted.len() < 2_900);
}

#[test]
fn read_media_business_protocol_health_capabilities_and_unknown_kind_are_stable() {
    let health = run_protocol(json!({
        "kind": "health_check",
        "payload": {}
    }));
    assert_eq!(health["ok"], true);
    assert_eq!(health["success"], true);
    assert_eq!(health["exit_code"], 0);
    assert_eq!(health["output"]["status"], "ok");

    let capabilities = run_protocol(json!({
        "kind": "capabilities",
        "payload": {}
    }));
    assert_eq!(capabilities["ok"], true);
    assert_eq!(capabilities["success"], true);
    assert_eq!(capabilities["output"]["id"], "read_media");
    assert_eq!(capabilities["output"]["supports_macro_command"], true);
    assert_eq!(capabilities["output"]["mutating"], false);

    let unknown = run_protocol(json!({
        "kind": "not_a_read_media_operation",
        "payload": {}
    }));
    assert_eq!(unknown["ok"], false);
    assert_eq!(unknown["success"], false);
    assert_eq!(unknown["exit_code"], 1);
    assert!(unknown["output"]["error"]
        .as_str()
        .is_some_and(|error| error.contains("unsupported protocol kind")));
}

#[test]
fn read_media_business_flow_omits_oversized_binary_document_without_upload() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive_path = dir.path().join("large.zip");
    std::fs::write(&archive_path, vec![b'x'; 120_000]).expect("write large archive");

    let response = execute(
        r#"{"paths":["large.zip"],"include_text":false,"document_attachment_bytes":100000}"#,
        dir.path(),
    );

    assert!(
        response.success,
        "large archive should be handled: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    let result = &response.output["media_results"][0];
    assert_eq!(result["success"], true);
    assert_eq!(result["media_type"], "document");
    assert_eq!(result["file_attachment_count"], 0);
    assert_eq!(result["file_attachments"], json!([]));
    assert_eq!(result["extracted_text"], "");
    assert!(response
        .stdout
        .contains("large.zip: document, 0 visual previews"));
}

#[test]
fn read_media_business_flow_reports_unknown_binary_document_as_text_warning() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("opaque.dat"),
        [0xff, 0xfe, 0xfd, 0x00, 0x80],
    )
    .expect("write opaque file");

    let response = execute(
        r#"{"paths":["opaque.dat"],"include_text":true,"document_attachment_bytes":100000}"#,
        dir.path(),
    );

    assert!(
        response.success,
        "unknown binary should be safe: {}",
        response.stderr
    );
    let result = &response.output["media_results"][0];
    assert_eq!(result["success"], true);
    assert_eq!(result["media_type"], "document");
    assert_eq!(result["file_attachment_count"], 0);
    assert!(result["extracted_text"]
        .as_str()
        .is_some_and(|text| text.contains("could not be decoded as text")));
}

#[test]
fn read_media_business_flow_returns_per_item_failure_for_unreadable_audio() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("broken.wav"), b"not a wav stream").expect("write wav");

    let response = execute(
        r#"{"paths":["broken.wav"],"audio_preview_bytes":100000}"#,
        dir.path(),
    );

    assert!(
        response.success,
        "per-item media errors should not fail the command envelope: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    let result = &response.output["media_results"][0];
    assert_eq!(result["path"], "broken.wav");
    assert_eq!(result["success"], false);
    assert!(result["error"].as_str().is_some_and(|error| {
        error.contains("audio extraction unavailable")
            || error.contains("audio extraction failed")
            || error.contains("failed to run ffmpeg")
    }));
    assert!(response.stdout.contains("broken.wav: failed"));
}

#[test]
fn read_media_business_flow_extracts_audio_preview_with_local_ffmpeg_and_clamped_limit() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let fake_ffmpeg = write_fake_ffmpeg(dir.path());
    let previous_ffmpeg = std::env::var_os("TURA_READ_MEDIA_FFMPEG");
    let previous_ffmpeg_path = std::env::var_os("FFMPEG_PATH");
    std::env::set_var("TURA_READ_MEDIA_FFMPEG", &fake_ffmpeg);
    std::env::remove_var("FFMPEG_PATH");
    std::fs::write(dir.path().join("voice.mp3"), b"local fake source audio").expect("write mp3");

    let response = execute(
        r#"{"paths":["voice.mp3"],"audioPreviewBytes":12}"#,
        dir.path(),
    );

    restore_env("TURA_READ_MEDIA_FFMPEG", previous_ffmpeg);
    restore_env("FFMPEG_PATH", previous_ffmpeg_path);

    assert!(
        response.success,
        "fake ffmpeg audio flow should succeed: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    let result = &response.output["media_results"][0];
    assert_eq!(result["success"], true);
    assert_eq!(result["path"], "voice.mp3");
    assert_eq!(result["media_type"], "audio");
    assert_eq!(result["visual_preview_count"], 0);
    assert_eq!(result["audio_preview_count"], 1);
    assert_eq!(result["file_attachment_count"], 0);
    let audio_preview = &result["audio_previews"][0];
    assert_eq!(audio_preview["audio_url"]["format"], "mp3");
    assert_eq!(audio_preview["compressed"], true);
    assert_eq!(
        audio_preview["max_size_bytes"], 100000,
        "policy clamps tiny audioPreviewBytes values before invoking ffmpeg"
    );
    assert_eq!(audio_preview["truncated_to_max_size"], false);
    assert!(audio_preview["audio_url"]["url"]
        .as_str()
        .is_some_and(|url| url.starts_with("data:audio/mpeg;base64,")));
    assert!(response
        .stdout
        .contains("voice.mp3: audio, 0 visual previews, 1 audio previews"));
}

#[test]
fn read_media_business_protocol_audio_preview_uses_local_ffmpeg_across_process_boundary() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let fake_ffmpeg = write_fake_ffmpeg(dir.path());
    let previous_ffmpeg = std::env::var_os("TURA_READ_MEDIA_FFMPEG");
    let previous_ffmpeg_path = std::env::var_os("FFMPEG_PATH");
    std::env::set_var("TURA_READ_MEDIA_FFMPEG", &fake_ffmpeg);
    std::env::remove_var("FFMPEG_PATH");
    std::fs::write(
        dir.path().join("protocol-audio.mp3"),
        b"protocol fake source audio",
    )
    .expect("write protocol audio");

    let response = run_protocol(json!({
        "kind": "execute",
        "payload": {
            "session_dir": dir.path().display().to_string(),
            "arguments": {
                "paths": ["protocol-audio.mp3"],
                "audioPreviewBytes": 12
            }
        }
    }));

    restore_env("TURA_READ_MEDIA_FFMPEG", previous_ffmpeg);
    restore_env("FFMPEG_PATH", previous_ffmpeg_path);

    assert_eq!(response["ok"], true);
    assert_eq!(response["success"], true);
    assert_eq!(response["exit_code"], 0);
    let result = &response["output"]["media_results"][0];
    assert_eq!(result["success"], true);
    assert_eq!(result["media_type"], "audio");
    assert_eq!(result["audio_preview_count"], 1);
    assert_eq!(result["audio_previews"][0]["max_size_bytes"], 100000);
    assert_eq!(
        result["audio_previews"][0]["note"],
        "Audio preview was compressed."
    );
    assert_eq!(result["audio_previews"][0]["compressed"], true);
    assert!(result["audio_previews"][0]["audio_url"]["url"]
        .as_str()
        .is_some_and(|url| url.starts_with("data:audio/mpeg;base64,")));
}

#[test]
fn read_media_business_flow_rejects_malformed_json_without_access_or_side_effects() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("safe.txt"), "safe").expect("write safe file");
    let malformed = "{\"paths\":[\"safe.txt\"],\"maxVisuals\":";

    let access = access(malformed, dir.path());
    assert!(access.read_paths.is_empty());
    assert!(access.write_paths.is_empty());
    assert!(!access.workspace_write);

    let response = execute(malformed, dir.path());

    assert!(!response.success);
    assert_eq!(response.exit_code, 1);
    assert!(response.stdout.is_empty());
    assert!(response
        .stderr
        .contains("invalid read_media command_line JSON"));
    assert_eq!(response.output["error"], response.stderr);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("safe.txt")).expect("safe file"),
        "safe",
        "malformed input must not mutate readable workspace files"
    );
}

#[test]
fn read_media_business_flow_extracts_video_frames_and_audio_preview_with_local_ffmpeg() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let fake_ffmpeg = write_fake_ffmpeg(dir.path());
    let previous_ffmpeg = std::env::var_os("TURA_READ_MEDIA_FFMPEG");
    let previous_ffmpeg_path = std::env::var_os("FFMPEG_PATH");
    std::env::set_var("TURA_READ_MEDIA_FFMPEG", &fake_ffmpeg);
    std::env::remove_var("FFMPEG_PATH");
    std::fs::write(dir.path().join("clip.mp4"), b"local fake video bytes").expect("write mp4");

    let response = execute(
        r#"{"paths":["clip.mp4"],"max_visuals":2,"max_side":64,"audio_preview_bytes":64}"#,
        dir.path(),
    );

    restore_env("TURA_READ_MEDIA_FFMPEG", previous_ffmpeg);
    restore_env("FFMPEG_PATH", previous_ffmpeg_path);

    assert!(
        response.success,
        "fake ffmpeg video flow should succeed: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    let result = &response.output["media_results"][0];
    assert_eq!(result["success"], true);
    assert_eq!(result["path"], "clip.mp4");
    assert_eq!(result["media_type"], "video");
    assert_eq!(result["visual_preview_count"], 1);
    assert_eq!(result["visual_contact_sheet"], true);
    assert_eq!(result["visual_previews"][0]["label"], "contact_sheet");
    assert_eq!(result["visual_previews"][0]["contact_sheet"], true);
    assert!(result["visual_previews"][0]["image_url"]["url"]
        .as_str()
        .is_some_and(|url| url.starts_with("data:image/jpeg;base64,")));
    assert_eq!(result["audio_preview_count"], 1);
    assert_eq!(result["file_attachment_count"], 0);
    let audio_preview = &result["audio_previews"][0];
    assert_eq!(audio_preview["type"], "audio_url");
    assert_eq!(audio_preview["audio_url"]["format"], "mp3");
    assert_eq!(audio_preview["max_size_bytes"], 100000);
    assert_eq!(audio_preview["truncated_to_max_size"], false);
    assert!(audio_preview["audio_url"]["url"]
        .as_str()
        .is_some_and(|url| url.starts_with("data:audio/mpeg;base64,")));
    assert!(response
        .stdout
        .contains("clip.mp4: video, 1 visual previews, 1 audio previews"));
}

#[test]
fn read_media_business_flow_reports_video_failure_from_local_ffmpeg_and_cv2_fallback() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let fake_ffmpeg = write_failing_ffmpeg(dir.path());
    let fake_python = write_failing_python(dir.path());
    let previous_ffmpeg = std::env::var_os("TURA_READ_MEDIA_FFMPEG");
    let previous_ffmpeg_path = std::env::var_os("FFMPEG_PATH");
    let previous_python = std::env::var_os("TURA_READ_MEDIA_PYTHON");
    std::env::set_var("TURA_READ_MEDIA_FFMPEG", &fake_ffmpeg);
    std::env::remove_var("FFMPEG_PATH");
    std::env::set_var("TURA_READ_MEDIA_PYTHON", &fake_python);
    std::fs::write(dir.path().join("broken.mp4"), b"broken fake video").expect("write broken mp4");

    let response = execute(
        r#"{"paths":["broken.mp4"],"max_visuals":2,"audio_preview_bytes":64}"#,
        dir.path(),
    );

    restore_env("TURA_READ_MEDIA_FFMPEG", previous_ffmpeg);
    restore_env("FFMPEG_PATH", previous_ffmpeg_path);
    restore_env("TURA_READ_MEDIA_PYTHON", previous_python);

    assert!(
        response.success,
        "per-item video errors should not fail the command envelope: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    let result = &response.output["media_results"][0];
    assert_eq!(result["path"], "broken.mp4");
    assert_eq!(result["success"], false);
    let error = result["error"].as_str().expect("video error text");
    assert!(
        error.contains("ffmpeg failed"),
        "unexpected video error: {error}"
    );
    assert!(
        error.contains("fake python cv2 failure")
            || error.contains("python cv2 fallback failed to start"),
        "unexpected video error: {error}"
    );
    assert!(response.stdout.contains("broken.mp4: failed"));
}

#[test]
fn read_media_business_flow_mixed_batch_workers_keep_order_and_isolate_failures() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let fake_ffmpeg = write_fake_ffmpeg(dir.path());
    let previous_ffmpeg = std::env::var_os("TURA_READ_MEDIA_FFMPEG");
    let previous_ffmpeg_path = std::env::var_os("FFMPEG_PATH");
    std::env::set_var("TURA_READ_MEDIA_FFMPEG", &fake_ffmpeg);
    std::env::remove_var("FFMPEG_PATH");

    std::fs::write(dir.path().join("notes.txt"), "mixed batch notes").expect("write notes");
    std::fs::write(dir.path().join("image.png"), png_bytes()).expect("write png");
    std::fs::write(dir.path().join("clip.mp4"), b"local fake video bytes").expect("write mp4");
    std::fs::write(dir.path().join("sound.mp3"), b"local fake audio bytes").expect("write mp3");
    std::fs::write(dir.path().join("blob.bin"), [0_u8, 1, 2, 3]).expect("write bin");

    let response = execute(
        r#"{
            "paths": ["notes.txt", "image.png", "clip.mp4", "sound.mp3", "blob.bin", "missing.mov"],
            "max_visuals": 1,
            "max_side": 64,
            "include_text": true,
            "document_attachment_bytes": 100000
        }"#,
        dir.path(),
    );

    restore_env("TURA_READ_MEDIA_FFMPEG", previous_ffmpeg);
    restore_env("FFMPEG_PATH", previous_ffmpeg_path);

    assert!(
        response.success,
        "mixed batch should keep envelope success while recording per-item failures: {}",
        response.stderr
    );
    assert_eq!(response.exit_code, 0);
    let results = response.output["media_results"]
        .as_array()
        .expect("mixed media results");
    assert_eq!(results.len(), 6);
    let paths = results
        .iter()
        .map(|item| normalize_path(item["path"].as_str().expect("result path")))
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![
            "notes.txt",
            "image.png",
            "clip.mp4",
            "sound.mp3",
            "blob.bin",
            "missing.mov"
        ],
        "worker results must be sorted back into request order"
    );
    assert_eq!(results[0]["success"], true);
    assert_eq!(results[0]["media_type"], "document");
    assert_eq!(results[1]["success"], true);
    assert_eq!(results[1]["media_type"], "image");
    assert_eq!(results[2]["success"], true);
    assert_eq!(results[2]["media_type"], "video");
    assert_eq!(results[3]["success"], true);
    assert_eq!(results[3]["media_type"], "audio");
    assert_eq!(results[4]["success"], true);
    assert_eq!(results[4]["media_type"], "document");
    assert_eq!(results[5]["success"], false);
    assert!(results[5]["error"]
        .as_str()
        .is_some_and(|error| error.contains("media path does not exist")));
    assert_eq!(response.output["visual_contact_sheet"], true);
    assert_eq!(response.output["visual_preview_count"], 1);
    assert!(response.output["visual_previews"][0]["image_url"]["url"]
        .as_str()
        .is_some_and(|url| url.starts_with("data:image/jpeg;base64,")));
    assert!(response.stdout.contains("notes.txt: document"));
    assert!(response.stdout.contains("image.png: image"));
    assert!(response.stdout.contains("clip.mp4: video"));
    assert!(response.stdout.contains("sound.mp3: audio"));
    assert!(response.stdout.contains("blob.bin: document"));
    assert!(response.stdout.contains("missing.mov: failed"));
}

fn run_protocol(request: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tura-command-read-media"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn read_media binary");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(request.to_string().as_bytes())
        .expect("write request");
    let output = child.wait_with_output().expect("protocol output");
    assert!(
        output.status.success(),
        "read_media protocol process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("protocol json response")
}

fn png_bytes() -> Vec<u8> {
    let image = image::DynamicImage::ImageRgba8(image::ImageBuffer::from_fn(4, 4, |x, y| {
        if (x + y) % 2 == 0 {
            image::Rgba([220, 20, 80, 255])
        } else {
            image::Rgba([20, 120, 220, 255])
        }
    }));
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("encode png");
    bytes
}

fn jpeg_bytes() -> Vec<u8> {
    let image = image::DynamicImage::ImageRgb8(image::ImageBuffer::from_fn(4, 4, |x, y| {
        if (x + y) % 2 == 0 {
            image::Rgb([230, 30, 40])
        } else {
            image::Rgb([40, 130, 230])
        }
    }));
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
        .expect("encode jpeg");
    bytes
}

fn write_fake_ffmpeg(dir: &std::path::Path) -> std::path::PathBuf {
    let frame = dir.join("fake-frame.jpg");
    std::fs::write(&frame, jpeg_bytes()).expect("write fake frame jpeg");
    #[cfg(windows)]
    {
        let script = dir.join("fake-ffmpeg.cmd");
        let ps1 = dir.join("fake-ffmpeg.ps1");
        std::fs::write(
            &ps1,
            r#"$argsText = $args -join ' '
if ($argsText -match '-f mp3') {
  $out = $args[$args.Count - 1]
  New-Item -ItemType Directory -Force -Path (Split-Path -LiteralPath $out) | Out-Null
  [System.IO.File]::WriteAllText($out, 'fake mp3 bytes', [System.Text.Encoding]::ASCII)
  exit 0
}
$pattern = $args[$args.Count - 1]
New-Item -ItemType Directory -Force -Path (Split-Path -LiteralPath $pattern) | Out-Null
$frame = Join-Path (Split-Path -LiteralPath $pattern) 'frame_001.jpg'
Copy-Item -LiteralPath "$env:TURA_READ_MEDIA_FAKE_FRAME" -Destination $frame -Force
$frame = Join-Path (Split-Path -LiteralPath $pattern) 'frame_002.jpg'
Copy-Item -LiteralPath "$env:TURA_READ_MEDIA_FAKE_FRAME" -Destination $frame -Force
exit 0
"#,
        )
        .expect("write fake ffmpeg ps1");
        std::fs::write(
            &script,
            r#"@echo off
set TURA_READ_MEDIA_FAKE_FRAME=%~dp0fake-frame.jpg
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0fake-ffmpeg.ps1" %*
exit /b %ERRORLEVEL%
"#,
        )
        .expect("write fake ffmpeg cmd");
        script
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("fake-ffmpeg.sh");
        std::fs::write(
            &script,
            r#"#!/usr/bin/env sh
set -eu
last=""
for arg in "$@"; do
  last="$arg"
done
case " $* " in
  *" -f mp3 "*) mkdir -p "$(dirname "$last")"; printf 'fake mp3 bytes' > "$last"; exit 0 ;;
esac
mkdir -p "$(dirname "$last")"
cp "$(dirname "$0")/fake-frame.jpg" "$(dirname "$last")/frame_001.jpg"
cp "$(dirname "$0")/fake-frame.jpg" "$(dirname "$last")/frame_002.jpg"
"#,
        )
        .expect("write fake ffmpeg sh");
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script)
            .expect("fake ffmpeg metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).expect("chmod fake ffmpeg");
        script
    }
}

fn write_failing_ffmpeg(dir: &std::path::Path) -> std::path::PathBuf {
    #[cfg(windows)]
    {
        let script = dir.join("failing-ffmpeg.cmd");
        std::fs::write(&script, "@echo off\r\nexit /b 7\r\n").expect("write failing ffmpeg cmd");
        script
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("failing-ffmpeg.sh");
        std::fs::write(&script, "#!/usr/bin/env sh\nexit 7\n").expect("write failing ffmpeg sh");
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script)
            .expect("failing ffmpeg metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).expect("chmod failing ffmpeg");
        script
    }
}

fn write_failing_python(dir: &std::path::Path) -> std::path::PathBuf {
    #[cfg(windows)]
    {
        let script = dir.join("failing-python.cmd");
        std::fs::write(
            &script,
            "@echo off\r\necho fake python cv2 failure 1>&2\r\nexit /b 9\r\n",
        )
        .expect("write failing python cmd");
        script
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("failing-python.sh");
        std::fs::write(
            &script,
            "#!/usr/bin/env sh\necho fake python cv2 failure >&2\nexit 9\n",
        )
        .expect("write failing python sh");
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script)
            .expect("failing python metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).expect("chmod failing python");
        script
    }
}

fn minimal_pdf_bytes() -> Vec<u8> {
    br#"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R >>
endobj
4 0 obj
<< /Length 68 >>
stream
BT /F1 12 Tf 10 100 Td (Local PDF business fixture) Tj ET
endstream
endobj
xref
0 5
0000000000 65535 f
0000000009 00000 n
0000000058 00000 n
0000000115 00000 n
0000000204 00000 n
trailer
<< /Root 1 0 R /Size 5 >>
startxref
322
%%EOF
"#
    .to_vec()
}

fn result_by_path<'a>(results: &'a [Value], path: &str) -> &'a Value {
    results
        .iter()
        .find(|item| {
            item["path"]
                .as_str()
                .is_some_and(|actual| normalize_path(actual) == normalize_path(path))
        })
        .unwrap_or_else(|| panic!("missing result for {path}: {results:?}"))
}

fn assert_result(results: &[Value], path: &str, success: bool) {
    let result = result_by_path(results, path);
    assert_eq!(result["success"], success);
    assert_eq!(result["media_type"], "document");
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn restore_env(key: &str, previous: Option<std::ffi::OsString>) {
    match previous {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
}
