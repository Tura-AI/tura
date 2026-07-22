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
    ContextSlice, GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest, Page,
    ReadContextSliceRequest, SessionContextRecord, SessionRecord, SessionSnapshot, SessionSummary,
    WorkspaceSummary,
};
use std::path::Path;

impl SessionLogStore {
    pub fn read_context_slice(&self, request: ReadContextSliceRequest) -> Result<ContextSlice> {
        if request.max_estimated_tokens == 0 {
            anyhow::bail!("context token budget must be greater than zero");
        }
        let workspace_db_path = self
            .workspace_db_path_for_session(&request.session_id)?
            .ok_or_else(|| anyhow::anyhow!("session {} not found", request.session_id))?;
        self.with_workspace_connection(&workspace_db_path, |conn| {
            let (next_sequence, retained_from_sequence, next_management_sequence) = conn
                .query_row(
                    "SELECT next_context_sequence, retained_from_sequence, next_management_sequence
                 FROM sessions WHERE session_id = ?1",
                    params![request.session_id],
                    |row| {
                        Ok((
                            row.get::<_, u64>(0)?,
                            row.get::<_, u64>(1)?,
                            row.get::<_, u64>(2)?,
                        ))
                    },
                )?;
            let mut statement = conn.prepare(
                "SELECT sequence, record_json FROM session_context_records
                 WHERE session_id = ?1 AND sequence >= ?2 AND sequence < ?3
                 ORDER BY sequence DESC",
            )?;
            let byte_budget = request.max_estimated_tokens.saturating_mul(4);
            let mut rows = statement.query(params![
                request.session_id,
                retained_from_sequence,
                next_sequence
            ])?;
            let mut selected_bytes = 0_u64;
            let mut records = Vec::new();
            while let Some(row) = rows.next()? {
                let raw_record = row.get::<_, String>(1)?;
                let record_bytes = raw_record.len() as u64;
                if !records.is_empty() && selected_bytes.saturating_add(record_bytes) > byte_budget
                {
                    break;
                }
                selected_bytes = selected_bytes.saturating_add(record_bytes);
                records.push(SessionContextRecord {
                    sequence: row.get(0)?,
                    raw_record,
                });
            }
            records.reverse();
            Ok(ContextSlice {
                records,
                retained_from_sequence,
                next_sequence,
                next_management_sequence,
            })
        })
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
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
        self.get_session_canonical(&request.session_id)
    }

    pub(super) fn get_session_canonical(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionSnapshot>> {
        let row = self.with_index_connection(|conn| {
            conn.query_row(
                "SELECT session_id, workspace_db_path
                 FROM sessions
                 WHERE session_id = ?1",
                params![session_id],
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
        let snapshot = SessionSnapshot {
            session_id: row.session_id,
            workspace: workspace_payload.workspace,
            name: workspace_payload.name,
            created_at: workspace_payload.created_at,
            updated_at: workspace_payload.updated_at,
            last_user_message_at: workspace_payload.last_user_message_at,
            message_count: workspace_payload.message_count as u64,
            lifecycle_projection: workspace_payload.lifecycle_projection,
            management: workspace_payload.management,
            metadata: workspace_payload.metadata,
            todos: workspace_payload.todos,
        };
        snapshot.validate().map_err(anyhow::Error::msg)?;
        Ok(Some(snapshot))
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
}
