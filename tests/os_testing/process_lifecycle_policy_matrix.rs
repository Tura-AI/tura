//! Required local business coverage for process-lifecycle policy decisions.
//!
//! The behavior tests in this directory exercise the current host. This matrix
//! pins the cross-OS contract so Windows, Linux, macOS, and fallback ports do
//! not silently drift apart as process management changes.

use code_tools::commands::shell_command::{
    current_shell_process_scope_strategy, ShellProcessScopeStrategy,
};
use tura_router::process_scope::{current_process_scope_strategy, ProcessScopeStrategy};

#[test]
fn lifecycle_policy_matrix_covers_all_supported_os_and_process_roles() {
    let rows = lifecycle_matrix();
    assert_eq!(
        rows.len(),
        SimulatedOs::ALL.len() * ProcessRole::ALL.len(),
        "every OS/role pair must have an explicit lifecycle policy row"
    );

    for os in SimulatedOs::ALL {
        let os_rows = rows.iter().filter(|row| row.os == os).collect::<Vec<_>>();
        assert_eq!(
            os_rows.len(),
            ProcessRole::ALL.len(),
            "{os:?} should define every process role"
        );
    }

    for role in ProcessRole::ALL {
        let role_rows = rows
            .iter()
            .filter(|row| row.role == role)
            .collect::<Vec<_>>();
        assert_eq!(
            role_rows.len(),
            SimulatedOs::ALL.len(),
            "{role:?} should define every OS family"
        );
    }
}

#[test]
fn gateway_is_the_reusable_owner_and_backends_are_gateway_tree_children() {
    for row in lifecycle_matrix().into_iter().filter(|row| {
        matches!(
            row.role,
            ProcessRole::GatewayFront | ProcessRole::RouterDaemon | ProcessRole::SessionDbOwner
        )
    }) {
        assert_eq!(
            row.can_be_adopted_by_next_owner,
            row.role == ProcessRole::GatewayFront
        );
        assert!(
            row.explicit_shutdown_required,
            "{:?}/{:?} should not rely on launcher-death cleanup",
            row.os, row.role
        );
        assert_eq!(
            row.parent_crash_outlives_parent,
            row.role == ProcessRole::GatewayFront
        );
    }
}

#[test]
fn gateway_front_is_persistent_tray_owner_not_gui_or_tui_child() {
    for row in lifecycle_matrix()
        .into_iter()
        .filter(|row| row.role == ProcessRole::GatewayFront)
    {
        assert_eq!(row.scope, ScopePrimitive::DetachedLeaseOwner);
        assert!(row.parent_crash_outlives_parent);
        assert!(row.can_be_adopted_by_next_owner);
        assert!(row.explicit_shutdown_required);
        assert!(
            row.process_tree_cleanup,
            "gateway quit/process-close must explicitly shut down router/session_db/runtime-owned work"
        );
    }
}

#[test]
fn runtime_workers_and_command_runs_have_tree_cleanup_on_supported_desktop_os() {
    for row in lifecycle_matrix().into_iter().filter(|row| {
        matches!(
            row.role,
            ProcessRole::RuntimeWorker | ProcessRole::CommandRun
        )
    }) {
        match row.os {
            SimulatedOs::Windows | SimulatedOs::Linux | SimulatedOs::MacOs => {
                assert!(
                    row.process_tree_cleanup,
                    "{:?}/{:?} must clean the whole child tree",
                    row.os, row.role
                );
                assert!(
                    !row.direct_child_only,
                    "{:?}/{:?} must not regress to direct-child-only kill",
                    row.os, row.role
                );
            }
            SimulatedOs::Other => {
                assert!(
                    row.direct_child_only,
                    "fallback OS should be marked as direct-child-only"
                );
            }
        }
    }
}

#[test]
fn macos_contract_records_no_pdeathsig_and_requires_router_owned_cleanup() {
    let macos_rows = lifecycle_matrix()
        .into_iter()
        .filter(|row| row.os == SimulatedOs::MacOs)
        .collect::<Vec<_>>();
    assert_eq!(macos_rows.len(), ProcessRole::ALL.len());

    for row in macos_rows.into_iter().filter(|row| {
        matches!(
            row.role,
            ProcessRole::RuntimeWorker | ProcessRole::CommandRun
        )
    }) {
        assert!(!row.parent_death_signal);
        assert_eq!(row.scope, ScopePrimitive::UnixProcessGroup);
        assert!(
            row.explicit_shutdown_required,
            "macOS requires router/gateway explicit shutdown because it has no PR_SET_PDEATHSIG"
        );
    }
}

#[test]
fn current_host_scope_strategies_match_the_policy_matrix() {
    let current_os = current_simulated_os();
    let worker_policy = lifecycle_matrix()
        .into_iter()
        .find(|row| row.os == current_os && row.role == ProcessRole::RuntimeWorker)
        .expect("current worker policy row");
    let command_policy = lifecycle_matrix()
        .into_iter()
        .find(|row| row.os == current_os && row.role == ProcessRole::CommandRun)
        .expect("current command_run policy row");

    assert_eq!(
        process_scope_strategy_for(worker_policy.scope),
        current_process_scope_strategy()
    );
    assert_eq!(
        shell_scope_strategy_for(command_policy.scope),
        current_shell_process_scope_strategy()
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimulatedOs {
    Windows,
    Linux,
    MacOs,
    Other,
}

impl SimulatedOs {
    const ALL: [Self; 4] = [Self::Windows, Self::Linux, Self::MacOs, Self::Other];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessRole {
    GatewayFront,
    RouterDaemon,
    SessionDbOwner,
    RuntimeWorker,
    CommandRun,
}

impl ProcessRole {
    const ALL: [Self; 5] = [
        Self::GatewayFront,
        Self::RouterDaemon,
        Self::SessionDbOwner,
        Self::RuntimeWorker,
        Self::CommandRun,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopePrimitive {
    DetachedLeaseOwner,
    GatewayProcessTreeChild,
    WindowsJobObject,
    UnixProcessGroup,
    DirectChildOnly,
}

#[derive(Debug, Clone, Copy)]
struct LifecyclePolicy {
    os: SimulatedOs,
    role: ProcessRole,
    scope: ScopePrimitive,
    process_tree_cleanup: bool,
    parent_death_signal: bool,
    parent_crash_outlives_parent: bool,
    can_be_adopted_by_next_owner: bool,
    explicit_shutdown_required: bool,
    direct_child_only: bool,
}

fn lifecycle_matrix() -> Vec<LifecyclePolicy> {
    let mut rows = Vec::new();
    for os in SimulatedOs::ALL {
        rows.push(front_policy(os));
        rows.push(router_policy(os));
        rows.push(session_db_policy(os));
        rows.push(runtime_worker_policy(os));
        rows.push(command_run_policy(os));
    }
    rows
}

fn front_policy(os: SimulatedOs) -> LifecyclePolicy {
    LifecyclePolicy {
        os,
        role: ProcessRole::GatewayFront,
        scope: ScopePrimitive::DetachedLeaseOwner,
        process_tree_cleanup: true,
        parent_death_signal: false,
        parent_crash_outlives_parent: true,
        can_be_adopted_by_next_owner: true,
        explicit_shutdown_required: true,
        direct_child_only: false,
    }
}

fn router_policy(os: SimulatedOs) -> LifecyclePolicy {
    let scope = backend_child_primitive(os);
    LifecyclePolicy {
        os,
        role: ProcessRole::RouterDaemon,
        scope,
        process_tree_cleanup: true,
        parent_death_signal: false,
        parent_crash_outlives_parent: false,
        can_be_adopted_by_next_owner: false,
        explicit_shutdown_required: true,
        direct_child_only: false,
    }
}

fn session_db_policy(os: SimulatedOs) -> LifecyclePolicy {
    let scope = backend_child_primitive(os);
    LifecyclePolicy {
        os,
        role: ProcessRole::SessionDbOwner,
        scope,
        process_tree_cleanup: true,
        parent_death_signal: false,
        parent_crash_outlives_parent: false,
        can_be_adopted_by_next_owner: false,
        explicit_shutdown_required: true,
        direct_child_only: matches!(scope, ScopePrimitive::DirectChildOnly),
    }
}

fn backend_child_primitive(os: SimulatedOs) -> ScopePrimitive {
    match os {
        SimulatedOs::Windows | SimulatedOs::Linux | SimulatedOs::MacOs => {
            ScopePrimitive::GatewayProcessTreeChild
        }
        SimulatedOs::Other => ScopePrimitive::DirectChildOnly,
    }
}

fn runtime_worker_policy(os: SimulatedOs) -> LifecyclePolicy {
    let scope = scoped_child_primitive(os);
    LifecyclePolicy {
        os,
        role: ProcessRole::RuntimeWorker,
        scope,
        process_tree_cleanup: !matches!(scope, ScopePrimitive::DirectChildOnly),
        parent_death_signal: matches!(os, SimulatedOs::Linux),
        parent_crash_outlives_parent: matches!(os, SimulatedOs::MacOs | SimulatedOs::Other),
        can_be_adopted_by_next_owner: false,
        explicit_shutdown_required: !matches!(os, SimulatedOs::Windows | SimulatedOs::Linux),
        direct_child_only: matches!(scope, ScopePrimitive::DirectChildOnly),
    }
}

fn command_run_policy(os: SimulatedOs) -> LifecyclePolicy {
    let scope = scoped_child_primitive(os);
    LifecyclePolicy {
        os,
        role: ProcessRole::CommandRun,
        scope,
        process_tree_cleanup: !matches!(scope, ScopePrimitive::DirectChildOnly),
        parent_death_signal: matches!(os, SimulatedOs::Linux),
        parent_crash_outlives_parent: matches!(os, SimulatedOs::MacOs | SimulatedOs::Other),
        can_be_adopted_by_next_owner: false,
        explicit_shutdown_required: !matches!(os, SimulatedOs::Windows | SimulatedOs::Linux),
        direct_child_only: matches!(scope, ScopePrimitive::DirectChildOnly),
    }
}

fn scoped_child_primitive(os: SimulatedOs) -> ScopePrimitive {
    match os {
        SimulatedOs::Windows => ScopePrimitive::WindowsJobObject,
        SimulatedOs::Linux | SimulatedOs::MacOs => ScopePrimitive::UnixProcessGroup,
        SimulatedOs::Other => ScopePrimitive::DirectChildOnly,
    }
}

fn current_simulated_os() -> SimulatedOs {
    if cfg!(windows) {
        SimulatedOs::Windows
    } else if cfg!(target_os = "linux") {
        SimulatedOs::Linux
    } else if cfg!(target_os = "macos") {
        SimulatedOs::MacOs
    } else {
        SimulatedOs::Other
    }
}

fn process_scope_strategy_for(scope: ScopePrimitive) -> ProcessScopeStrategy {
    match scope {
        ScopePrimitive::WindowsJobObject => ProcessScopeStrategy::WindowsJobObject,
        ScopePrimitive::UnixProcessGroup => ProcessScopeStrategy::UnixProcessGroup,
        ScopePrimitive::DirectChildOnly => ProcessScopeStrategy::DirectChildOnly,
        ScopePrimitive::DetachedLeaseOwner | ScopePrimitive::GatewayProcessTreeChild => {
            panic!("front/router/session_db scopes are not worker process strategies")
        }
    }
}

fn shell_scope_strategy_for(scope: ScopePrimitive) -> ShellProcessScopeStrategy {
    match scope {
        ScopePrimitive::WindowsJobObject => ShellProcessScopeStrategy::WindowsJobObject,
        ScopePrimitive::UnixProcessGroup => ShellProcessScopeStrategy::UnixProcessGroup,
        ScopePrimitive::DirectChildOnly => ShellProcessScopeStrategy::DirectChildOnly,
        ScopePrimitive::DetachedLeaseOwner | ScopePrimitive::GatewayProcessTreeChild => {
            panic!("front/router/session_db scopes are not command_run strategies")
        }
    }
}
