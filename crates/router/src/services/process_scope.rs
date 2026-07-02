#![allow(unsafe_code)]

use anyhow::{anyhow, Result};
use tokio::process::{Child, Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessScopeStrategy {
    WindowsJobObject,
    UnixProcessGroup,
    DirectChildOnly,
}

pub fn current_process_scope_strategy() -> ProcessScopeStrategy {
    if cfg!(windows) {
        ProcessScopeStrategy::WindowsJobObject
    } else if cfg!(unix) {
        ProcessScopeStrategy::UnixProcessGroup
    } else {
        ProcessScopeStrategy::DirectChildOnly
    }
}

pub fn configure_scoped_spawn(command: &mut Command) {
    let _strategy = current_process_scope_strategy();
    command.kill_on_drop(true);
    configure_platform_spawn(command);
}

pub fn attach_child_scope(child: &Child) -> Result<Option<WorkerProcessScope>> {
    WorkerProcessScope::attach(child)
}

#[cfg(windows)]
#[derive(Debug)]
pub struct WorkerProcessScope {
    job: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
unsafe impl Send for WorkerProcessScope {}

#[cfg(windows)]
unsafe impl Sync for WorkerProcessScope {}

#[cfg(windows)]
impl WorkerProcessScope {
    fn attach(child: &Child) -> Result<Option<Self>> {
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };

        let Some(pid) = child.id() else {
            return Ok(None);
        };

        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(anyhow!("failed to create worker job object"));
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
                return Err(anyhow!("failed to configure worker job object"));
            }

            let process: HANDLE = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if process.is_null() {
                CloseHandle(job);
                return Err(anyhow!("failed to open worker process for job assignment"));
            }

            let assigned = AssignProcessToJobObject(job, process);
            CloseHandle(process);
            if assigned == 0 {
                CloseHandle(job);
                return Err(anyhow!("failed to assign worker process to job object"));
            }

            Ok(Some(Self { job }))
        }
    }

    pub fn terminate(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for WorkerProcessScope {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
}

#[cfg(unix)]
#[derive(Debug)]
pub struct WorkerProcessScope {
    pgid: i32,
}

#[cfg(unix)]
impl WorkerProcessScope {
    fn attach(child: &Child) -> Result<Option<Self>> {
        let Some(pid) = child.id() else {
            return Ok(None);
        };
        Ok(Some(Self { pgid: pid as i32 }))
    }

    pub fn terminate(&self) {
        unsafe {
            let _ = kill(-self.pgid, SIGKILL);
        }
    }
}

#[cfg(not(any(unix, windows)))]
#[derive(Debug)]
pub struct WorkerProcessScope;

#[cfg(not(any(unix, windows)))]
impl WorkerProcessScope {
    fn attach(_child: &Child) -> Result<Option<Self>> {
        Ok(None)
    }

    pub fn terminate(&self) {}
}

#[cfg(windows)]
fn configure_platform_spawn(command: &mut Command) {
    command.creation_flags(
        tura_path::process_hardening::WINDOWS_CREATE_NO_WINDOW
            | tura_path::process_hardening::WINDOWS_CREATE_NEW_PROCESS_GROUP,
    );
}

#[cfg(unix)]
fn configure_platform_spawn(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    configure_parent_death_signal(command);
}

#[cfg(not(any(unix, windows)))]
fn configure_platform_spawn(_command: &mut Command) {}

#[cfg(unix)]
const SIGKILL: i32 = 9;

#[cfg(target_os = "linux")]
const SIGTERM: i32 = 15;

#[cfg(target_os = "linux")]
const PR_SET_PDEATHSIG: i32 = 1;

#[cfg(unix)]
extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[cfg(target_os = "linux")]
extern "C" {
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
                    "router parent died before worker exec",
                ));
            }
            Ok(())
        });
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn configure_parent_death_signal(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::{current_process_scope_strategy, ProcessScopeStrategy};

    #[test]
    fn current_platform_uses_expected_worker_scope_strategy() {
        let strategy = current_process_scope_strategy();
        if cfg!(windows) {
            assert_eq!(strategy, ProcessScopeStrategy::WindowsJobObject);
        } else if cfg!(unix) {
            assert_eq!(strategy, ProcessScopeStrategy::UnixProcessGroup);
        } else {
            assert_eq!(strategy, ProcessScopeStrategy::DirectChildOnly);
        }
    }

    #[test]
    fn process_scope_strategy_names_cover_all_supported_os_families() {
        let strategies = [
            ProcessScopeStrategy::WindowsJobObject,
            ProcessScopeStrategy::UnixProcessGroup,
            ProcessScopeStrategy::DirectChildOnly,
        ];

        assert!(strategies.contains(&ProcessScopeStrategy::WindowsJobObject));
        assert!(strategies.contains(&ProcessScopeStrategy::UnixProcessGroup));
        assert!(strategies.contains(&ProcessScopeStrategy::DirectChildOnly));
    }

    #[test]
    fn scoped_spawn_enables_kill_on_drop_for_async_cancellation_paths() {
        let mut command = tokio::process::Command::new("definitely-missing-test-binary");
        assert!(!command.get_kill_on_drop());

        super::configure_scoped_spawn(&mut command);

        assert!(command.get_kill_on_drop());
    }
}
