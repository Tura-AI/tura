use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("check") => run_check(args.collect()),
        _ => {
            eprintln!("Usage: cargo run --manifest-path xtask/Cargo.toml -- check [quality args]");
            ExitCode::from(2)
        }
    }
}

fn run_check(args: Vec<String>) -> ExitCode {
    let script = if cfg!(windows) {
        "scripts/check-backend-quality.ps1"
    } else {
        "scripts/check-backend-quality.sh"
    };
    let status = if cfg!(windows) {
        Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script])
            .args(args)
            .status()
    } else {
        Command::new("sh").arg(script).args(args).status()
    };
    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(error) => {
            eprintln!("failed to run backend quality checks: {error}");
            ExitCode::from(1)
        }
    }
}
