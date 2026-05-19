pub fn workspace_context(workspace_directory: &str) -> String {
    format!(
        "Current workspace directory: {workspace_directory}. Treat relative file and directory paths as relative to this workspace. When the user says this directory or current directory, they mean this workspace."
    )
}
