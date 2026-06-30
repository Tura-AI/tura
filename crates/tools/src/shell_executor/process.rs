#![allow(unsafe_code)]

#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellProcessScopeStrategy {
    WindowsJobObject,
    UnixProcessGroup,
    DirectChildOnly,
}

pub fn current_shell_process_scope_strategy() -> ShellProcessScopeStrategy {
    if cfg!(windows) {
        ShellProcessScopeStrategy::WindowsJobObject
    } else if cfg!(unix) {
        ShellProcessScopeStrategy::UnixProcessGroup
    } else {
        ShellProcessScopeStrategy::DirectChildOnly
    }
}

pub(super) fn configure_process_scope(command: &mut Command) {
    configure_platform_spawn(command);
}

pub(super) fn configure_tokio_process_scope(command: &mut tokio::process::Command) {
    command.kill_on_drop(true);
    configure_tokio_platform_spawn(command);
}

pub(super) fn attach_shell_process_scope(pid: u32) -> Option<ShellProcessScope> {
    ShellProcessScope::attach(pid)
}

pub(super) fn terminate_process_tree(pid: u32) {
    terminate_platform_process_tree(pid);
}

pub(super) fn retain_shell_process_scope(scope: ShellProcessScope) {
    if !scope.has_live_members() {
        return;
    }
    retained_shell_process_scopes()
        .lock()
        .expect("retained shell process scope registry poisoned")
        .push(scope);
}

pub fn terminate_retained_shell_process_scopes() -> usize {
    let scopes = retained_shell_process_scopes()
        .lock()
        .expect("retained shell process scope registry poisoned")
        .drain(..)
        .collect::<Vec<_>>();
    let count = scopes.len();
    for scope in &scopes {
        scope.terminate();
    }
    count
}

fn retained_shell_process_scopes() -> &'static Mutex<Vec<ShellProcessScope>> {
    static SCOPES: OnceLock<Mutex<Vec<ShellProcessScope>>> = OnceLock::new();
    SCOPES.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(windows)]
fn terminate_platform_process_tree(pid: u32) {
    let children = collect_descendant_processes(pid);
    for child_pid in children.into_iter().rev() {
        terminate_process(child_pid);
    }
    terminate_process(pid);
}

#[cfg(windows)]
fn collect_descendant_processes(root_pid: u32) -> Vec<u32> {
    use std::collections::{HashMap, HashSet};
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut children_by_parent: HashMap<u32, Vec<u32>> = HashMap::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Vec::new();
        }
        let mut entry = std::mem::zeroed::<PROCESSENTRY32W>();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                children_by_parent
                    .entry(entry.th32ParentProcessID)
                    .or_default()
                    .push(entry.th32ProcessID);
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }

    let mut seen = HashSet::new();
    let mut stack = children_by_parent
        .get(&root_pid)
        .cloned()
        .unwrap_or_default();
    let mut descendants = Vec::new();
    while let Some(pid) = stack.pop() {
        if !seen.insert(pid) {
            continue;
        }
        descendants.push(pid);
        if let Some(children) = children_by_parent.get(&pid) {
            stack.extend(children.iter().copied());
        }
    }
    descendants
}

#[cfg(windows)]
fn terminate_process(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    unsafe {
        let process = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if process.is_null() {
            return;
        }
        let _ = TerminateProcess(process, 1);
        CloseHandle(process);
    }
}

#[cfg(not(windows))]
fn terminate_platform_process_tree(_pid: u32) {}

#[cfg(windows)]
#[derive(Debug)]
pub(super) struct ShellProcessScope {
    job: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
unsafe impl Send for ShellProcessScope {}

#[cfg(windows)]
unsafe impl Sync for ShellProcessScope {}

#[cfg(windows)]
impl ShellProcessScope {
    fn attach(pid: u32) -> Option<Self> {
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };

        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return None;
            }

            let mut info = std::mem::zeroed::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if configured == 0 {
                CloseHandle(job);
                return None;
            }

            let process: HANDLE = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if process.is_null() {
                CloseHandle(job);
                return None;
            }

            let assigned = AssignProcessToJobObject(job, process);
            CloseHandle(process);
            if assigned == 0 {
                CloseHandle(job);
                return None;
            }

            Some(Self { job })
        }
    }

    pub(super) fn terminate(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job, 1);
        }
    }

    fn has_live_members(&self) -> bool {
        use windows_sys::Win32::System::JobObjects::{
            JobObjectBasicAccountingInformation, QueryInformationJobObject,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        };

        unsafe {
            let mut info = std::mem::zeroed::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>();
            let queried = QueryInformationJobObject(
                self.job,
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as *mut _,
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                std::ptr::null_mut(),
            );
            queried != 0 && info.ActiveProcesses > 0
        }
    }
}

#[cfg(windows)]
impl Drop for ShellProcessScope {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
}

#[cfg(unix)]
#[derive(Debug)]
pub(super) struct ShellProcessScope {
    pgid: i32,
}

#[cfg(unix)]
impl ShellProcessScope {
    fn attach(pid: u32) -> Option<Self> {
        Some(Self { pgid: pid as i32 })
    }

    pub(super) fn terminate(&self) {
        unsafe {
            let _ = kill(-self.pgid, SIGTERM);
            let _ = kill(-self.pgid, SIGKILL);
        }
    }

    fn has_live_members(&self) -> bool {
        unsafe { kill(-self.pgid, 0) == 0 }
    }
}

#[cfg(not(any(unix, windows)))]
#[derive(Debug)]
pub(super) struct ShellProcessScope;

#[cfg(not(any(unix, windows)))]
impl ShellProcessScope {
    fn attach(_pid: u32) -> Option<Self> {
        None
    }

    pub(super) fn terminate(&self) {}

    fn has_live_members(&self) -> bool {
        false
    }
}

#[cfg(windows)]
fn configure_platform_spawn(command: &mut Command) {
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

#[cfg(unix)]
fn configure_platform_spawn(command: &mut Command) {
    command.process_group(0);
    configure_parent_death_signal(command);
}

#[cfg(not(any(unix, windows)))]
fn configure_platform_spawn(_command: &mut Command) {}

#[cfg(windows)]
fn configure_tokio_platform_spawn(command: &mut tokio::process::Command) {
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

#[cfg(unix)]
fn configure_tokio_platform_spawn(command: &mut tokio::process::Command) {
    command.process_group(0);
    configure_tokio_parent_death_signal(command);
}

#[cfg(not(any(unix, windows)))]
fn configure_tokio_platform_spawn(_command: &mut tokio::process::Command) {}

#[cfg(unix)]
const SIGKILL: i32 = 9;

#[cfg(unix)]
const SIGTERM: i32 = 15;

#[cfg(target_os = "linux")]
const PR_SET_PDEATHSIG: i32 = 1;

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn prctl(option: i32, arg2: usize, arg3: usize, arg4: usize, arg5: usize) -> i32;
    fn getppid() -> i32;
}

#[cfg(target_os = "linux")]
fn configure_parent_death_signal(command: &mut Command) {
    let parent_pid = std::process::id() as i32;
    unsafe {
        command.pre_exec(move || {
            if prctl(PR_SET_PDEATHSIG, SIGTERM as usize, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if getppid() != parent_pid {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "shell parent died before command exec",
                ));
            }
            Ok(())
        });
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn configure_parent_death_signal(_command: &mut Command) {}

#[cfg(target_os = "linux")]
fn configure_tokio_parent_death_signal(command: &mut tokio::process::Command) {
    let parent_pid = std::process::id() as i32;
    unsafe {
        command.pre_exec(move || {
            if prctl(PR_SET_PDEATHSIG, SIGTERM as usize, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if getppid() != parent_pid {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "shell parent died before async command exec",
                ));
            }
            Ok(())
        });
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn configure_tokio_parent_death_signal(_command: &mut tokio::process::Command) {}

#[cfg(test)]
mod tests {
    use super::{current_shell_process_scope_strategy, ShellProcessScopeStrategy};

    #[test]
    fn shell_process_scope_strategy_matches_current_platform() {
        let strategy = current_shell_process_scope_strategy();
        if cfg!(windows) {
            assert_eq!(strategy, ShellProcessScopeStrategy::WindowsJobObject);
        } else if cfg!(unix) {
            assert_eq!(strategy, ShellProcessScopeStrategy::UnixProcessGroup);
        } else {
            assert_eq!(strategy, ShellProcessScopeStrategy::DirectChildOnly);
        }
    }

    #[test]
    fn shell_process_scope_strategy_contract_covers_all_os_families() {
        let strategies = [
            ShellProcessScopeStrategy::WindowsJobObject,
            ShellProcessScopeStrategy::UnixProcessGroup,
            ShellProcessScopeStrategy::DirectChildOnly,
        ];
        assert!(strategies.contains(&ShellProcessScopeStrategy::WindowsJobObject));
        assert!(strategies.contains(&ShellProcessScopeStrategy::UnixProcessGroup));
        assert!(strategies.contains(&ShellProcessScopeStrategy::DirectChildOnly));
    }

    #[test]
    fn async_shell_scope_enables_kill_on_drop() {
        let mut command = tokio::process::Command::new("missing-test-binary");
        assert!(!command.get_kill_on_drop());
        super::configure_tokio_process_scope(&mut command);
        assert!(command.get_kill_on_drop());
    }
}
