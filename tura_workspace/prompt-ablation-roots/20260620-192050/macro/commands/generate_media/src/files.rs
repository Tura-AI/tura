use super::types::{Dimensions, GenerateMediaArgs, ImageBytes, DEFAULT_SIZE};
use base64::{engine::general_purpose, Engine as _};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(super) fn output_dir(args: &GenerateMediaArgs, session_dir: &Path) -> PathBuf {
    let path = PathBuf::from(&args.output_dir);
    if path.is_absolute() {
        path
    } else {
        session_dir.join(path)
    }
}

pub(super) fn workspace_relative_path(path: &str, session_dir: &Path) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    let resolved = if path.is_absolute() {
        path
    } else {
        session_dir.join(path)
    };
    resolved
        .strip_prefix(session_dir)
        .ok()
        .map(Path::to_path_buf)
}

pub(super) fn relative_or_display(path: &Path, session_dir: &Path) -> String {
    path.strip_prefix(session_dir)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

pub(super) fn resolve_reference(path: &str, session_dir: &Path) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        session_dir.join(candidate)
    }
}

pub(super) fn reference_data_url(path: &str, session_dir: &Path) -> Result<String, String> {
    if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("data:") {
        return Ok(path.to_string());
    }
    let resolved = resolve_reference(path, session_dir);
    let bytes = std::fs::read(&resolved).map_err(|err| {
        format!(
            "failed to read reference image {}: {err}",
            resolved.display()
        )
    })?;
    Ok(format!(
        "data:{};base64,{}",
        mime_type_for_path(&resolved),
        general_purpose::STANDARD.encode(bytes)
    ))
}

pub(super) fn reference_part_bytes(
    path: &str,
    session_dir: &Path,
) -> Result<(String, Vec<u8>), String> {
    let resolved = resolve_reference(path, session_dir);
    let bytes = std::fs::read(&resolved).map_err(|err| {
        format!(
            "failed to read reference image {}: {err}",
            resolved.display()
        )
    })?;
    let name = resolved
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("reference.png")
        .to_string();
    Ok((name, bytes))
}

pub(super) fn write_images(
    images: &[ImageBytes],
    args: &GenerateMediaArgs,
    session_dir: &Path,
    provider: &str,
) -> Result<Vec<Value>, String> {
    let dir = output_dir(args, session_dir);
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("failed to create output dir {}: {err}", dir.display()))?;
    let mut out = Vec::new();
    for (index, image) in images.iter().enumerate() {
        let extension = extension_for_mime(&image.mime_type).unwrap_or(&args.output_format);
        let path = write_unique_download(
            &dir,
            &format!("generate-media-{provider}-{}", index + 1),
            extension,
            &image.bytes,
        )?;
        let metadata = std::fs::metadata(&path).ok();
        out.push(json!({
            "path": relative_or_display(&path, session_dir),
            "absolute_path": path.display().to_string(),
            "name": path.file_name().and_then(|value| value.to_str()).unwrap_or_default(),
            "file_type": "image",
            "content_type": image.mime_type,
            "size": metadata.map(|m| m.len()).unwrap_or(0),
            "source_url": image.source_url,
        }));
    }
    Ok(out)
}

pub(super) fn write_unique_download(
    output_dir: &Path,
    base_name: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    for copy in 0..1000 {
        let suffix = if copy == 0 {
            String::new()
        } else {
            format!("-{copy}")
        };
        let path = output_dir.join(format!("{base_name}{suffix}.{extension}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(bytes)
                    .map_err(|err| format!("failed to write generated image: {err}"))?;
                return Ok(path);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(format!("failed to write generated image: {err}")),
        }
    }
    Err(format!(
        "failed to choose unique image name for {base_name}.{extension}"
    ))
}

pub(super) fn dimensions(args: &GenerateMediaArgs) -> Result<Dimensions, String> {
    if let (Some(width), Some(height)) = (args.width, args.height) {
        return validate_dimensions(width, height);
    }
    if let Some(size) = args.size.as_deref().filter(|value| *value != "auto") {
        let (width, height) = parse_size(size)?;
        return validate_dimensions(width, height);
    }
    if let Some(ratio) = args.aspect_ratio.as_deref() {
        let (w, h) = parse_ratio(ratio)?;
        let long = 1024u32;
        let (width, height) = if w >= h {
            (long, ((long as f64) * (h as f64 / w as f64)).round() as u32)
        } else {
            (((long as f64) * (w as f64 / h as f64)).round() as u32, long)
        };
        return validate_dimensions(round_to_16(width), round_to_16(height));
    }
    let (width, height) = parse_size(DEFAULT_SIZE)?;
    validate_dimensions(width, height)
}

pub(super) fn openai_size(args: &GenerateMediaArgs) -> Result<String, String> {
    if args.size.as_deref() == Some("auto") {
        return Ok("auto".to_string());
    }
    let dims = dimensions(args)?;
    validate_openai_gpt_image_2_size(dims)?;
    Ok(format!("{}x{}", dims.width, dims.height))
}

pub(super) fn provider_dimensions(
    args: &GenerateMediaArgs,
    max_edge: u32,
) -> Result<Dimensions, String> {
    let dims = dimensions(args)?;
    if dims.width <= max_edge && dims.height <= max_edge {
        return Ok(dims);
    }
    let scale = max_edge as f64 / dims.width.max(dims.height) as f64;
    validate_dimensions(
        round_to_16((dims.width as f64 * scale).round() as u32),
        round_to_16((dims.height as f64 * scale).round() as u32),
    )
}

pub(super) fn aspect_ratio(args: &GenerateMediaArgs) -> Result<String, String> {
    if let Some(value) = args.aspect_ratio.as_deref() {
        return Ok(value.to_string());
    }
    let dims = dimensions(args)?;
    let gcd = gcd(dims.width, dims.height);
    Ok(format!("{}:{}", dims.width / gcd, dims.height / gcd))
}

pub(super) fn image_size_label(args: &GenerateMediaArgs) -> Result<String, String> {
    let dims = dimensions(args)?;
    if dims.width.max(dims.height) >= 1800 {
        Ok("2K".to_string())
    } else {
        Ok("1K".to_string())
    }
}

fn validate_dimensions(width: u32, height: u32) -> Result<Dimensions, String> {
    if width < 64 || height < 64 {
        return Err("image dimensions must be at least 64x64".to_string());
    }
    Ok(Dimensions { width, height })
}

fn validate_openai_gpt_image_2_size(dims: Dimensions) -> Result<(), String> {
    let max_edge = dims.width.max(dims.height);
    let min_edge = dims.width.min(dims.height);
    let pixels = dims.width as u64 * dims.height as u64;
    if max_edge > 3840 {
        return Err("gpt-image-2 maximum edge is 3840px".to_string());
    }
    if !dims.width.is_multiple_of(16) || !dims.height.is_multiple_of(16) {
        return Err("gpt-image-2 width and height must be multiples of 16".to_string());
    }
    if max_edge as f64 / min_edge as f64 > 3.0 {
        return Err("gpt-image-2 aspect ratio must be between 1:3 and 3:1".to_string());
    }
    if !(655_360..=8_294_400).contains(&pixels) {
        return Err("gpt-image-2 total pixels must be between 655360 and 8294400".to_string());
    }
    Ok(())
}

fn parse_size(value: &str) -> Result<(u32, u32), String> {
    let normalized = value.trim().to_ascii_lowercase();
    let Some((width, height)) = normalized.split_once('x') else {
        return Err(format!("invalid image size: {value}"));
    };
    let width = width
        .parse::<u32>()
        .map_err(|_| format!("invalid image width: {value}"))?;
    let height = height
        .parse::<u32>()
        .map_err(|_| format!("invalid image height: {value}"))?;
    Ok((width, height))
}

fn parse_ratio(value: &str) -> Result<(u32, u32), String> {
    let Some((width, height)) = value.trim().split_once(':') else {
        return Err(format!("invalid aspect ratio: {value}"));
    };
    let width = width
        .parse::<u32>()
        .map_err(|_| format!("invalid aspect ratio: {value}"))?;
    let height = height
        .parse::<u32>()
        .map_err(|_| format!("invalid aspect ratio: {value}"))?;
    if width == 0 || height == 0 {
        return Err(format!("invalid aspect ratio: {value}"));
    }
    Ok((width, height))
}

fn round_to_16(value: u32) -> u32 {
    ((value.max(64) + 8) / 16).max(4) * 16
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a.max(1)
}

pub(super) fn mime_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

pub(super) fn mime_type_for_format(format: &str) -> String {
    match format {
        "jpeg" | "jpg" => "image/jpeg".to_string(),
        "webp" => "image/webp".to_string(),
        _ => "image/png".to_string(),
    }
}

pub(super) fn extension_for_mime(mime_type: &str) -> Option<&'static str> {
    match mime_type.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/png" => Some("png"),
        _ => None,
    }
}
