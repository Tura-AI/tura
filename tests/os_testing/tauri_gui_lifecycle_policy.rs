use std::fs;
use std::path::PathBuf;

#[test]
fn tauri_gui_owns_single_instance_and_restore_contract() {
    let root = repo_root();
    let cargo_toml = fs::read_to_string(root.join("apps/tauri/src-tauri/Cargo.toml"))
        .expect("read Tauri Cargo.toml");
    let main_rs = fs::read_to_string(root.join("apps/tauri/src-tauri/src/main.rs"))
        .expect("read Tauri main.rs");
    let tauri_conf = fs::read_to_string(root.join("apps/tauri/src-tauri/tauri.conf.json"))
        .expect("read Tauri config");

    assert!(
        cargo_toml.contains("tauri-plugin-single-instance"),
        "tura_gui must depend on Tauri's single-instance plugin so every launcher shares one GUI process"
    );

    let builder = main_rs
        .find("tauri::Builder::default()")
        .expect("Tauri builder is present");
    let plugin = main_rs
        .find("tauri_plugin_single_instance::init")
        .expect("single-instance plugin is registered");
    let setup = main_rs.find(".setup(").expect("Tauri setup is present");

    assert!(
        builder < plugin && plugin < setup,
        "single-instance must be the first Tauri plugin so duplicate GUI launches are intercepted before setup"
    );
    assert!(
        main_rs.contains("restore_main_window_from_args"),
        "duplicate launches must reuse the same restore/navigation path as first launch"
    );
    assert!(
        main_rs.contains("window.show()")
            && main_rs.contains("window.unminimize()")
            && main_rs.contains("window.set_focus()"),
        "restoring an existing GUI must show, unminimize, and focus the main window"
    );
    assert!(
        tauri_conf.contains("\"label\": \"main\""),
        "the configured Tauri window must be explicitly labeled main so permissions and restore logic target the same window"
    );
    assert!(
        main_rs.contains("is_gui_startup_base_url"),
        "cold-start restore must reject transient blank URLs before applying tray launch arguments"
    );
    assert!(
        !main_rs.contains("hide_child_console_window_and_detach")
            && !main_rs.contains("process_group(0)")
            && !main_rs.contains("spawn_gateway_child")
            && !main_rs.contains("OWNED_GATEWAY"),
        "standalone GUI must only connect to gateway; it must not spawn or own a gateway process"
    );
    assert!(
        main_rs.contains("gateway is not running at")
            && main_rs.contains("start tura_gateway before launching tura_gui"),
        "standalone GUI must fail clearly when no gateway is reachable"
    );
}

fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("apps/tauri/src-tauri/Cargo.toml").exists() {
            return dir;
        }
        assert!(dir.pop(), "repo root not found from CARGO_MANIFEST_DIR");
    }
}
