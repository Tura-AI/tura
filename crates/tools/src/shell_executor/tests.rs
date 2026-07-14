use super::{execute, ShellKind};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

#[test]
fn executed_simple_batch_reads_emit_plain_output_with_blank_line_separator() {
    let root = std::env::temp_dir().join(format!("tura-shell-batch-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src")).expect("create temp src");
    fs::write(root.join("src/a.txt"), "alpha\n").expect("write a");
    fs::write(root.join("src/b.txt"), "bravo\n").expect("write b");

    let (command, shell_kind) = if cfg!(windows) {
        (
            "Get-Content src/a.txt; Get-Content src/b.txt",
            ShellKind::ShellCommand,
        )
    } else {
        ("cat src/a.txt; cat src/b.txt", ShellKind::Bash)
    };
    let response = execute(command, &root, 10, shell_kind);
    let _ = fs::remove_dir_all(&root);

    assert!(response.success, "{}", response.stderr);
    let output = response.output.as_str().unwrap_or_default();
    assert!(!output.contains("---FILE---"), "{output}");
    assert!(output.contains("alpha"), "{output}");
    assert!(output.contains("bravo"), "{output}");
    assert!(
        output.contains("alpha\n\nbravo") || output.contains("alpha\r\n\r\nbravo"),
        "{output}"
    );
}

#[test]
fn timeout_kills_descendants_that_hold_output_pipes() {
    let started = Instant::now();
    let response = execute(
        r#"{"command":"sh -c 'sleep 10 & wait'","timeout_ms":1000}"#,
        Path::new("."),
        120,
        ShellKind::Bash,
    );

    assert!(!response.success);
    assert_eq!(response.exit_code, -1);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "timeout should not wait for orphaned descendants"
    );
}

#[test]
#[cfg(unix)]
fn exited_parent_returns_even_when_descendant_holds_output_pipes() {
    let started = Instant::now();
    let response = execute(
        r#"{"command":"sh -c 'sleep 3 &'","timeout_ms":10000}"#,
        Path::new("."),
        120,
        ShellKind::Bash,
    );

    assert!(response.success);
    assert_eq!(response.exit_code, 0);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "early parent exit should not wait for descendant-held pipes"
    );
}
