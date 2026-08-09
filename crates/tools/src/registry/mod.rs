pub mod core;
pub mod manifest;

pub use manifest::{
    CommandExecution, CommandManifest, command_registry_directories, discover_manifests,
    manifest_for,
};
