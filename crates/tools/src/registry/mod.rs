pub mod core;
pub mod manifest;

pub use manifest::{
    command_registry_directories, discover_manifests, manifest_for, CommandExecution,
    CommandManifest,
};
