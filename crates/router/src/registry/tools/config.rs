use super::manifest::ToolManifest;
use std::collections::{BTreeMap, BTreeSet};

pub fn validate_configurable_values(
    manifest: &ToolManifest,
    values: &BTreeMap<String, serde_json::Value>,
) -> Result<(), String> {
    let entries = manifest
        .configurable
        .iter()
        .map(|entry| (entry.key.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    for (key, value) in values {
        let entry = entries
            .get(key.as_str())
            .ok_or_else(|| format!("unknown configurable key: {key}"))?;
        match entry.value_type.as_str() {
            "enum" => {
                let Some(value) = value.as_str() else {
                    return Err(format!("enum configurable {key} must be a string"));
                };
                let allowed = entry.enum_values.iter().collect::<BTreeSet<_>>();
                if !allowed.contains(&value.to_string()) {
                    return Err(format!("invalid enum value for {key}: {value}"));
                }
            }
            "string" => {
                if !value.is_string() {
                    return Err(format!("string configurable {key} must be a string"));
                }
            }
            "boolean" => {
                if !value.is_boolean() {
                    return Err(format!("boolean configurable {key} must be a boolean"));
                }
            }
            _ => return Err(format!("unsupported configurable type for {key}")),
        }
    }
    Ok(())
}
