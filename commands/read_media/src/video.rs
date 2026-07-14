use super::paths::{command_configured_python, command_local_python, find_on_path, temp_work_dir};
use super::types::{MediaContent, ReadMediaArgs};
use base64::{engine::general_purpose, Engine as _};
use serde_json::{json, Value};
use std::path::Path;

pub(super) fn process_video(path: &Path, args: &ReadMediaArgs) -> Result<MediaContent, String> {
    let Some(ffmpeg) = resolve_ffmpeg() else {
        return process_video_with_python_cv2(path, args).map(|visual_previews| MediaContent {
            text: String::new(),
            visual_previews,
            audio_previews: Vec::new(),
            file_attachments: Vec::new(),
        });
    };
    let temp_dir = temp_work_dir("tura-read-media");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|err| format!("failed to create temp frame dir: {err}"))?;
    let pattern = temp_dir.join("frame_%03d.jpg");
    let fps_filter = format!("fps=1,scale='min({0},iw)':-2", args.max_side);
    let mut command = std::process::Command::new(ffmpeg);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(path)
        .arg("-vf")
        .arg(fps_filter)
        .arg("-frames:v")
        .arg(args.max_visuals.to_string())
        .arg("-q:v")
        .arg("4")
        .arg("-y")
        .arg(&pattern);
    tura_path::process_hardening::hide_child_console_window(&mut command);
    let status = command.status().map_err(|err| {
        let _ = std::fs::remove_dir_all(&temp_dir);
        format!("failed to run ffmpeg: {err}")
    })?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return process_video_with_python_cv2(path, args)
            .map(|visual_previews| MediaContent {
                text: String::new(),
                visual_previews,
                audio_previews: Vec::new(),
                file_attachments: Vec::new(),
            })
            .map_err(|cv_err| format!("ffmpeg failed with status {status}; {cv_err}"));
    }
    let mut previews = Vec::new();
    let mut entries = std::fs::read_dir(&temp_dir)
        .map_err(|err| format!("failed to read temp frames: {err}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for (index, frame) in entries.into_iter().take(args.max_visuals).enumerate() {
        let bytes = std::fs::read(&frame).map_err(|err| format!("failed to read frame: {err}"))?;
        previews.push(json!({
            "type": "image_url",
            "label": format!("T{}MS", index * 1000),
            "image_url": { "url": format!("data:image/jpeg;base64,{}", general_purpose::STANDARD.encode(bytes)) }
        }));
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
    let audio_previews = process_audio(path, args).unwrap_or_default();
    Ok(MediaContent {
        text: String::new(),
        visual_previews: previews,
        audio_previews,
        file_attachments: Vec::new(),
    })
}

pub(super) fn process_audio(path: &Path, args: &ReadMediaArgs) -> Result<Vec<Value>, String> {
    let Some(ffmpeg) = resolve_ffmpeg() else {
        return Err("audio extraction unavailable: install ffmpeg".to_string());
    };
    let temp_dir = temp_work_dir("tura-read-media-audio");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|err| format!("failed to create temp audio dir: {err}"))?;
    let output_path = temp_dir.join("preview.mp3");
    let mut command = std::process::Command::new(ffmpeg);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(path)
        .arg("-vn")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg("-b:a")
        .arg("24k")
        .arg("-fs")
        .arg(args.audio_preview_bytes.to_string())
        .arg("-f")
        .arg("mp3")
        .arg("-y")
        .arg(&output_path);
    tura_path::process_hardening::hide_child_console_window(&mut command);
    let output = command.output().map_err(|err| {
        let _ = std::fs::remove_dir_all(&temp_dir);
        format!("failed to run ffmpeg for audio extraction: {err}")
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(format!("audio extraction failed: {stderr}"));
    }
    let bytes = std::fs::read(&output_path).map_err(|err| {
        let _ = std::fs::remove_dir_all(&temp_dir);
        format!("failed to read compressed audio preview: {err}")
    })?;
    let _ = std::fs::remove_dir_all(&temp_dir);
    if bytes.is_empty() {
        return Err("audio extraction produced no data".to_string());
    }
    let compressed_to_limit = bytes.len() as u64 >= args.audio_preview_bytes;
    Ok(vec![json!({
        "type": "audio_url",
        "audio_url": {
            "url": format!("data:audio/mpeg;base64,{}", general_purpose::STANDARD.encode(bytes)),
            "format": "mp3"
        },
        "compressed": true,
        "max_size_bytes": args.audio_preview_bytes,
        "truncated_to_max_size": compressed_to_limit,
        "note": if compressed_to_limit {
            "Audio preview was compressed and capped to the configured byte limit."
        } else {
            "Audio preview was compressed."
        }
    })])
}

pub(super) fn process_video_with_python_cv2(
    path: &Path,
    args: &ReadMediaArgs,
) -> Result<Vec<Value>, String> {
    let script = r#"
import base64, json, sys
import cv2

path = sys.argv[1]
max_frames = int(sys.argv[2])
max_side = int(sys.argv[3])
cap = cv2.VideoCapture(path)
if not cap.isOpened():
    raise SystemExit("failed to open video")
total = int(cap.get(cv2.CAP_PROP_FRAME_COUNT) or 0)
step = max(1, total // max(1, max_frames)) if total > 0 else 1
frames = []
index = 0
frame_no = 0
while len(frames) < max_frames:
    ok, frame = cap.read()
    if not ok:
        break
    if index % step == 0:
        h, w = frame.shape[:2]
        longest = max(w, h)
        if longest > max_side:
            scale = max_side / float(longest)
            frame = cv2.resize(frame, (max(1, int(w * scale)), max(1, int(h * scale))))
        ok, encoded = cv2.imencode(".jpg", frame, [int(cv2.IMWRITE_JPEG_QUALITY), 80])
        if ok:
            msec = int(cap.get(cv2.CAP_PROP_POS_MSEC) or (frame_no * 1000))
            frames.append({"label": f"T{msec}MS", "data": base64.b64encode(encoded.tobytes()).decode("ascii")})
            frame_no += 1
    index += 1
cap.release()
print(json.dumps(frames))
"#;
    let python = command_local_python("TURA_READ_MEDIA_PYTHON")
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "python".to_string());
    let mut command = std::process::Command::new(&python);
    command
        .arg("-c")
        .arg(script)
        .arg(path)
        .arg(args.max_visuals.to_string())
        .arg(args.max_side.to_string());
    tura_path::process_hardening::hide_child_console_window(&mut command);
    let output = command.output().map_err(|err| {
        format!("ffmpeg not found and python cv2 fallback failed to start: {err}")
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "video frame extraction unavailable: run commands/read_media/install.* or install ffmpeg; {stderr}"
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let frames: Vec<Value> = serde_json::from_str(stdout.trim())
        .map_err(|err| format!("python cv2 fallback returned invalid JSON: {err}"))?;
    if frames.is_empty() {
        return Err("video frame extraction produced no frames".to_string());
    }
    Ok(frames
        .into_iter()
        .map(|frame| {
            let label = frame.get("label").and_then(Value::as_str).unwrap_or("F");
            let data = frame
                .get("data")
                .and_then(Value::as_str)
                .unwrap_or_default();
            json!({
                "type": "image_url",
                "label": label,
                "image_url": { "url": format!("data:image/jpeg;base64,{data}") }
            })
        })
        .collect())
}

pub(super) fn resolve_ffmpeg() -> Option<String> {
    for env_name in ["TURA_READ_MEDIA_FFMPEG", "FFMPEG_PATH"] {
        if let Ok(path) = std::env::var(env_name) {
            if !path.trim().is_empty() && Path::new(&path).exists() {
                return Some(path);
            }
        }
    }
    resolve_imageio_ffmpeg()
        .or_else(|| find_on_path("ffmpeg").map(|path| path.display().to_string()))
}

fn resolve_imageio_ffmpeg() -> Option<String> {
    let python = command_configured_python("TURA_READ_MEDIA_PYTHON")
        .map(|path| path.display().to_string())?;
    let mut command = std::process::Command::new(python);
    command
        .arg("-c")
        .arg("import imageio_ffmpeg; print(imageio_ffmpeg.get_ffmpeg_exe())");
    tura_path::process_hardening::hide_child_console_window(&mut command);
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !path.is_empty() && Path::new(&path).exists() {
        Some(path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{process_audio, resolve_ffmpeg};
    use crate::types::ReadMediaArgs;
    use std::ffi::OsString;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn args(audio_preview_bytes: u64) -> ReadMediaArgs {
        ReadMediaArgs {
            paths: vec!["clip.mp4".to_string()],
            include_text: true,
            max_text_chars: 40_000,
            max_visuals: 3,
            max_side: 320,
            max_files: 10,
            pdf_max_pages: 2,
            document_attachment_bytes: 1_000_000,
            audio_preview_bytes,
        }
    }

    fn restore_env(key: &str, value: Option<OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

    fn fake_ffmpeg_script(exit_success: bool) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = if cfg!(windows) {
            dir.path().join("ffmpeg.cmd")
        } else {
            dir.path().join("ffmpeg")
        };
        if cfg!(windows) {
            let body = if exit_success {
                "@echo off\r\nexit /b 0\r\n"
            } else {
                "@echo off\r\necho fake ffmpeg failure 1>&2\r\nexit /b 7\r\n"
            };
            std::fs::write(&path, body).expect("write fake ffmpeg");
        } else {
            let body = if exit_success {
                "#!/bin/sh\nexit 0\n"
            } else {
                "#!/bin/sh\necho fake ffmpeg failure >&2\nexit 7\n"
            };
            std::fs::write(&path, body).expect("write fake ffmpeg");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
                permissions.set_mode(0o755);
                std::fs::set_permissions(&path, permissions).expect("chmod fake ffmpeg");
            }
        }
        (dir, path)
    }

    #[test]
    fn resolve_ffmpeg_prefers_existing_explicit_env_and_ignores_missing_env() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let previous_ffmpeg = std::env::var_os("TURA_READ_MEDIA_FFMPEG");
        let previous_ffmpeg_path = std::env::var_os("FFMPEG_PATH");
        let previous_python = std::env::var_os("TURA_READ_MEDIA_PYTHON");
        let previous_command_python = std::env::var_os("TURA_COMMAND_PYTHON");

        std::env::remove_var("FFMPEG_PATH");
        std::env::set_var(
            "TURA_READ_MEDIA_PYTHON",
            "definitely-missing-python-for-read-media",
        );
        std::env::set_var(
            "TURA_COMMAND_PYTHON",
            "definitely-missing-python-for-read-media",
        );
        std::env::set_var(
            "TURA_READ_MEDIA_FFMPEG",
            "definitely-missing-ffmpeg-for-read-media",
        );
        let missing = resolve_ffmpeg();
        if let Some(path) = missing.as_deref() {
            assert!(
                !path.contains("definitely-missing-ffmpeg-for-read-media"),
                "missing explicit env path must be ignored"
            );
        }

        let (_dir, fake) = fake_ffmpeg_script(true);
        std::env::set_var("TURA_READ_MEDIA_FFMPEG", &fake);
        assert_eq!(
            resolve_ffmpeg().as_deref(),
            Some(fake.to_string_lossy().as_ref())
        );

        restore_env("TURA_READ_MEDIA_FFMPEG", previous_ffmpeg);
        restore_env("FFMPEG_PATH", previous_ffmpeg_path);
        restore_env("TURA_READ_MEDIA_PYTHON", previous_python);
        restore_env("TURA_COMMAND_PYTHON", previous_command_python);
    }

    #[test]
    fn process_audio_reports_ffmpeg_stderr_on_failure() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let previous_ffmpeg = std::env::var_os("TURA_READ_MEDIA_FFMPEG");
        let previous_ffmpeg_path = std::env::var_os("FFMPEG_PATH");
        let (_dir, fake) = fake_ffmpeg_script(false);
        let media = tempfile::NamedTempFile::new().expect("media file");
        std::env::set_var("TURA_READ_MEDIA_FFMPEG", &fake);
        std::env::remove_var("FFMPEG_PATH");

        let error = process_audio(media.path(), &args(123_456)).expect_err("ffmpeg should fail");

        assert!(error.contains("audio extraction failed"));
        assert!(error.contains("fake ffmpeg failure"));
        restore_env("TURA_READ_MEDIA_FFMPEG", previous_ffmpeg);
        restore_env("FFMPEG_PATH", previous_ffmpeg_path);
    }
}
