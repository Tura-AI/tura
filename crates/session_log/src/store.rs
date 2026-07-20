use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

mod connection;
mod helpers;
mod payload;
mod queue;
mod read;
mod session_commands;
mod write;

const INDEX_DB_FILE: &str = "index.sqlite3";

/// SQLite-backed session log entry point.
///
/// The public store API lives on this type. Implementation details are split
/// by responsibility under `store/`: queue replay, writes, reads, workspace
/// payload loading, shared helpers, and SQLite connection/schema handling.
#[derive(Clone)]
pub struct SessionLogStore {
    data_dir: PathBuf,
    index_db_path: PathBuf,
}

impl SessionLogStore {
    pub fn open_default() -> Result<Self> {
        Self::open(crate::path::default_db_dir())
    }

    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("failed to create db directory {}", data_dir.display()))?;
        let store = Self {
            index_db_path: data_dir.join(INDEX_DB_FILE),
            data_dir,
        };
        store.with_index_connection(|conn| connection::init_index_db(conn))?;
        Ok(store)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

#[cfg(test)]
mod tests;
