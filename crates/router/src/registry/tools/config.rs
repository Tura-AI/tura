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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::tools::manifest::{LimitsSection, PathsSection, RuntimeSection};
    use router_contract::ConfigurableEntry;
    use serde_json::json;

    #[test]
    fn validate_configurable_values_accepts_known_enum_string_and_boolean_values() {
        let manifest = manifest(vec![
            configurable("mode", "enum", json!("fast"), &["fast", "safe"]),
            configurable("label", "string", json!("default"), &[]),
            configurable("enabled", "boolean", json!(true), &[]),
        ]);
        let values = BTreeMap::from([
            ("mode".to_string(), json!("safe")),
            ("label".to_string(), json!("custom")),
            ("enabled".to_string(), json!(false)),
        ]);

        validate_configurable_values(&manifest, &values).expect("valid config");
    }

    #[test]
    fn validate_configurable_values_rejects_unknown_keys_and_wrong_types() {
        let manifest = manifest(vec![
            configurable("mode", "enum", json!("fast"), &["fast", "safe"]),
            configurable("label", "string", json!("default"), &[]),
            configurable("enabled", "boolean", json!(true), &[]),
        ]);

        let unknown = BTreeMap::from([("binary".to_string(), json!("tool"))]);
        assert_eq!(
            validate_configurable_values(&manifest, &unknown).expect_err("unknown key"),
            "unknown configurable key: binary"
        );

        let enum_number = BTreeMap::from([("mode".to_string(), json!(1))]);
        assert_eq!(
            validate_configurable_values(&manifest, &enum_number).expect_err("enum type"),
            "enum configurable mode must be a string"
        );

        let string_bool = BTreeMap::from([("label".to_string(), json!(true))]);
        assert_eq!(
            validate_configurable_values(&manifest, &string_bool).expect_err("string type"),
            "string configurable label must be a string"
        );

        let bool_string = BTreeMap::from([("enabled".to_string(), json!("yes"))]);
        assert_eq!(
            validate_configurable_values(&manifest, &bool_string).expect_err("boolean type"),
            "boolean configurable enabled must be a boolean"
        );
    }

    #[test]
    fn validate_configurable_values_rejects_enum_values_outside_allowlist() {
        let manifest = manifest(vec![configurable(
            "mode",
            "enum",
            json!("fast"),
            &["fast", "safe"],
        )]);
        let values = BTreeMap::from([("mode".to_string(), json!("turbo"))]);

        assert_eq!(
            validate_configurable_values(&manifest, &values).expect_err("invalid enum"),
            "invalid enum value for mode: turbo"
        );
    }

    #[test]
    fn validate_configurable_values_rejects_unsupported_configurable_type() {
        let manifest = manifest(vec![configurable("count", "integer", json!(1), &[])]);
        let values = BTreeMap::from([("count".to_string(), json!(2))]);

        assert_eq!(
            validate_configurable_values(&manifest, &values).expect_err("unsupported type"),
            "unsupported configurable type for count"
        );
    }

    fn manifest(configurable: Vec<ConfigurableEntry>) -> ToolManifest {
        ToolManifest {
            id: "test_tool".to_string(),
            name: "Test Tool".to_string(),
            description: "Test manifest".to_string(),
            core: false,
            category: "test".to_string(),
            execution: "one_shot".to_string(),
            state_machine: "default".to_string(),
            supports_macro_command: false,
            mutating: false,
            network: false,
            runtime: RuntimeSection {
                binary: "test-tool".to_string(),
                entry: String::new(),
                language: "rust".to_string(),
            },
            limits: LimitsSection {
                default_timeout_ms: 100,
                max_timeout_ms: 200,
            },
            paths: PathsSection {
                prompt: "prompt.md".to_string(),
                schema: "schema.json".to_string(),
                policy: "policy.toml".to_string(),
            },
            configurable,
            manifest_path: std::path::PathBuf::from("tool.json"),
        }
    }

    fn configurable(
        key: &str,
        value_type: &str,
        default: serde_json::Value,
        enum_values: &[&str],
    ) -> ConfigurableEntry {
        ConfigurableEntry {
            key: key.to_string(),
            label: key.to_string(),
            description: format!("{key} setting"),
            value_type: value_type.to_string(),
            default,
            enum_values: enum_values.iter().map(|value| value.to_string()).collect(),
            required: false,
            scope: "workspace".to_string(),
        }
    }
}
