use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Condvar, Mutex, OnceLock};

pub const POLICY: &str = include_str!("policy.toml");

const DEFAULT_LOCK_SCOPE: &str = "";
const LOCK_SCOPE_SEPARATOR: char = '\u{1f}';
const WORKSPACE_LOCK_PATH: &str = ".";

static FILE_LOCKS: OnceLock<FileLockManager> = OnceLock::new();

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Access {
    pub read_paths: Vec<String>,
    pub write_paths: Vec<String>,
    pub workspace_write: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_scope: Option<String>,
}

impl Access {
    pub fn is_read_only(&self) -> bool {
        self.write_paths.is_empty() && !self.workspace_write
    }

    fn lock_requests(&self) -> Vec<(String, LockMode)> {
        let scope = self
            .lock_scope
            .as_deref()
            .map(str::trim)
            .filter(|scope| !scope.is_empty());
        if self.workspace_write {
            return vec![(lock_key(scope, WORKSPACE_LOCK_PATH), LockMode::Write)];
        }
        let mut requests = Vec::new();
        requests.extend(self.read_paths.iter().map(|path| {
            (
                lock_key(scope, path.trim().trim_end_matches(['/', '\\'])),
                LockMode::Read,
            )
        }));
        requests.extend(self.write_paths.iter().map(|path| {
            (
                lock_key(scope, path.trim().trim_end_matches(['/', '\\'])),
                LockMode::Write,
            )
        }));
        requests
    }
}

pub fn acquire(access: &Access) -> LockGuard<'static> {
    FILE_LOCKS.get_or_init(FileLockManager::new).acquire(access)
}

#[derive(Default)]
struct LockState {
    readers: BTreeMap<String, usize>,
    writers: BTreeSet<String>,
}

struct FileLockManager {
    state: Mutex<LockState>,
    condvar: Condvar,
}

impl FileLockManager {
    fn new() -> Self {
        Self {
            state: Mutex::new(LockState::default()),
            condvar: Condvar::new(),
        }
    }

    fn acquire(&self, access: &Access) -> LockGuard<'_> {
        let mut locks = access.lock_requests();
        locks.sort();
        let mut acquired = Vec::new();
        for (key, mode) in locks {
            self.acquire_one(&key, mode);
            acquired.push((key, mode));
        }
        LockGuard {
            manager: self,
            acquired,
        }
    }

    fn acquire_one(&self, key: &str, mode: LockMode) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        while !can_acquire(&state, key, mode) {
            state = self
                .condvar
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
        match mode {
            LockMode::Read => {
                *state.readers.entry(key.to_string()).or_insert(0) += 1;
            }
            LockMode::Write => {
                state.writers.insert(key.to_string());
            }
        }
    }

    fn release_one(&self, key: &str, mode: LockMode) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        match mode {
            LockMode::Read => {
                let count = state
                    .readers
                    .get(key)
                    .copied()
                    .unwrap_or(0)
                    .saturating_sub(1);
                if count == 0 {
                    state.readers.remove(key);
                } else {
                    state.readers.insert(key.to_string(), count);
                }
            }
            LockMode::Write => {
                state.writers.remove(key);
            }
        }
        self.condvar.notify_all();
    }
}

pub struct LockGuard<'a> {
    manager: &'a FileLockManager,
    acquired: Vec<(String, LockMode)>,
}

impl Drop for LockGuard<'_> {
    fn drop(&mut self) {
        for (key, mode) in self.acquired.iter().rev() {
            self.manager.release_one(key, *mode);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum LockMode {
    Read,
    Write,
}

fn can_acquire(state: &LockState, key: &str, mode: LockMode) -> bool {
    if is_workspace_lock(key) && mode == LockMode::Write {
        return state
            .readers
            .keys()
            .all(|reader| !same_lock_scope(reader, key))
            && state
                .writers
                .iter()
                .all(|writer| !same_lock_scope(writer, key));
    }
    if state.writers.contains(key) || has_workspace_writer_in_scope(state, key) {
        return false;
    }
    if mode == LockMode::Read {
        return true;
    }
    state.readers.get(key).copied().unwrap_or(0) == 0
        && state
            .readers
            .keys()
            .all(|reader| !is_workspace_lock(reader) || !same_lock_scope(reader, key))
}

fn lock_key(scope: Option<&str>, path: &str) -> String {
    match scope {
        Some(scope) => format!("{scope}{LOCK_SCOPE_SEPARATOR}{path}"),
        None => path.to_string(),
    }
}

fn has_workspace_writer_in_scope(state: &LockState, key: &str) -> bool {
    state
        .writers
        .iter()
        .any(|writer| is_workspace_lock(writer) && same_lock_scope(writer, key))
}

fn same_lock_scope(left: &str, right: &str) -> bool {
    lock_scope(left) == lock_scope(right)
}

fn is_workspace_lock(key: &str) -> bool {
    lock_path(key) == WORKSPACE_LOCK_PATH
}

fn lock_scope(key: &str) -> &str {
    key.split_once(LOCK_SCOPE_SEPARATOR)
        .map(|(scope, _)| scope)
        .unwrap_or(DEFAULT_LOCK_SCOPE)
}

fn lock_path(key: &str) -> &str {
    key.split_once(LOCK_SCOPE_SEPARATOR)
        .map(|(_, path)| path)
        .unwrap_or(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn workspace_write_blocks_path_locks_until_released() {
        let manager = Arc::new(FileLockManager::new());
        let workspace_guard = manager.acquire(&Access {
            workspace_write: true,
            ..Access::default()
        });
        let acquired = Arc::new(AtomicBool::new(false));
        let worker_manager = Arc::clone(&manager);
        let worker_acquired = Arc::clone(&acquired);

        let worker = std::thread::spawn(move || {
            let _guard = worker_manager.acquire(&Access {
                write_paths: vec!["src/lib.rs".to_string()],
                ..Access::default()
            });
            worker_acquired.store(true, Ordering::SeqCst);
        });

        std::thread::sleep(Duration::from_millis(50));
        assert!(!acquired.load(Ordering::SeqCst));
        drop(workspace_guard);
        worker.join().expect("worker should acquire after release");
        assert!(acquired.load(Ordering::SeqCst));
    }

    #[test]
    fn workspace_write_only_blocks_same_lock_scope() {
        let manager = FileLockManager::new();
        let _first_scope_guard = manager.acquire(&Access {
            workspace_write: true,
            lock_scope: Some("session-a".to_string()),
            ..Access::default()
        });

        let _second_scope_guard = manager.acquire(&Access {
            write_paths: vec!["src/lib.rs".to_string()],
            lock_scope: Some("session-b".to_string()),
            ..Access::default()
        });
    }

    #[test]
    fn workspace_write_blocks_path_locks_inside_same_lock_scope() {
        let manager = Arc::new(FileLockManager::new());
        let workspace_guard = manager.acquire(&Access {
            workspace_write: true,
            lock_scope: Some("session-a".to_string()),
            ..Access::default()
        });
        let acquired = Arc::new(AtomicBool::new(false));
        let worker_manager = Arc::clone(&manager);
        let worker_acquired = Arc::clone(&acquired);

        let worker = std::thread::spawn(move || {
            let _guard = worker_manager.acquire(&Access {
                write_paths: vec!["src/lib.rs".to_string()],
                lock_scope: Some("session-a".to_string()),
                ..Access::default()
            });
            worker_acquired.store(true, Ordering::SeqCst);
        });

        std::thread::sleep(Duration::from_millis(50));
        assert!(!acquired.load(Ordering::SeqCst));
        drop(workspace_guard);
        worker.join().expect("worker should acquire after release");
        assert!(acquired.load(Ordering::SeqCst));
    }
}
