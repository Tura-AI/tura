use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

const SERVICES: &[&str] = &["gateway", "runtime", "router", "session_log"];
const BOUNDARIES: &[&str] = &[
    "lifecycle",
    "router_contract",
    "runtime_contract",
    "session_log_contract",
];

#[test]
fn service_and_boundary_dependency_directions_are_enforced() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("lifecycle crate should be inside the workspace")
        .to_path_buf();
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["metadata", "--format-version=1", "--locked"])
        .current_dir(&workspace)
        .output()
        .expect("cargo metadata should start");
    assert!(
        output.status.success(),
        "cargo metadata failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata: Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata should return JSON");
    let package_names = metadata["packages"]
        .as_array()
        .expect("metadata packages should be an array")
        .iter()
        .map(|package| {
            (
                package["id"].as_str().expect("package id").to_string(),
                package["name"].as_str().expect("package name").to_string(),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut forbidden = Vec::new();
    for node in metadata["resolve"]["nodes"]
        .as_array()
        .expect("resolved dependency nodes should be present")
    {
        let source = package_names
            .get(node["id"].as_str().expect("dependency node id"))
            .expect("dependency source package should exist");
        for dependency in node["deps"]
            .as_array()
            .expect("node deps should be an array")
        {
            let target = package_names
                .get(dependency["pkg"].as_str().expect("dependency package id"))
                .expect("dependency target package should exist");
            for kind in dependency["dep_kinds"]
                .as_array()
                .expect("dependency kinds should be an array")
            {
                let kind = kind["kind"].as_str().unwrap_or("normal");
                let service_to_service = source != target
                    && SERVICES.contains(&source.as_str())
                    && SERVICES.contains(&target.as_str())
                    && matches!(kind, "normal" | "build");
                let reversed_boundary =
                    BOUNDARIES.contains(&source.as_str()) && SERVICES.contains(&target.as_str());
                if service_to_service || reversed_boundary {
                    forbidden.push(format!("{source} -[{kind}]-> {target}"));
                }
            }
        }
    }

    forbidden.sort();
    forbidden.dedup();
    assert!(
        forbidden.is_empty(),
        "forbidden service dependency edges:\n{}",
        forbidden.join("\n")
    );
}
