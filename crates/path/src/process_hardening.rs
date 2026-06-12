#![allow(unsafe_code)]

use std::ffi::{OsStr, OsString};

const EXACT_DANGEROUS_ENV: &[&str] = &[
    "LD_PRELOAD",
    "LD_AUDIT",
    "LD_LIBRARY_PATH",
    "LD_DEBUG",
    "LD_PROFILE",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_PRINT_LIBRARIES",
    "DYLD_PRINT_TO_FILE",
    "MallocStackLogging",
    "MallocStackLoggingNoCompact",
    "MallocScribble",
    "MallocPreScribble",
    "MALLOC_LOG_FILE",
];

const PREFIX_DANGEROUS_ENV: &[&str] = &["DYLD_"];

pub fn harden_current_process(label: &str) -> Vec<OsString> {
    let os_hardening = apply_os_hardening();
    let removed = remove_dangerous_env_vars(std::env::vars_os());
    if hardening_log_enabled() {
        if !os_hardening.applied.is_empty() {
            eprintln!(
                "[tura:{label}] process hardening applied: {}",
                os_hardening.applied.join(", ")
            );
        }
        if !os_hardening.errors.is_empty() {
            eprintln!(
                "[tura:{label}] process hardening warnings: {}",
                os_hardening.errors.join("; ")
            );
        }
        if !removed.is_empty() {
            eprintln!(
                "[tura:{label}] removed process injection environment variables: {}",
                removed
                    .iter()
                    .map(|value| value.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    removed
}

pub fn remove_dangerous_env_vars(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<OsString> {
    let mut removed = Vec::new();
    for (key, _) in vars {
        if dangerous_env_key(&key) {
            std::env::remove_var(&key);
            removed.push(key);
        }
    }
    removed.sort();
    removed.dedup();
    removed
}

pub fn dangerous_env_key(key: &OsStr) -> bool {
    let Some(key) = key.to_str() else {
        return false;
    };
    EXACT_DANGEROUS_ENV
        .iter()
        .any(|candidate| key.eq_ignore_ascii_case(candidate))
        || PREFIX_DANGEROUS_ENV.iter().any(|prefix| {
            key.len() >= prefix.len() && key[..prefix.len()].eq_ignore_ascii_case(prefix)
        })
}

fn hardening_log_enabled() -> bool {
    ["TURA_PROCESS_HARDENING_LOG", "TURA_DEBUG_RUNTIME"]
        .into_iter()
        .any(|key| {
            std::env::var(key).ok().is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessHardeningStrategy {
    LinuxPrctlAndRlimit,
    MacosPtraceAndRlimit,
    UnixRlimitOnly,
    EnvOnly,
}

pub fn current_process_hardening_strategy() -> ProcessHardeningStrategy {
    if cfg!(target_os = "linux") {
        ProcessHardeningStrategy::LinuxPrctlAndRlimit
    } else if cfg!(target_os = "macos") {
        ProcessHardeningStrategy::MacosPtraceAndRlimit
    } else if cfg!(unix) {
        ProcessHardeningStrategy::UnixRlimitOnly
    } else {
        ProcessHardeningStrategy::EnvOnly
    }
}

#[derive(Debug, Default)]
struct OsHardeningReport {
    applied: Vec<&'static str>,
    errors: Vec<String>,
}

fn apply_os_hardening() -> OsHardeningReport {
    let mut report = OsHardeningReport::default();
    apply_core_dump_limit(&mut report);
    apply_platform_attach_guard(&mut report);
    report
}

#[cfg(unix)]
fn apply_core_dump_limit(report: &mut OsHardeningReport) {
    const RLIMIT_CORE: i32 = 4;
    #[repr(C)]
    struct RLimit {
        rlim_cur: u64,
        rlim_max: u64,
    }
    unsafe extern "C" {
        fn setrlimit(resource: i32, rlim: *const RLimit) -> i32;
    }

    let limit = RLimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let rc = unsafe { setrlimit(RLIMIT_CORE, &limit) };
    if rc == 0 {
        report.applied.push("core_dump_limit=0");
    } else {
        report.errors.push(format!(
            "setrlimit(RLIMIT_CORE) failed: {}",
            std::io::Error::last_os_error()
        ));
    }
}

#[cfg(not(unix))]
fn apply_core_dump_limit(_report: &mut OsHardeningReport) {}

#[cfg(target_os = "linux")]
fn apply_platform_attach_guard(report: &mut OsHardeningReport) {
    const PR_SET_DUMPABLE: i32 = 4;
    unsafe extern "C" {
        fn prctl(option: i32, arg2: usize, arg3: usize, arg4: usize, arg5: usize) -> i32;
    }

    let rc = unsafe { prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) };
    if rc == 0 {
        report.applied.push("linux_dumpable=0");
    } else {
        report.errors.push(format!(
            "prctl(PR_SET_DUMPABLE) failed: {}",
            std::io::Error::last_os_error()
        ));
    }
}

#[cfg(target_os = "macos")]
fn apply_platform_attach_guard(report: &mut OsHardeningReport) {
    const PT_DENY_ATTACH: i32 = 31;
    unsafe extern "C" {
        fn ptrace(request: i32, pid: i32, addr: *mut std::ffi::c_void, data: i32) -> i32;
    }

    let rc = unsafe { ptrace(PT_DENY_ATTACH, 0, std::ptr::null_mut(), 0) };
    if rc == 0 {
        report.applied.push("macos_deny_attach");
    } else {
        report.errors.push(format!(
            "ptrace(PT_DENY_ATTACH) failed: {}",
            std::io::Error::last_os_error()
        ));
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn apply_platform_attach_guard(_report: &mut OsHardeningReport) {}

#[cfg(test)]
mod tests {
    use super::{
        current_process_hardening_strategy, dangerous_env_key, remove_dangerous_env_vars,
        ProcessHardeningStrategy,
    };
    use std::ffi::{OsStr, OsString};

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn dangerous_env_key_detects_loader_and_malloc_injection_names() {
        for key in [
            "LD_PRELOAD",
            "ld_audit",
            "DYLD_INSERT_LIBRARIES",
            "DYLD_FAKE_SUFFIX",
            "MallocStackLogging",
            "MALLOC_LOG_FILE",
        ] {
            assert!(
                dangerous_env_key(OsStr::new(key)),
                "{key} should be removed"
            );
        }

        for key in ["PATH", "TURA_HOME", "RUST_LOG", "OPENAI_LOGIN"] {
            assert!(
                !dangerous_env_key(OsStr::new(key)),
                "{key} should be preserved"
            );
        }
    }

    #[test]
    fn remove_dangerous_env_vars_removes_only_listed_keys_from_process_env() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let previous_ld = std::env::var_os("LD_PRELOAD");
        let previous_dyld = std::env::var_os("DYLD_TEST_REMOVE");
        let previous_safe = std::env::var_os("TURA_SAFE_TEST_ENV");
        std::env::set_var("LD_PRELOAD", "inject.so");
        std::env::set_var("DYLD_TEST_REMOVE", "inject.dylib");
        std::env::set_var("TURA_SAFE_TEST_ENV", "keep");

        let removed = remove_dangerous_env_vars([
            (OsString::from("LD_PRELOAD"), OsString::from("inject.so")),
            (
                OsString::from("DYLD_TEST_REMOVE"),
                OsString::from("inject.dylib"),
            ),
            (OsString::from("TURA_SAFE_TEST_ENV"), OsString::from("keep")),
        ]);

        assert_eq!(
            removed,
            vec![
                OsString::from("DYLD_TEST_REMOVE"),
                OsString::from("LD_PRELOAD")
            ]
        );
        assert!(std::env::var_os("LD_PRELOAD").is_none());
        assert!(std::env::var_os("DYLD_TEST_REMOVE").is_none());
        assert_eq!(
            std::env::var_os("TURA_SAFE_TEST_ENV"),
            Some(OsString::from("keep"))
        );

        restore_env("LD_PRELOAD", previous_ld);
        restore_env("DYLD_TEST_REMOVE", previous_dyld);
        restore_env("TURA_SAFE_TEST_ENV", previous_safe);
    }

    #[test]
    fn hardening_strategy_matches_current_platform() {
        let strategy = current_process_hardening_strategy();
        if cfg!(target_os = "linux") {
            assert_eq!(strategy, ProcessHardeningStrategy::LinuxPrctlAndRlimit);
        } else if cfg!(target_os = "macos") {
            assert_eq!(strategy, ProcessHardeningStrategy::MacosPtraceAndRlimit);
        } else if cfg!(unix) {
            assert_eq!(strategy, ProcessHardeningStrategy::UnixRlimitOnly);
        } else {
            assert_eq!(strategy, ProcessHardeningStrategy::EnvOnly);
        }
    }

    #[test]
    fn hardening_strategy_contract_covers_supported_os_families() {
        let strategies = [
            ProcessHardeningStrategy::LinuxPrctlAndRlimit,
            ProcessHardeningStrategy::MacosPtraceAndRlimit,
            ProcessHardeningStrategy::UnixRlimitOnly,
            ProcessHardeningStrategy::EnvOnly,
        ];

        assert!(strategies.contains(&ProcessHardeningStrategy::LinuxPrctlAndRlimit));
        assert!(strategies.contains(&ProcessHardeningStrategy::MacosPtraceAndRlimit));
        assert!(strategies.contains(&ProcessHardeningStrategy::UnixRlimitOnly));
        assert!(strategies.contains(&ProcessHardeningStrategy::EnvOnly));
    }

    fn restore_env(key: &str, previous: Option<OsString>) {
        if let Some(value) = previous {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }
}
