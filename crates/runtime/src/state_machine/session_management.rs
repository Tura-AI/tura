use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use std::path::PathBuf;

use super::agent_management::AgentName;

/// UTC timestamp with millisecond precision.
///
/// `chrono::DateTime<Utc>` already stores sub-second precision. When serialized,
/// you should prefer an RFC 3339 formatter with milliseconds, for example:
/// `2026-04-08T12:34:56.789Z`.
pub type UtcDateTimeMs = DateTime<Utc>;

/// Runtime-scoped hexadecimal identifier.
pub type SessionId = String;

/// Natural-language session name.
pub type SessionName = String;

/// High-level task category for the whole session.
pub type SessionTopic = String;

/// User input text that started the task.
pub type UserInputText = String;

/// Summarized user goal extracted from the original request.
pub type UserGoal = String;

/// Free-form historical execution log entry.
pub type SessionLogEntry = String;

/// Free-form memory text recalled for a step.
pub type StepMemory = String;

/// JSON text describing the tools needed by a step.
pub type StepToolJson = String;

/// Context text needed by a step.
pub type StepContext = String;

/// Delivery target description for a step.
pub type DeliverableDescription = String;

/// Path for a deliverable produced by a step.
pub type DeliverablePath = PathBuf;

/// Describes one file passed into the session at start time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInput {
    /// File name.
    pub file_name: String,
    /// Absolute file path.
    pub file_path: PathBuf,
    /// File size in bytes.
    pub file_size_bytes: u64,
    /// Last modification time in UTC.
    pub last_modified_at: UtcDateTimeMs,
    /// Optional file description or note.
    pub description: Option<String>,
}

/// Original input payload that created the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInput {
    /// Raw user input.
    pub user_input: UserInputText,
    /// Optional files provided together with the input.
    pub file_input: Vec<FileInput>,
    /// Requested agent name for this session, when the caller selects one explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentName>,
    /// Dynamic client/runtime context captured for the current user turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_context: Option<String>,
}

/// Completion status for one task-plan item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// One executable subtask in the session plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TaskStep {
    /// Short subtask name used by compact UI and status prompts.
    #[serde(default)]
    pub task_name: String,
    /// Current completion status.
    #[serde(default)]
    pub status: TaskStatus,
    /// Compact state-machine summary visible to normal runtime turns.
    #[serde(default)]
    pub task_summary: String,
    /// Human-readable subtask description.
    #[serde(default)]
    pub step_task: String,
    /// Total turn count consumed by this step, including child processes.
    #[serde(default)]
    pub step_turn: u64,
    /// Recalled memory needed to finish the step.
    #[serde(default)]
    pub step_memory: StepMemory,
    /// Tool description as JSON text.
    #[serde(default)]
    pub step_tool: StepToolJson,
    /// Context needed for the step.
    #[serde(default)]
    pub step_context: StepContext,
    /// Agent responsible for the step.
    #[serde(default)]
    pub step_agent_name: AgentName,
    /// Description of the expected deliverable.
    #[serde(default)]
    pub step_deliverable_description: DeliverableDescription,
    /// Absolute output path of the deliverable.
    #[serde(default)]
    pub step_deliverable_path: DeliverablePath,
}

/// Session-level task plan split into compact summary and detailed task records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TaskPlan {
    /// Compact task summary visible to normal turns.
    #[serde(default)]
    pub summary: String,
    /// Detailed planning records. Only planning/delegation paths should write these.
    #[serde(default)]
    pub detailed_tasks: Vec<TaskStep>,
}

/// State machine for a session lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Session has been created but not started.
    Created,
    /// Session is actively processing work.
    Running,
    /// Session is temporarily paused.
    Paused,
    /// Session finished successfully.
    Completed,
    /// Session finished with a failure.
    Failed,
    /// Session was manually cancelled.
    Cancelled,
}

impl SessionState {
    /// Returns true if transitioning from `self` to `next` is allowed.
    pub fn can_transition_to(self, next: SessionState) -> bool {
        use SessionState::*;

        match (self, next) {
            (Created, Running | Cancelled) => true,
            (Running, Paused | Completed | Failed | Cancelled) => true,
            (Paused, Running | Cancelled | Failed) => true,
            (Completed | Failed | Cancelled, _) => false,
            _ if self == next => true,
            _ => false,
        }
    }
}

/// Root session state object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionManagement {
    /// Runtime-scoped session identifier.
    pub session_id: SessionId,
    /// Natural-language session name.
    pub session_name: SessionName,
    /// Absolute directory path of the session.
    pub session_directory: PathBuf,
    /// Whether this session uses Docker.
    pub session_uses_docker: bool,
    /// High-level session topic.
    pub session_topic: SessionTopic,
    /// Total turn count across the whole tree of the session.
    pub session_current_turn: u64,
    /// Historical execution log entries.
    pub session_log: Vec<SessionLogEntry>,
    /// Session creation timestamp in UTC.
    pub session_created_at: UtcDateTimeMs,
    /// Last activation time in UTC.
    pub session_last_update_at: UtcDateTimeMs,
    /// Current run start time in UTC.
    pub session_started_at: UtcDateTimeMs,
    /// Original input payload.
    pub input: SessionInput,
    /// Summarized overall user goal.
    pub user_goal: UserGoal,
    /// Planned subtasks.
    #[serde(default, deserialize_with = "deserialize_task_plan")]
    pub task_plan: TaskPlan,
    /// Current lifecycle state.
    pub state: SessionState,
    /// Whether runtime context should inject the previous tool response verbatim.
    #[serde(default = "default_use_last_tool_call_response")]
    pub use_last_tool_call_response: bool,
    /// Whether this session was spawned as a child/delegated session.
    #[serde(default)]
    pub is_child_session: bool,
    /// Whether command execution may bypass workspace permission restrictions.
    #[serde(default)]
    pub disable_permission_restrictions: bool,
}

fn default_use_last_tool_call_response() -> bool {
    true
}

impl SessionManagement {
    /// Creates a new session in `Created` state.
    pub fn new(
        session_id: SessionId,
        session_name: SessionName,
        session_directory: PathBuf,
        session_uses_docker: bool,
        session_topic: SessionTopic,
        input: SessionInput,
        user_goal: UserGoal,
        now: UtcDateTimeMs,
    ) -> Self {
        Self {
            session_id,
            session_name,
            session_directory,
            session_uses_docker,
            session_topic,
            session_current_turn: 0,
            session_log: Vec::new(),
            session_created_at: now,
            session_last_update_at: now,
            session_started_at: now,
            input,
            user_goal,
            task_plan: TaskPlan::default(),
            state: SessionState::Created,
            use_last_tool_call_response: true,
            is_child_session: false,
            disable_permission_restrictions: false,
        }
    }

    /// Applies a validated state transition and refreshes `session_last_update_at`.
    pub fn transition(&mut self, next: SessionState, now: UtcDateTimeMs) -> Result<(), String> {
        if !self.state.can_transition_to(next) {
            return Err(format!(
                "invalid session state transition: {:?} -> {:?}",
                self.state, next
            ));
        }

        self.state = next;
        self.session_last_update_at = now;
        Ok(())
    }

    /// Prepares an existing conversation session for a new user turn.
    ///
    /// `Completed`, `Failed`, and `Cancelled` describe the previous runtime turn,
    /// not the lifetime of the conversation. Reusing a session after switching
    /// back to it should keep its history but start the next run from `Created`.
    pub fn prepare_for_new_user_turn(&mut self, input: SessionInput, now: UtcDateTimeMs) {
        self.input = input;
        if matches!(
            self.state,
            SessionState::Completed | SessionState::Failed | SessionState::Cancelled
        ) {
            self.state = SessionState::Created;
            self.session_started_at = now;
        }
        self.session_last_update_at = now;
    }

    /// Appends a log entry and refreshes the update timestamp.
    pub fn push_log(&mut self, entry: impl Into<String>, now: UtcDateTimeMs) {
        self.session_log.push(entry.into());
        self.session_last_update_at = now;
    }

    /// Adds one planned task step.
    pub fn add_task_step(&mut self, step: TaskStep, now: UtcDateTimeMs) {
        self.task_plan.detailed_tasks.push(step);
        self.session_last_update_at = now;
    }

    /// Increments the total turn count by one.
    pub fn increment_turn(&mut self, now: UtcDateTimeMs) {
        self.session_current_turn += 1;
        self.session_last_update_at = now;
    }

    pub fn task_plan_summary_json(&self) -> serde_json::Value {
        serde_json::json!({
            "summary": self.task_plan.summary,
            "tasks": self.task_plan.detailed_tasks.iter().enumerate().map(|(index, task)| {
                serde_json::json!({
                    "index": index + 1,
                    "status": task.status,
                    "task_summary": task.task_summary,
                })
            }).collect::<Vec<_>>(),
        })
    }

    pub fn task_plan_detail_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.task_plan)
            .unwrap_or_else(|_| serde_json::json!({ "summary": "", "detailed_tasks": [] }))
    }
}

fn deserialize_task_plan<'de, D>(deserializer: D) -> Result<TaskPlan, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if value.is_array() {
        let detailed_tasks = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        return Ok(TaskPlan {
            summary: String::new(),
            detailed_tasks,
        });
    }
    serde_json::from_value(value).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::{SessionInput, SessionManagement, SessionState};
    use chrono::Utc;
    use std::path::PathBuf;

    fn session_in_state(state: SessionState) -> SessionManagement {
        let now = Utc::now();
        let mut session = SessionManagement::new(
            "sess-test".to_string(),
            "Test".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "first".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
            },
            "goal".to_string(),
            now,
        );
        session.state = state;
        session
    }

    #[test]
    fn completed_session_can_prepare_for_another_user_turn() {
        let now = Utc::now();
        let mut session = session_in_state(SessionState::Completed);

        session.prepare_for_new_user_turn(
            SessionInput {
                user_input: "second".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
            },
            now,
        );

        assert_eq!(session.state, SessionState::Created);
        assert_eq!(session.input.user_input, "second");
        assert!(session.transition(SessionState::Running, now).is_ok());
    }
}
