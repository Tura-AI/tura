use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Condvar, Mutex, OnceLock};

pub const POLICY: &str = include_str!("policy.toml");

const WORKSPACE_LOCK: &str = ".";

static FILE_LOCKS: OnceLock<FileLockManager> = OnceLock::new();

#[derive(Clone, Debug, Default)]
pub struct Access {
    pub read_paths: Vec<String>,
    pub write_paths: Vec<String>,
    pub workspace_write: bool,
}

impl Access {
    pub fn is_read_only(&self) -> bool {
        self.write_paths.is_empty() && !self.workspace_write
    }

    fn lock_requests(&self) -> Vec<(String, LockMode)> {
        if self.workspace_write {
            return vec![(WORKSPACE_LOCK.to_string(), LockMode::Write)];
        }
        let mut requests = Vec::new();
        requests.extend(
            self.read_paths
                .iter()
                .cloned()
                .map(|path| (path, LockMode::Read)),
        );
        requests.extend(
            self.write_paths
                .iter()
                .cloned()
                .map(|path| (path, LockMode::Write)),
        );
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
        let mut state = self.state.lock().expect("file lock state poisoned");
        while !can_acquire(&state, key, mode) {
            state = self.condvar.wait(state).expect("file lock wait poisoned");
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
        let mut state = self.state.lock().expect("file lock state poisoned");
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
    if key == WORKSPACE_LOCK && mode == LockMode::Write {
        return state.readers.is_empty() && state.writers.is_empty();
    }
    if state.writers.contains(WORKSPACE_LOCK) || state.writers.contains(key) {
        return false;
    }
    if mode == LockMode::Read {
        return true;
    }
    state.readers.get(WORKSPACE_LOCK).copied().unwrap_or(0) == 0
        && state.readers.get(key).copied().unwrap_or(0) == 0
}
