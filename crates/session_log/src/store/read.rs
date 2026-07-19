use super::connection::{init_workspace_db, with_connection};
use super::helpers::{bounded_page, parse_json_field};
use super::payload::{
    index_session_from_row, load_workspace_session_payload, load_workspace_session_summary_payload,
    IndexSessionRow,
};
use super::SessionLogStore;
use crate::path::normalize_workspace;
use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use session_log_contract::{
    GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest, Page, SessionRecord,
    SessionSnapshot, SessionSummary, WorkspaceSummary,
};
use std::path::Path;
use std::time::Duration;

const STALE_RUNNING_SESSION_TIMEOUT: Duration = Duration::from_secs(120);

impl SessionLogStore {
    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        self.sweep_missing_workspace_dbs()?;
        self.mark_stale_running_sessions_interrupted(STALE_RUNNING_SESSION_TIMEOUT)?;
        self.with_index_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT workspace, COUNT(*), COALESCE(MAX(updated_at), 0)
                 FROM sessions
                 WHERE workspace != ''
                 GROUP BY workspace
                 ORDER BY MAX(updated_at) DESC, workspace ASC",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(WorkspaceSummary {
                        directory: row.get(0)?,
                        session_count: row.get::<_, i64>(1)? as u64,
                        last_updated_at: row.get(2)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    pub fn list_sessions(
        &self,
        request: ListSessionsRequest,
    ) -> Result<(Page, Vec<SessionSnapshot>)> {
        self.sweep_missing_workspace_dbs()?;
        self.mark_stale_running_sessions_interrupted(STALE_RUNNING_SESSION_TIMEOUT)?;
        let workspace = normalize_workspace(&request.workspace);
        let page_size = request.page_size.clamp(1, 500);
        let (page, total, index_rows) = self.with_index_connection(|conn| {
            let total = conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE workspace = ?1",
                params![workspace],
                |row| row.get::<_, i64>(0),
            )? as u64;
            let page = bounded_page(request.page, page_size, total, false);
            let mut stmt = conn.prepare(
                "SELECT session_id, workspace_db_path
                 FROM sessions
                 WHERE workspace = ?1
                 ORDER BY last_user_message_at DESC, session_id ASC
                 LIMIT ?2 OFFSET ?3",
            )?;
            let index_rows = stmt
                .query_map(
                    params![workspace, page_size as i64, (page * page_size) as i64],
                    index_session_from_row,
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok((page, total, index_rows))
        })?;
        let sessions = index_rows
            .into_iter()
            .filter_map(|row| match self.snapshot_from_index_row(row) {
                Ok(Some(snapshot)) => Some(Ok(snapshot)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((
            Page {
                page,
                page_size,
                total,
            },
            sessions,
        ))
    }

    pub fn list_session_summaries(
        &self,
        request: ListSessionsRequest,
    ) -> Result<(Page, Vec<SessionSummary>)> {
        self.sweep_missing_workspace_dbs()?;
        self.mark_stale_running_sessions_interrupted(STALE_RUNNING_SESSION_TIMEOUT)?;
        let workspace = normalize_workspace(&request.workspace);
        let page_size = request.page_size.clamp(1, 500);
        let (page, total, index_rows) = self.with_index_connection(|conn| {
            let total = conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE workspace = ?1",
                params![workspace],
                |row| row.get::<_, i64>(0),
            )? as u64;
            let page = bounded_page(request.page, page_size, total, false);
            let mut stmt = conn.prepare(
                "SELECT session_id, workspace_db_path
                 FROM sessions
                 WHERE workspace = ?1
                 ORDER BY last_user_message_at DESC, session_id ASC
                 LIMIT ?2 OFFSET ?3",
            )?;
            let index_rows = stmt
                .query_map(
                    params![workspace, page_size as i64, (page * page_size) as i64],
                    index_session_from_row,
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok((page, total, index_rows))
        })?;
        let sessions = index_rows
            .into_iter()
            .filter_map(|row| match self.summary_from_index_row(row) {
                Ok(Some(snapshot)) => Some(Ok(snapshot)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((
            Page {
                page,
                page_size,
                total,
            },
            sessions,
        ))
    }

    pub fn get_session(&self, request: GetSessionRequest) -> Result<Option<SessionSnapshot>> {
        self.mark_stale_running_sessions_interrupted(STALE_RUNNING_SESSION_TIMEOUT)?;
        let row = self.with_index_connection(|conn| {
            conn.query_row(
                "SELECT session_id, workspace_db_path
                 FROM sessions
                 WHERE session_id = ?1",
                params![request.session_id],
                index_session_from_row,
            )
            .optional()
            .map_err(Into::into)
        })?;
        row.map(|row| self.snapshot_from_index_row(row))
            .transpose()
            .map(Option::flatten)
    }

    pub fn list_session_records(
        &self,
        request: ListSessionRecordsRequest,
    ) -> Result<(Page, Vec<SessionRecord>)> {
        self.mark_stale_running_sessions_interrupted(STALE_RUNNING_SESSION_TIMEOUT)?;
        let workspace_db_path = self.with_index_connection(|conn| {
            conn.query_row(
                "SELECT workspace_db_path FROM sessions WHERE session_id = ?1",
                params![request.session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })?;
        let Some(workspace_db_path) = workspace_db_path else {
            return Ok((Page::default(), Vec::new()));
        };
        if !Path::new(&workspace_db_path).exists() {
            self.delete_index_session(&request.session_id)?;
            return Ok((Page::default(), Vec::new()));
        }
        let page_size = request.page_size.clamp(1, 500);
        with_connection(Path::new(&workspace_db_path), init_workspace_db, |conn| {
            let total = conn.query_row(
                "SELECT COUNT(*) FROM session_records WHERE session_id = ?1",
                params![request.session_id],
                |row| row.get::<_, i64>(0),
            )? as u64;
            let page = bounded_page(request.page, page_size, total, true);
            let mut stmt = conn.prepare(
                "SELECT session_id, message_id, role, created_at, updated_at, record_json
                 FROM session_records
                 WHERE session_id = ?1
                 ORDER BY created_at ASC, id ASC
                 LIMIT ?2 OFFSET ?3",
            )?;
            let rows = stmt
                .query_map(
                    params![
                        request.session_id,
                        page_size as i64,
                        (page * page_size) as i64
                    ],
                    |row| {
                        let record_json: String = row.get(5)?;
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                            record_json,
                        ))
                    },
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let records = rows
                .into_iter()
                .map(
                    |(session_id, message_id, role, created_at, updated_at, record_json)| {
                        Ok(SessionRecord {
                            record: parse_json_field(
                                &record_json,
                                "record_json",
                                Some(&session_id),
                            )?,
                            session_id,
                            message_id,
                            role,
                            created_at,
                            updated_at,
                        })
                    },
                )
                .collect::<Result<Vec<_>>>()?;
            Ok((
                Page {
                    page,
                    page_size,
                    total,
                },
                records,
            ))
        })
    }

    fn snapshot_from_index_row(&self, row: IndexSessionRow) -> Result<Option<SessionSnapshot>> {
        let workspace_payload =
            load_workspace_session_payload(&row.workspace_db_path, &row.session_id)?;
        let Some(workspace_payload) = workspace_payload else {
            self.delete_index_session(&row.session_id)?;
            return Ok(None);
        };
        Ok(Some(SessionSnapshot {
            session_id: row.session_id,
            workspace: workspace_payload.workspace,
            name: workspace_payload.name,
            parent_id: workspace_payload.parent_id,
            created_at: workspace_payload.created_at,
            updated_at: workspace_payload.updated_at,
            last_user_message_at: workspace_payload.last_user_message_at,
            state: workspace_payload.state,
            status: workspace_payload.status,
            message_count: workspace_payload.message_count as u64,
            task_management: workspace_payload.task_management,
            management: workspace_payload.management,
            session: workspace_payload.session,
            todos: workspace_payload.todos,
        }))
    }

    fn summary_from_index_row(&self, row: IndexSessionRow) -> Result<Option<SessionSummary>> {
        let workspace_payload =
            load_workspace_session_summary_payload(&row.workspace_db_path, &row.session_id)?;
        let Some(workspace_payload) = workspace_payload else {
            self.delete_index_session(&row.session_id)?;
            return Ok(None);
        };
        Ok(Some(SessionSummary {
            session_id: row.session_id,
            workspace: workspace_payload.workspace,
            name: workspace_payload.name,
            parent_id: workspace_payload.parent_id,
            created_at: workspace_payload.created_at,
            updated_at: workspace_payload.updated_at,
            last_user_message_at: workspace_payload.last_user_message_at,
            state: workspace_payload.state,
            status: workspace_payload.status,
            message_count: workspace_payload.message_count as u64,
            task_management: workspace_payload.task_management,
        }))
    }

    pub(super) fn delete_index_session(&self, session_id: &str) -> Result<()> {
        self.with_index_connection(|conn| {
            conn.execute(
                "DELETE FROM sessions WHERE session_id = ?1",
                params![session_id],
            )?;
            Ok(())
        })
    }

    fn sweep_missing_workspace_dbs(&self) -> Result<()> {
        let missing = self.with_index_connection(|conn| {
            let mut stmt = conn.prepare("SELECT DISTINCT workspace_db_path FROM sessions")?;
            let paths = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|path| !Path::new(path).exists())
                .collect::<Vec<_>>();
            for path in &paths {
                conn.execute(
                    "DELETE FROM sessions WHERE workspace_db_path = ?1",
                    params![path],
                )?;
            }
            Ok(paths)
        })?;
        for path in missing {
            tracing::warn!(
                path,
                "removed session index snapshots for missing workspace DB"
            );
        }
        Ok(())
    }
}
