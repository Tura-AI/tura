use std::path::PathBuf;

use image::{ImageBuffer, Rgba};
use tura_command_image_generate::execute;
use tura_llm_rust::TuraConfig;

fn truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn falsy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off"
    )
}

fn live_disabled() -> bool {
    std::env::var("TURA_LIVE_IMAGE_GENERATE")
        .ok()
        .is_some_and(|value| falsy(&value))
}

fn provider_requested(provider: &str) -> bool {
    let key = format!("TURA_LIVE_IMAGE_GENERATE_{}", provider.to_ascii_uppercase());
    std::env::var(key)
        .ok()
        .map(|value| truthy(&value))
        .unwrap_or(true)
}

fn provider_has_key(provider: &str) -> bool {
    if provider == "openai" && codex_auth_json_exists() {
        return true;
    }
    provider_key_names(provider)
        .iter()
        .find_map(|key| config_value(key))
        .is_some_and(|value| !value.is_empty())
}

fn config_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            TuraConfig::default()
                .get(key)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn codex_auth_json_exists() -> bool {
    let path = std::env::var_os("CODEX_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|path| path.join("auth.json"))
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(|home| PathBuf::from(home).join(".codex").join("auth.json"))
        });
    path.is_some_and(|path| path.exists())
}

fn provider_key_names(provider: &str) -> &'static [&'static str] {
    match provider {
        "openai" => &[
            "CODEX_OPENAI_OAUTH_TOKEN",
            "CODEX_OAUTH_TOKEN",
            "OPENAI_OAUTH_TOKEN",
            "CHATGPT_OAUTH_TOKEN",
            "OPENAI_OPENAPI_KEY",
            "OPENAI_API_KEY_OPENAPI",
            "OPENAI_API_KEY",
            "CHATGPT_API_KEY",
        ],
        "replicate" => &["REPLICATE_API_TOKEN", "REPLICATE_API_KEY"],
        "gemini" => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        "grok3" => &["XAI_API_KEY", "GROK_API_KEY"],
        _ => &[],
    }
}

fn session_dir(name: &str) -> PathBuf {
    let root = std::env::current_dir()
        .expect("current dir")
        .join("target")
        .join("image-generate-live")
        .join(name);
    std::fs::create_dir_all(&root).expect("create live session dir");
    root
}

fn should_run_provider(provider: &str) -> bool {
    if live_disabled() {
        eprintln!("skipping live image_generate {provider}; TURA_LIVE_IMAGE_GENERATE disables it");
        return false;
    }
    if !provider_requested(provider) {
        eprintln!(
            "skipping live image_generate {provider}; TURA_LIVE_IMAGE_GENERATE_{} is not enabled",
            provider.to_ascii_uppercase()
        );
        return false;
    }
    if !provider_has_key(provider) {
        eprintln!(
            "skipping live image_generate {provider}; missing one of {:?}",
            provider_key_names(provider)
        );
        return false;
    }
    true
}

fn assert_generated_image(response: &tura_command_image_generate::CommandResponse, dir: &PathBuf) {
    assert!(
        response.success,
        "live image_generate failed: {}",
        response.stderr
    );
    assert_eq!(response.output["result_count"], 1);
    let path = response.output["images"][0]["path"]
        .as_str()
        .expect("generated image path");
    let absolute = dir.join(path);
    assert!(
        absolute.exists(),
        "generated image missing: {}",
        absolute.display()
    );
    assert!(
        std::fs::metadata(&absolute).expect("metadata").len() > 100,
        "generated image too small: {}",
        absolute.display()
    );
}

fn run_provider(provider: &str) {
    if !should_run_provider(provider) {
        return;
    }
    let dir = session_dir(provider);
    let command = format!(
        "--prompt \"minimal black ink icon of a cat, plain white background\" --negative-prompt \"text, watermark, blur\" --provider {provider} --width 1024 --height 1024 --quality low --n 1 --output-dir media/{provider} --format png"
    );
    let response = execute(&command, &dir, 180);
    assert_generated_image(&response, &dir);
}

fn write_reference_image(dir: &PathBuf) -> PathBuf {
    let path = dir.join("reference.png");
    let image = ImageBuffer::from_fn(128, 128, |x, y| {
        if (x / 16 + y / 16) % 2 == 0 {
            Rgba([240u8, 80, 80, 255])
        } else {
            Rgba([40u8, 120, 220, 255])
        }
    });
    image.save(&path).expect("write reference image");
    path
}

fn first_reference_provider_with_key() -> Option<&'static str> {
    ["gemini", "grok3", "openai"]
        .into_iter()
        .find(|provider| should_run_provider(provider))
}

#[test]
fn live_image_generate_openai_chatgpt_image_2() {
    run_provider("openai");
}

#[test]
fn live_image_generate_replicate_z_image_turbo() {
    run_provider("replicate");
}

#[test]
fn live_image_generate_gemini_3_1_flash() {
    run_provider("gemini");
}

#[test]
fn live_image_generate_grok3() {
    run_provider("grok3");
}

#[test]
fn live_image_generate_default_order_falls_back_to_available_provider() {
    if live_disabled() {
        eprintln!(
            "skipping live image_generate default order; TURA_LIVE_IMAGE_GENERATE disables it"
        );
        return;
    }
    let dir = session_dir("default-order");
    let response = execute(
        "--prompt \"small colorful glass cube on a clean desk, product render\" --negative-prompt \"text, watermark, blur\" --width 1024 --height 1024 --quality low --n 1 --output-dir media/default --format png",
        &dir,
        240,
    );
    assert_generated_image(&response, &dir);
}

#[test]
fn live_image_generate_reference_image_smoke() {
    let Some(provider) = first_reference_provider_with_key() else {
        eprintln!(
            "skipping live image_generate reference smoke; no reference-capable provider key"
        );
        return;
    };
    let dir = session_dir("reference-smoke");
    let reference = write_reference_image(&dir);
    let command = format!(
        "--prompt \"simple square app icon inspired by the reference colors, no text\" --negative-prompt \"letters, watermark, blur\" --provider {provider} --reference \"{}\" --width 1024 --height 1024 --quality low --n 1 --output-dir media/reference --format png",
        reference.display()
    );
    let response = execute(&command, &dir, 180);
    assert_generated_image(&response, &dir);
    assert_eq!(
        response.output["references"][0],
        reference.display().to_string()
    );
}
