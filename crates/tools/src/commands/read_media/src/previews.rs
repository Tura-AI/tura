use base64::{engine::general_purpose, Engine as _};
use image::{
    codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, GenericImageView, Rgb, RgbImage,
};
use serde_json::{json, Value};

pub(super) fn compact_visual_previews(output: &mut Value) -> Result<(), String> {
    let Some(results) = output
        .get_mut("media_results")
        .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };
    for result in results.iter_mut() {
        compact_result_visual_previews(result)?;
    }
    let mut aggregate = Vec::new();
    for result in results.iter() {
        let path = result
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("media");
        let previews = result
            .get("visual_previews")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (index, preview) in previews.into_iter().enumerate() {
            if let Some(url) = preview_data_url(&preview) {
                let label = preview
                    .get("label")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| format!("#{:02}", index + 1));
                aggregate.push(SheetItem {
                    label: format!("#{:02} {}", aggregate.len() + 1, label),
                    detail: path.to_string(),
                    data_url: url.to_string(),
                });
            }
        }
    }
    if aggregate.len() <= 1 {
        return Ok(());
    }
    let sheets = contact_sheet_previews(&aggregate)?;
    for result in results.iter_mut() {
        if result
            .get("visual_previews")
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false)
        {
            result["visual_previews"] = json!([]);
            result["visual_preview_count"] = json!(0);
            result["visual_previews_compacted_into"] = json!("top_level_contact_sheet");
        }
    }
    output["visual_preview_count"] = json!(sheets.len());
    output["visual_previews"] = json!(sheets);
    output["visual_contact_sheet"] = json!(true);
    Ok(())
}

fn compact_result_visual_previews(result: &mut Value) -> Result<(), String> {
    let Some(previews) = result
        .get("visual_previews")
        .and_then(Value::as_array)
        .cloned()
    else {
        return Ok(());
    };
    if previews.len() <= 1 {
        return Ok(());
    }
    let path = result
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("media");
    let items = previews
        .iter()
        .enumerate()
        .filter_map(|(index, preview)| {
            let data_url = preview_data_url(preview)?;
            let label = preview
                .get("label")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("#{:02}", index + 1));
            Some(SheetItem {
                label: format!("#{:02} {}", index + 1, label),
                detail: path.to_string(),
                data_url: data_url.to_string(),
            })
        })
        .collect::<Vec<_>>();
    if items.len() <= 1 {
        return Ok(());
    }
    let sheets = contact_sheet_previews(&items)?;
    result["visual_previews"] = json!(sheets);
    result["visual_preview_count"] = json!(result["visual_previews"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0));
    result["visual_contact_sheet"] = json!(true);
    Ok(())
}

struct SheetItem {
    label: String,
    detail: String,
    data_url: String,
}

fn contact_sheet_previews(items: &[SheetItem]) -> Result<Vec<Value>, String> {
    let mut sheets = Vec::new();
    for chunk in items.chunks(12) {
        let sheet = render_contact_sheet(chunk)?;
        let encoded = encode_preview_jpeg(sheet, 1024, 76)?;
        sheets.push(json!({
            "type": "image_url",
            "label": "contact_sheet",
            "contact_sheet": true,
            "item_count": chunk.len(),
            "items": chunk.iter().map(|item| json!({
                "label": item.label,
                "path": item.detail,
            })).collect::<Vec<_>>(),
            "image_url": { "url": format!("data:image/jpeg;base64,{}", encoded) }
        }));
    }
    Ok(sheets)
}

fn render_contact_sheet(items: &[SheetItem]) -> Result<DynamicImage, String> {
    let cols = if items.len() <= 4 { 2 } else { 4 };
    let tile_w = 240u32;
    let tile_h = 190u32;
    let rows = items.len().div_ceil(cols);
    let mut sheet = RgbImage::from_pixel(
        tile_w * cols as u32,
        tile_h * rows as u32,
        Rgb([245, 245, 245]),
    );
    for (index, item) in items.iter().enumerate() {
        let image = image_from_data_url(&item.data_url)?;
        let thumb = resize_dynamic_image(image, 150);
        let x = (index % cols) as u32 * tile_w;
        let y = (index / cols) as u32 * tile_h;
        paste_rgb(
            &mut sheet,
            &thumb.to_rgb8(),
            x + (tile_w - thumb.width()) / 2,
            y + 8,
        );
        draw_text(
            &mut sheet,
            x + 8,
            y + 164,
            &item.label.to_ascii_uppercase(),
            Rgb([0, 0, 0]),
        );
    }
    Ok(DynamicImage::ImageRgb8(sheet))
}

fn preview_data_url(value: &Value) -> Option<&str> {
    value
        .get("image_url")
        .and_then(|image| image.get("url"))
        .and_then(Value::as_str)
}

fn image_from_data_url(data_url: &str) -> Result<DynamicImage, String> {
    let (_, encoded) = data_url
        .split_once(',')
        .ok_or_else(|| "invalid image data URL".to_string())?;
    let bytes = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|err| format!("invalid base64 image preview: {err}"))?;
    image::load_from_memory(&bytes).map_err(|err| format!("failed to decode preview image: {err}"))
}

fn paste_rgb(canvas: &mut RgbImage, image: &RgbImage, x: u32, y: u32) {
    for yy in 0..image.height() {
        for xx in 0..image.width() {
            let tx = x + xx;
            let ty = y + yy;
            if tx < canvas.width() && ty < canvas.height() {
                canvas.put_pixel(tx, ty, *image.get_pixel(xx, yy));
            }
        }
    }
}

pub(super) fn draw_text(canvas: &mut RgbImage, x: u32, y: u32, text: &str, color: Rgb<u8>) {
    let mut cursor = x;
    for ch in text.chars().take(18) {
        draw_char(canvas, cursor, y, ch, color);
        cursor += 7;
    }
}

fn draw_char(canvas: &mut RgbImage, x: u32, y: u32, ch: char, color: Rgb<u8>) {
    let pattern = font_pattern(ch);
    for (row, bits) in pattern.iter().enumerate() {
        for (col, bit) in bits.chars().enumerate() {
            if bit == '1' {
                for dy in 0..2 {
                    for dx in 0..2 {
                        let px = x + col as u32 * 2 + dx;
                        let py = y + row as u32 * 2 + dy;
                        if px < canvas.width() && py < canvas.height() {
                            canvas.put_pixel(px, py, color);
                        }
                    }
                }
            }
        }
    }
}

fn font_pattern(ch: char) -> [&'static str; 7] {
    match ch {
        '0' => ["111", "101", "101", "101", "101", "101", "111"],
        '1' => ["010", "110", "010", "010", "010", "010", "111"],
        '2' => ["111", "001", "001", "111", "100", "100", "111"],
        '3' => ["111", "001", "001", "111", "001", "001", "111"],
        '4' => ["101", "101", "101", "111", "001", "001", "001"],
        '5' => ["111", "100", "100", "111", "001", "001", "111"],
        '6' => ["111", "100", "100", "111", "101", "101", "111"],
        '7' => ["111", "001", "001", "010", "010", "010", "010"],
        '8' => ["111", "101", "101", "111", "101", "101", "111"],
        '9' => ["111", "101", "101", "111", "001", "001", "111"],
        'A' => ["111", "101", "101", "111", "101", "101", "101"],
        'D' => ["110", "101", "101", "101", "101", "101", "110"],
        'F' => ["111", "100", "100", "110", "100", "100", "100"],
        'G' => ["111", "100", "100", "101", "101", "101", "111"],
        'I' => ["111", "010", "010", "010", "010", "010", "111"],
        'M' => ["101", "111", "111", "101", "101", "101", "101"],
        'P' => ["111", "101", "101", "111", "100", "100", "100"],
        'S' => ["111", "100", "100", "111", "001", "001", "111"],
        'T' => ["111", "010", "010", "010", "010", "010", "010"],
        'V' => ["101", "101", "101", "101", "101", "101", "010"],
        '#' => ["010", "111", "010", "010", "111", "010", "000"],
        ':' => ["000", "010", "010", "000", "010", "010", "000"],
        '-' => ["000", "000", "000", "111", "000", "000", "000"],
        ' ' => ["000", "000", "000", "000", "000", "000", "000"],
        _ => ["000", "000", "000", "000", "000", "000", "000"],
    }
}

pub(super) fn encode_preview_jpeg(
    image: DynamicImage,
    max_side: u32,
    quality: u8,
) -> Result<String, String> {
    let image = resize_dynamic_image(image, max_side);
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut bytes, quality);
    encoder
        .encode_image(&image)
        .map_err(|err| format!("failed to encode preview jpeg: {err}"))?;
    Ok(general_purpose::STANDARD.encode(bytes))
}

fn resize_dynamic_image(image: DynamicImage, max_side: u32) -> DynamicImage {
    let (width, height) = image.dimensions();
    let longest = width.max(height);
    if longest <= max_side || longest == 0 {
        return image;
    }
    let scale = max_side as f32 / longest as f32;
    let new_width = ((width as f32) * scale).round().max(1.0) as u32;
    let new_height = ((height as f32) * scale).round().max(1.0) as u32;
    image.resize(new_width, new_height, FilterType::Lanczos3)
}
