use std::collections::BTreeMap;

use serde_json::json;
use tura_router::registry::tools::{ToolPatch, ToolRegistry};

fn registry() -> ToolRegistry {
    let current = std::env::current_dir().expect("current dir");
    let root = current
        .ancestors()
        .find(|path| path.join("crates").join("tools").is_dir() && path.join("commands").is_dir())
        .expect("repo root")
        .to_path_buf();
    ToolRegistry::discover(root)
}

#[test]
fn router_loads_core_and_external_command_manifests() {
    let tools = registry().list();
    let ids = tools
        .iter()
        .map(|tool| tool.id.as_str())
        .collect::<Vec<_>>();

    for id in [
        "shell_command",
        "bash",
        "zsh",
        "apply_patch",
        "task_status",
        "planning",
        "generate_media",
        "read_media",
        "web_discover",
    ] {
        assert!(ids.contains(&id), "missing tool manifest for {id}");
    }

    let read_media = tools
        .iter()
        .find(|tool| tool.id == "read_media")
        .expect("read_media manifest");
    assert!(!read_media.core);
    assert_eq!(read_media.execution, "one_shot");
    assert_eq!(
        read_media.binary.as_deref(),
        Some("tura-command-read-media")
    );

    let generate_media = tools
        .iter()
        .find(|tool| tool.id == "generate_media")
        .expect("generate_media manifest");
    assert!(!generate_media.core);
    assert!(generate_media.mutating);
    assert_eq!(
        generate_media.binary.as_deref(),
        Some("tura-command-generate-media")
    );

    let shell = tools
        .iter()
        .find(|tool| tool.id == "shell_command")
        .expect("shell_command manifest");
    assert!(shell.core);
    assert_eq!(shell.execution, "in_process");
}

#[test]
fn router_rejects_unsafe_tool_patch_fields() {
    let err = registry()
        .patch_tool(
            "read_media",
            ToolPatch {
                core: Some(true),
                ..ToolPatch::default()
            },
        )
        .expect_err("core is manifest-owned");
    assert!(err.contains("unsafe manifest fields"));

    let err = registry()
        .patch_tool(
            "read_media",
            ToolPatch {
                binary: Some("evil".to_string()),
                ..ToolPatch::default()
            },
        )
        .expect_err("binary is manifest-owned");
    assert!(err.contains("unsafe manifest fields"));
}

#[test]
fn router_validates_configurable_entries_and_values() {
    let config = registry()
        .config("read_media")
        .expect("read_media config should exist");
    let pdf_pages = config
        .configurable
        .iter()
        .find(|entry| entry.key == "pdf_default_pages")
        .expect("pdf_default_pages entry");
    assert_eq!(pdf_pages.value_type, "enum");
    assert!(pdf_pages.enum_values.contains(&"5".to_string()));
    assert!(pdf_pages.default.is_string());

    let mut values = BTreeMap::new();
    values.insert("pdf_default_pages".to_string(), json!("10"));
    let patched = registry()
        .patch_config("read_media", values)
        .expect("valid enum value");
    assert_eq!(patched.values["pdf_default_pages"], json!("10"));

    let mut invalid = BTreeMap::new();
    invalid.insert("pdf_default_pages".to_string(), json!(7));
    assert!(registry().patch_config("read_media", invalid).is_err());

    let mut unknown = BTreeMap::new();
    unknown.insert("binary".to_string(), json!("tura-command-read-media"));
    assert!(registry().patch_config("read_media", unknown).is_err());
}

#[test]
fn aliases_resolve_to_canonical_tool_ids() {
    assert_eq!(
        registry().get("view_media").expect("alias").id,
        "read_media"
    );
    assert_eq!(
        registry().get("text_to_image").expect("alias").id,
        "generate_media"
    );
    assert_eq!(
        registry().get("web_search").expect("alias").id,
        "web_discover"
    );
}
