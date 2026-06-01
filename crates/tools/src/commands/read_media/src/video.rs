use super::paths::{find_on_path, temp_work_dir};
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
    let status = std::process::Command::new(ffmpeg)
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
        .arg(&pattern)
        .status()
        .map_err(|err| {
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
    let output = std::process::Command::new(ffmpeg)
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
        .arg(&output_path)
        .output()
        .map_err(|err| {
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
    let output = std::process::Command::new("python")
        .arg("-c")
        .arg(script)
        .arg(path)
        .arg(args.max_visuals.to_string())
        .arg(args.max_side.to_string())
        .output()
        .map_err(|err| {
            format!("ffmpeg not found and python cv2 fallback failed to start: {err}")
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "video frame extraction unavailable: install ffmpeg or python cv2; {stderr}"
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
    if let Ok(path) = std::env::var("FFMPEG_PATH") {
        if !path.trim().is_empty() && Path::new(&path).exists() {
            return Some(path);
        }
    }
    find_on_path("ffmpeg")
        .map(|path| path.display().to_string())
        .or_else(resolve_imageio_ffmpeg)
}

fn resolve_imageio_ffmpeg() -> Option<String> {
    let output = std::process::Command::new("python")
        .arg("-c")
        .arg("import imageio_ffmpeg; print(imageio_ffmpeg.get_ffmpeg_exe())")
        .output()
        .ok()?;
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
