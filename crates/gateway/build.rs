#[cfg(windows)]
fn main() {
    let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
        .join("tura")
        .join("icon.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    if icon.exists() {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(icon.to_string_lossy().as_ref());
        if let Err(error) = resource.compile() {
            println!("cargo:warning=failed to embed Tura icon: {error}");
        }
    }
}

#[cfg(not(windows))]
fn main() {}
