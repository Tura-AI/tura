use super::files::{downloaded_file_value, write_unique_download};
use super::filter::filter_results;
use super::html::{direct_webpage_urls, title_from_url};
use super::search::{resolve_page_url, search_websites};
use super::types::{SearchResult, WebDiscoverArgs};
use super::util::{
    clean_text, extension_from_url, html_unescape, json_unescape, safe_filename, truncate_chars,
};
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[derive(Clone, Debug)]
pub(super) struct AssetSourceQuery {
    pub(super) source: &'static str,
    pub(super) asset_type: String,
    pub(super) query: String,
}

#[derive(Clone, Debug)]
struct AssetCandidate {
    title: String,
    download_url: Option<String>,
    page_url: Option<String>,
    snippet: String,
    source: String,
    asset_type: String,
    license: Option<String>,
}

type AssetRecordsResult = (Vec<Value>, Vec<Value>, Vec<String>);

pub(super) fn asset_records(
    args: &WebDiscoverArgs,
    client: &Client,
    search_query: &str,
    output_dir: Option<&Path>,
    session_dir: &Path,
) -> Result<AssetRecordsResult, String> {
    let (candidates, searched_sources) = discover_asset_candidates(args, client, search_query)?;
    if let Some(output_dir) = output_dir {
        download_asset_candidates(args, client, &candidates, output_dir, session_dir)
            .map(|(records, files)| (records, files, searched_sources))
    } else {
        let records = candidates
            .iter()
            .map(|candidate| asset_candidate_record(candidate, None, Vec::new(), None))
            .collect();
        Ok((records, Vec::new(), searched_sources))
    }
}

pub(super) fn asset_source_queries(asset_type: &str, query: &str) -> Vec<AssetSourceQuery> {
    let q = query.trim();
    let mut sources = Vec::new();
    let mut push = |source: &'static str, typed: &str, template: &str| {
        sources.push(AssetSourceQuery {
            source,
            asset_type: typed.to_string(),
            query: template.replace("{q}", q),
        });
    };

    if matches!(asset_type, "auto" | "3d") {
        push(
            "polydown_poly_pizza",
            "3d",
            "polydown poly pizza {q} glb 3d model",
        );
        push(
            "objaverse",
            "3d",
            "site:objaverse.allenai.org {q} 3d model glb",
        );
        push(
            "sketchfab_download_api",
            "3d",
            "site:sketchfab.com {q} downloadable 3d model glb",
        );
    }

    if matches!(asset_type, "auto" | "texture") {
        push(
            "ambientcg_api",
            "texture",
            "site:ambientcg.com {q} texture material download",
        );
        push(
            "polyhaven",
            "texture",
            "site:polyhaven.com {q} texture hdr hdri download",
        );
    }

    if matches!(asset_type, "auto" | "2d") {
        push(
            "kenney",
            "2d",
            "site:kenney.nl/assets {q} 2d sprites ui download",
        );
        push(
            "opengameart_2d",
            "2d",
            "site:opengameart.org {q} 2d sprite ui asset",
        );
    }

    if matches!(asset_type, "auto" | "shader") {
        push(
            "magic_ui",
            "shader",
            "site:magicui.design {q} shader animation component",
        );
        push(
            "shadcn_ui",
            "shader",
            "site:ui.shadcn.com {q} shader component",
        );
        push(
            "shader_repos",
            "shader",
            "site:github.com {q} shader glsl wgsl",
        );
    }

    if matches!(asset_type, "auto" | "audio") {
        push(
            "freesound_api",
            "audio",
            "site:freesound.org {q} sound effect wav",
        );
        push(
            "opengameart_audio",
            "audio",
            "site:opengameart.org {q} sound effect audio",
        );
    }

    push(
        "internet_archive",
        match asset_type {
            "3d" | "texture" | "2d" | "shader" | "audio" => asset_type,
            _ => "auto",
        },
        "site:archive.org {q} asset download zip glb wav png",
    );
    sources
}

fn discover_asset_candidates(
    args: &WebDiscoverArgs,
    client: &Client,
    search_query: &str,
) -> Result<(Vec<AssetCandidate>, Vec<String>), String> {
    let requested_type = args.asset_type.as_deref().unwrap_or("auto");
    let direct_urls = direct_webpage_urls(search_query);
    if !direct_urls.is_empty() {
        let candidates = direct_urls
            .into_iter()
            .map(|url| {
                let asset_type = classify_asset_type(&url, &url, None, requested_type);
                AssetCandidate {
                    title: title_from_url(&url),
                    download_url: Some(url.clone()),
                    page_url: None,
                    snippet: "Direct asset URL from query.".to_string(),
                    source: "direct_asset_url".to_string(),
                    asset_type,
                    license: None,
                }
            })
            .collect();
        return Ok((candidates, vec!["direct_asset_url".to_string()]));
    }

    let source_queries = asset_source_queries(requested_type, search_query);
    let mut searched_sources = Vec::new();
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let per_source_limit = args.max_results.clamp(1, 4);
    let hard_candidate_cap = args.max_results.clamp(1, 20) * 4;
    let mut errors = Vec::new();

    for source_query in source_queries {
        searched_sources.push(source_query.source.to_string());
        let results = match search_websites(client, &source_query.query, per_source_limit) {
            Ok(results) => filter_results(results, args)?,
            Err(err) => {
                errors.push(format!("{}: {err}", source_query.source));
                continue;
            }
        };

        for result in results {
            if candidates.len() >= hard_candidate_cap {
                continue;
            }
            let candidate = enrich_asset_candidate(client, result, &source_query);
            let dedupe_key = candidate
                .download_url
                .as_ref()
                .or(candidate.page_url.as_ref())
                .cloned()
                .unwrap_or_else(|| candidate.title.clone());
            if seen.insert(dedupe_key) {
                candidates.push(candidate);
            }
        }
    }

    if candidates.is_empty() && !errors.is_empty() {
        return Err(format!(
            "asset source searches failed: {}",
            errors.join(" | ")
        ));
    }
    candidates.truncate(args.max_results.clamp(1, 20));
    Ok((candidates, searched_sources))
}

fn enrich_asset_candidate(
    client: &Client,
    result: SearchResult,
    source_query: &AssetSourceQuery,
) -> AssetCandidate {
    let direct_url = looks_like_direct_asset_url(&result.url).then(|| result.url.clone());
    let page_url = if direct_url.is_some() {
        result.page_url.clone()
    } else {
        Some(result.url.clone())
    };
    let mut candidate = AssetCandidate {
        title: result.title.clone(),
        download_url: direct_url,
        page_url,
        snippet: result.snippet.clone(),
        source: source_query.source.to_string(),
        asset_type: classify_asset_type(
            &result.url,
            &format!("{} {}", result.title, result.snippet),
            Some(source_query.source),
            &source_query.asset_type,
        ),
        license: None,
    };

    if candidate.download_url.is_none() {
        if let Some(page_url) = candidate.page_url.clone() {
            if let Some(enriched) =
                extract_downloadable_asset_from_page(client, &page_url, &source_query.asset_type)
            {
                candidate.download_url = Some(enriched.url);
                candidate.license = enriched.license;
                candidate.asset_type = classify_asset_type(
                    candidate.download_url.as_deref().unwrap_or_default(),
                    &candidate.title,
                    Some(source_query.source),
                    &source_query.asset_type,
                );
            }
        }
    }

    candidate
}

struct PageAsset {
    url: String,
    license: Option<String>,
}

fn extract_downloadable_asset_from_page(
    client: &Client,
    page_url: &str,
    fallback_type: &str,
) -> Option<PageAsset> {
    if let Some(url) = sketchfab_download_url(client, page_url) {
        return Some(PageAsset { url, license: None });
    }
    let html = client
        .get(page_url)
        .send()
        .and_then(|reply| reply.error_for_status())
        .ok()?
        .text()
        .ok()?;

    if let Some(url) = extract_poly_pizza_glb(&html) {
        return Some(PageAsset { url, license: None });
    }

    let license = extract_license_text(&html);
    let links = extract_asset_links(&html, page_url, fallback_type);
    links
        .into_iter()
        .next()
        .map(|url| PageAsset { url, license })
}

fn sketchfab_download_url(client: &Client, page_url: &str) -> Option<String> {
    let model_id = Regex::new(r#"/3d-models/[^/]+-([0-9a-f]{32})"#)
        .ok()
        .and_then(|re| re.captures(page_url))
        .and_then(|capture| capture.get(1).map(|value| value.as_str().to_string()))?;
    let token = std::env::var("TURA_SKETCHFAB_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("SKETCHFAB_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })?;
    let raw = client
        .get(format!(
            "https://api.sketchfab.com/v3/models/{model_id}/download"
        ))
        .bearer_auth(token)
        .send()
        .and_then(|reply| reply.error_for_status())
        .ok()?
        .json::<Value>()
        .ok()?;
    raw.get("gltf")
        .and_then(|value| value.get("url"))
        .or_else(|| raw.get("glb").and_then(|value| value.get("url")))
        .and_then(Value::as_str)
        .filter(|url| url.starts_with("http"))
        .map(ToString::to_string)
}

fn extract_poly_pizza_glb(html: &str) -> Option<String> {
    let decoded = json_unescape(html).replace("\\u002F", "/");
    for pattern in [
        r#"https?://static\.poly\.pizza/[^"'\s<>)]+?\.glb(?:[?#][^"'\s<>)\\]*)?"#,
        r#""ResourceID"\s*:\s*"([0-9a-fA-F-]{20,})""#,
        r#""resourceId"\s*:\s*"([0-9a-fA-F-]{20,})""#,
    ] {
        let Ok(re) = Regex::new(pattern) else {
            continue;
        };
        if pattern.starts_with("https?") {
            if let Some(url) = re.find(&decoded).map(|item| item.as_str().to_string()) {
                return Some(url);
            }
        } else if let Some(id) = re
            .captures(&decoded)
            .and_then(|capture| capture.get(1).map(|value| value.as_str()))
        {
            return Some(format!("https://static.poly.pizza/{id}.glb"));
        }
    }
    None
}

fn extract_asset_links(html: &str, base_url: &str, fallback_type: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |candidate: &str| {
        let decoded = html_unescape(&json_unescape(candidate))
            .replace("\\u002F", "/")
            .replace("\\/", "/");
        if let Some(url) = resolve_page_url(base_url, &decoded) {
            if asset_url_matches_type(&url, fallback_type) && seen.insert(url.clone()) {
                out.push(url);
            }
        }
    };

    if let Ok(attr_re) = Regex::new(
        r#"(?is)\b(?:href|src|data-src|data-download|download|content)\s*=\s*['"]([^'"]+)['"]"#,
    ) {
        for capture in attr_re.captures_iter(html) {
            if let Some(value) = capture.get(1) {
                push(value.as_str());
            }
        }
    }

    if let Ok(url_re) = Regex::new(
        r#"https?:\\?/\\?/[^"'\s<>)]+?\.(?:zip|glb|gltf|obj|fbx|blend|stl|usdz|dae|png|jpg|jpeg|webp|svg|gif|avif|hdr|exr|ktx2|dds|tga|wav|mp3|ogg|flac|m4a|aac|opus|glsl|wgsl|vert|frag|hlsl)(?:[?#][^"'\s<>)\\]*)?"#,
    ) {
        for item in url_re.find_iter(html) {
            push(item.as_str());
        }
    }

    out.sort_by_key(|url| asset_link_rank(url, fallback_type));
    out
}

fn extract_license_text(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    for marker in ["cc0", "creative commons", "public domain", "mit license"] {
        if let Some(index) = lower.find(marker) {
            let start = index.saturating_sub(80);
            let end = (index + 160).min(html.len());
            return Some(truncate_chars(&clean_text(&html[start..end]), 180));
        }
    }
    None
}

fn download_asset_candidates(
    args: &WebDiscoverArgs,
    client: &Client,
    candidates: &[AssetCandidate],
    output_dir: &Path,
    session_dir: &Path,
) -> Result<(Vec<Value>, Vec<Value>), String> {
    let mut records = Vec::new();
    let mut downloaded = Vec::new();

    for (index, candidate) in candidates.iter().enumerate() {
        let Some(url) = candidate.download_url.as_deref() else {
            records.push(asset_candidate_record(
                candidate,
                None,
                Vec::new(),
                Some("no direct downloadable asset found on result page".to_string()),
            ));
            continue;
        };
        let bytes = match client
            .get(url)
            .send()
            .and_then(|reply| reply.error_for_status())
            .and_then(|reply| reply.bytes())
        {
            Ok(bytes) => bytes,
            Err(err) => {
                records.push(asset_candidate_record(
                    candidate,
                    None,
                    Vec::new(),
                    Some(format!("download failed: {err}")),
                ));
                continue;
            }
        };
        let size = bytes.len() as u64;
        if size < args.min_size || size > args.max_size {
            records.push(asset_candidate_record(
                candidate,
                None,
                Vec::new(),
                Some(format!(
                    "download size {size} bytes outside {}..{}",
                    args.min_size, args.max_size
                )),
            ));
            continue;
        }

        let asset_type = candidate.asset_type.as_str();
        let type_dir = output_dir.join(asset_type);
        std::fs::create_dir_all(&type_dir)
            .map_err(|err| format!("failed to create asset output dir: {err}"))?;
        let ext = extension_from_url(url).unwrap_or("bin");
        let base_name = format!("{:02}-{}", index + 1, safe_filename(&candidate.title));
        let path = if ext == "zip" {
            let archive_dir = type_dir.join("archives");
            std::fs::create_dir_all(&archive_dir)
                .map_err(|err| format!("failed to create asset archive dir: {err}"))?;
            write_unique_download(&archive_dir, &base_name, ext, bytes.as_ref())?
        } else {
            write_unique_download(&type_dir, &base_name, ext, bytes.as_ref())?
        };
        let file = downloaded_file_value(
            &path,
            session_dir,
            url,
            candidate.page_url.as_deref(),
            asset_type,
        );

        let extracted_files = if ext == "zip" {
            let extract_dir = unique_dir(&type_dir, &format!("{base_name}-extracted"))?;
            extract_zip_archive(
                &path,
                &extract_dir,
                session_dir,
                url,
                candidate.page_url.as_deref(),
                asset_type,
            )?
        } else {
            Vec::new()
        };
        downloaded.push(file.clone());
        downloaded.extend(extracted_files.iter().cloned());
        records.push(asset_candidate_record(
            candidate,
            Some(file),
            extracted_files,
            None,
        ));
    }

    Ok((records, downloaded))
}

fn extract_zip_archive(
    archive_path: &Path,
    extract_dir: &Path,
    session_dir: &Path,
    source_url: &str,
    source_page_url: Option<&str>,
    asset_type: &str,
) -> Result<Vec<Value>, String> {
    std::fs::create_dir_all(extract_dir)
        .map_err(|err| format!("failed to create zip extract dir: {err}"))?;
    let file = File::open(archive_path).map_err(|err| format!("failed to open zip: {err}"))?;
    let mut archive = ZipArchive::new(file).map_err(|err| format!("failed to read zip: {err}"))?;
    let mut out = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| format!("failed to read zip entry: {err}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(enclosed_name) = entry.enclosed_name() else {
            continue;
        };
        let target = extract_dir.join(enclosed_name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create zip entry dir: {err}"))?;
        }
        let mut target_file =
            File::create(&target).map_err(|err| format!("failed to write zip entry: {err}"))?;
        std::io::copy(&mut entry, &mut target_file)
            .map_err(|err| format!("failed to extract zip entry: {err}"))?;
        out.push(downloaded_file_value(
            &target,
            session_dir,
            source_url,
            source_page_url,
            asset_type,
        ));
    }
    Ok(out)
}

fn unique_dir(parent: &Path, base_name: &str) -> Result<PathBuf, String> {
    for copy in 0..1000 {
        let suffix = if copy == 0 {
            String::new()
        } else {
            format!("-{copy}")
        };
        let path = parent.join(format!("{base_name}{suffix}"));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(format!("failed to create unique extract dir: {err}")),
        }
    }
    Err(format!(
        "failed to choose unique extract dir for {base_name}"
    ))
}

fn asset_candidate_record(
    candidate: &AssetCandidate,
    file: Option<Value>,
    extracted_files: Vec<Value>,
    error: Option<String>,
) -> Value {
    json!({
        "title": &candidate.title,
        "url": candidate.download_url.clone(),
        "page_url": candidate.page_url.clone(),
        "snippet": &candidate.snippet,
        "source": &candidate.source,
        "asset_type": &candidate.asset_type,
        "license": candidate.license.clone(),
        "file_type": "asset",
        "local_path": file.as_ref().map(|value| value["path"].clone()),
        "size": file.as_ref().map(|value| value["size"].clone()),
        "extracted_files": extracted_files,
        "download_error": error,
    })
}

fn looks_like_direct_asset_url(url: &str) -> bool {
    extension_from_url(url).is_some_and(|ext| {
        matches!(
            ext,
            "zip"
                | "glb"
                | "gltf"
                | "obj"
                | "fbx"
                | "blend"
                | "stl"
                | "usdz"
                | "dae"
                | "png"
                | "jpg"
                | "jpeg"
                | "webp"
                | "svg"
                | "gif"
                | "avif"
                | "hdr"
                | "exr"
                | "ktx2"
                | "dds"
                | "tga"
                | "wav"
                | "mp3"
                | "ogg"
                | "flac"
                | "m4a"
                | "aac"
                | "opus"
                | "glsl"
                | "wgsl"
                | "vert"
                | "frag"
                | "hlsl"
        )
    })
}

fn asset_url_matches_type(url: &str, fallback_type: &str) -> bool {
    let Some(ext) = extension_from_url(url) else {
        return false;
    };
    match fallback_type {
        "3d" => matches!(
            ext,
            "zip" | "glb" | "gltf" | "obj" | "fbx" | "blend" | "stl" | "usdz" | "dae"
        ),
        "texture" => matches!(
            ext,
            "zip" | "png" | "jpg" | "jpeg" | "webp" | "hdr" | "exr" | "ktx2" | "dds" | "tga"
        ),
        "2d" => matches!(
            ext,
            "zip" | "png" | "jpg" | "jpeg" | "webp" | "svg" | "gif" | "avif"
        ),
        "shader" => matches!(
            ext,
            "zip" | "glsl" | "wgsl" | "vert" | "frag" | "hlsl" | "js" | "ts" | "tsx"
        ),
        "audio" => matches!(
            ext,
            "zip" | "wav" | "mp3" | "ogg" | "flac" | "m4a" | "aac" | "opus"
        ),
        _ => looks_like_direct_asset_url(url),
    }
}

fn asset_link_rank(url: &str, fallback_type: &str) -> (u8, String) {
    let ext = extension_from_url(url).unwrap_or("bin");
    let rank = match fallback_type {
        "3d" => match ext {
            "glb" => 0,
            "gltf" => 1,
            "zip" => 2,
            "obj" | "fbx" | "blend" => 3,
            _ => 9,
        },
        "texture" => match ext {
            "zip" => 0,
            "hdr" | "exr" | "ktx2" => 1,
            "png" | "jpg" | "jpeg" | "webp" => 2,
            _ => 9,
        },
        "audio" => match ext {
            "wav" | "ogg" | "flac" => 0,
            "mp3" | "m4a" => 1,
            "zip" => 2,
            _ => 9,
        },
        "shader" => match ext {
            "glsl" | "wgsl" | "vert" | "frag" | "hlsl" => 0,
            "zip" => 1,
            _ => 9,
        },
        _ => match ext {
            "zip" => 1,
            _ => 0,
        },
    };
    (rank, url.to_string())
}

fn classify_asset_type(url: &str, text: &str, source: Option<&str>, fallback_type: &str) -> String {
    if fallback_type != "auto" {
        return fallback_type.to_string();
    }
    let source_text = format!(
        "{} {} {}",
        url.to_ascii_lowercase(),
        text.to_ascii_lowercase(),
        source.unwrap_or_default().to_ascii_lowercase()
    );
    if has_any(
        &source_text,
        &["freesound", ".wav", ".mp3", ".ogg", "sound", "sfx"],
    ) {
        "audio".to_string()
    } else if has_any(
        &source_text,
        &[
            "poly.pizza",
            "objaverse",
            "sketchfab",
            ".glb",
            ".gltf",
            ".obj",
            ".fbx",
            "3d model",
        ],
    ) {
        "3d".to_string()
    } else if has_any(
        &source_text,
        &[
            "ambientcg",
            "polyhaven",
            "texture",
            "material",
            ".hdr",
            ".exr",
            ".ktx2",
        ],
    ) {
        "texture".to_string()
    } else if has_any(
        &source_text,
        &["shader", ".glsl", ".wgsl", "magicui", "shadcn"],
    ) {
        "shader".to_string()
    } else {
        "2d".to_string()
    }
}

fn has_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}
