use std::path::Path;

pub const PERMISSIONS_INSTRUCTIONS: &str = "<permissions instructions>\nFilesystem sandboxing defines which files can be read or written. `sandbox_mode` is `danger-full-access`: No filesystem sandboxing - all commands are permitted. Network access is enabled.\nApproval policy is currently never. Do not provide the `sandbox_permissions` for any reason, commands will be rejected.\n</permissions instructions>";

pub fn environment_context(
    cwd: &Path,
    shell: &str,
    current_date: impl std::fmt::Display,
    timezone: &str,
    system_language: &str,
) -> String {
    format!(
        "<environment_context>\n  <cwd>{}</cwd>\n  <workspace_roots>\n    <root>{}</root>\n  </workspace_roots>\n  <shell>{}</shell>\n  <current_date>{}</current_date>\n  <timezone>{}</timezone>\n  <system_language>{}</system_language>\n</environment_context>",
        cwd.display(),
        cwd.display(),
        shell.trim(),
        current_date,
        timezone.trim(),
        system_language.trim(),
    )
}

pub fn current_objective_block(overall: &str, current_task: Option<&str>) -> String {
    let overall = overall.trim();
    match current_task.map(str::trim).filter(|task| !task.is_empty()) {
        Some(task) => format!("[current objective]:\n{overall}\n\n{task}"),
        None => format!("[current objective]:\n{overall}"),
    }
}

pub fn current_task_text(task_summary: &str) -> &str {
    task_summary.trim()
}

#[cfg(test)]
mod tests {
    use super::{current_objective_block, environment_context};
    use std::path::Path;

    #[test]
    fn environment_context_formats_dynamic_values() {
        let content = environment_context(
            Path::new("C:/workspace"),
            "powershell",
            "2026-06-19",
            "Europe/Paris",
            "zh",
        );

        assert!(content.contains("<cwd>C:/workspace</cwd>"));
        assert!(content.contains("<shell>powershell</shell>"));
        assert!(content.contains("<current_date>2026-06-19</current_date>"));
        assert!(content.contains("<timezone>Europe/Paris</timezone>"));
        assert!(content.contains("<system_language>zh</system_language>"));
    }

    #[test]
    fn current_objective_block_keeps_optional_current_task_shape() {
        assert_eq!(
            current_objective_block("ship feature", None),
            "[current objective]:\nship feature"
        );
        assert_eq!(
            current_objective_block("ship feature", Some("patch parser")),
            "[current objective]:\nship feature\n\npatch parser"
        );
    }
}
