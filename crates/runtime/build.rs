use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() -> io::Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let runtime_prompt_root = manifest_dir.join("src").join("runtime_prompt");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let generated_path = out_dir.join("runtime_prompt_manuals.rs");

    println!("cargo:rerun-if-changed={}", runtime_prompt_root.display());

    let mut prompt_dirs = Vec::new();
    if let Ok(entries) = fs::read_dir(&runtime_prompt_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            println!("cargo:rerun-if-changed={}", path.display());
            let identity_path = path.join("prompt_identity.json");
            let prompt_path = path.join("prompt.md");
            if identity_path.is_file() && prompt_path.is_file() {
                println!("cargo:rerun-if-changed={}", identity_path.display());
                println!("cargo:rerun-if-changed={}", prompt_path.display());
                prompt_dirs.push(path);
            }
        }
    }

    prompt_dirs.sort();

    let mut generated = String::from(
        "const EMBEDDED_RUNTIME_PROMPT_MANUALS: &[EmbeddedRuntimePromptManual] = &[\n",
    );
    for dir in prompt_dirs {
        let identity_path = rust_string_literal(&dir.join("prompt_identity.json"));
        let prompt_path = rust_string_literal(&dir.join("prompt.md"));
        generated.push_str("    EmbeddedRuntimePromptManual {\n");
        generated.push_str("        identity_json: include_str!(");
        generated.push_str(&identity_path);
        generated.push_str("),\n");
        generated.push_str("        prompt: include_str!(");
        generated.push_str(&prompt_path);
        generated.push_str("),\n");
        generated.push_str("    },\n");
    }
    generated.push_str("];\n");

    fs::write(generated_path, generated)
}

fn rust_string_literal(path: &Path) -> String {
    format!("{:?}", path.to_string_lossy())
}
