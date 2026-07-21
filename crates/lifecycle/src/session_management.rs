use chrono::{DateTime, Utc};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

/// UTC timestamp with millisecond precision.
///
/// `chrono::DateTime<Utc>` already stores sub-second precision. When serialized,
/// you should prefer an RFC 3339 formatter with milliseconds, for example:
/// `2026-04-08T12:34:56.789Z`.
pub type UtcDateTimeMs = DateTime<Utc>;

use crate::{
    ContextTokenStats, PlanStatus, SessionAggregate, SessionCommand, SessionId, SessionProjection,
    SessionQuery, SessionState, TaskPlan, TaskStep,
};

pub type AgentName = String;

/// Natural-language session name.
pub type SessionName = String;

/// Runtime prompt manual task categories active for the whole session.
pub type SessionTaskType = Vec<String>;

/// Command capabilities loaded into the active session context.
pub type SessionCapabilities = Vec<String>;

/// User input text that started the task.
pub type UserInputText = String;

/// Summarized user goal extracted from the original request.
pub type UserGoal = String;

/// Free-form execution log entry.
pub type SessionLogEntry = String;

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
    /// Optional per-run planning override. None means keep the selected agent's
    /// configured capabilities; Some(true) adds planning, Some(false) removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planning_mode_override: Option<bool>,
}

pub type TaskStatus = PlanStatus;

pub const SESSION_CONTEXT_TOKEN_LIMIT: u64 = crate::DEFAULT_CONTEXT_TOKEN_LIMIT;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionLogCompactionPoint {
    /// Absolute session_log index of the compact record.
    #[serde(default)]
    pub compact_entry_index: u64,
    /// Number of entries omitted before the retained session_log slice.
    #[serde(default)]
    pub retained_before: u64,
    /// Absolute index that became the start of the retained slice.
    #[serde(default)]
    pub retained_from_index: u64,
    /// Compact timestamp.
    pub compacted_at: UtcDateTimeMs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionLogRetention {
    /// Number of historical session_log entries omitted from this in-memory state.
    #[serde(default)]
    pub omitted_entries: u64,
    /// Boundary recorded when the latest context compaction trimmed history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_compaction: Option<SessionLogCompactionPoint>,
}

/// Root session state object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionManagement {
    /// Canonical session identity and lifecycle state.
    lifecycle: SessionAggregate,
    /// Natural-language session name.
    pub session_name: SessionName,
    /// Whether the session name should follow the latest task summary.
    pub auto_session_name: bool,
    /// Absolute directory path of the session.
    pub session_directory: PathBuf,
    /// Whether this session uses Docker.
    pub session_uses_docker: bool,
    /// Runtime prompt manual task types active for this session.
    pub task_type: SessionTaskType,
    /// Command capabilities already loaded into the active session context.
    pub session_capabilities: SessionCapabilities,
    /// Total turn count across the whole tree of the session.
    pub session_current_turn: u64,
    /// Historical execution log entries.
    pub session_log: Vec<SessionLogEntry>,
    /// Retention state for compacted session_log history.
    pub session_log_retention: SessionLogRetention,
    /// Session creation timestamp in UTC.
    pub session_created_at: UtcDateTimeMs,
    /// Last activation time in UTC.
    pub session_last_update_at: UtcDateTimeMs,
    /// Last user message received by this session in UTC.
    pub session_last_user_message_at: UtcDateTimeMs,
    /// Current run start time in UTC.
    pub session_started_at: UtcDateTimeMs,
    /// Original input payload.
    pub input: SessionInput,
    /// Summarized overall user goal.
    pub user_goal: UserGoal,
    /// Current objective used for planning completion-audit reminders.
    pub current_objective: String,
    /// Whether runtime context should inject the previous tool response verbatim.
    pub use_last_tool_call_response: bool,
    /// Whether this session was spawned as a child/delegated session.
    pub is_child_session: bool,
    /// Whether command execution may bypass workspace permission restrictions.
    pub disable_permission_restrictions: bool,
    /// Whether the active agent state for this run includes planning.
    pub planning_enabled: bool,
    /// Whether the active agent requests reflective task-status prompt style.
    pub reflection_enabled: bool,
    /// Whether runtime operation manuals may be injected for active task types.
    pub op_manual_enabled: bool,
    /// Whether the caller disabled operation manuals for this run.
    pub no_op_manual: bool,
    /// Whether this session should keep running until the goal is explicitly
    /// settled by task_status.
    pub goal_mode: bool,
    /// Last user command that explicitly enabled goal mode.
    pub last_goal_user_input: String,
    /// Latest provider-reported input token count and active compaction limit.
    pub context_tokens: ContextTokenStats,
    /// Latest terminal provider token/cost report for the session.
    pub runtime_usage: serde_json::Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionManagementDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_name: Option<SessionName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_session_name: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<SessionTaskType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_capabilities: Option<SessionCapabilities>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_current_turn: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_log_retention: Option<SessionLogRetention>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_last_update_at: Option<UtcDateTimeMs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_last_user_message_at: Option<UtcDateTimeMs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_started_at: Option<UtcDateTimeMs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<SessionInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_goal: Option<UserGoal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_objective: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_last_tool_call_response: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_child_session: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_permission_restrictions: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planning_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_manual_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_op_manual: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_goal_user_input: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<ContextTokenStats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_usage: Option<serde_json::Value>,
}

impl<'de> Deserialize<'de> for SessionManagement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            session_id: SessionId,
            session_name: SessionName,
            #[serde(default = "default_auto_session_name")]
            auto_session_name: bool,
            session_directory: PathBuf,
            session_uses_docker: bool,
            #[serde(default, deserialize_with = "deserialize_task_type")]
            task_type: SessionTaskType,
            #[serde(default, deserialize_with = "deserialize_session_capabilities")]
            session_capabilities: SessionCapabilities,
            session_current_turn: u64,
            session_log: Vec<SessionLogEntry>,
            #[serde(default)]
            session_log_retention: SessionLogRetention,
            session_created_at: UtcDateTimeMs,
            session_last_update_at: UtcDateTimeMs,
            #[serde(default = "Utc::now")]
            session_last_user_message_at: UtcDateTimeMs,
            session_started_at: UtcDateTimeMs,
            input: SessionInput,
            user_goal: UserGoal,
            #[serde(default)]
            current_objective: String,
            #[serde(default, deserialize_with = "deserialize_task_plan")]
            task_plan: TaskPlan,
            state: SessionState,
            #[serde(default = "default_use_last_tool_call_response")]
            use_last_tool_call_response: bool,
            #[serde(default)]
            is_child_session: bool,
            #[serde(default)]
            disable_permission_restrictions: bool,
            #[serde(default)]
            planning_enabled: bool,
            #[serde(default)]
            reflection_enabled: bool,
            #[serde(default = "default_op_manual_enabled")]
            op_manual_enabled: bool,
            #[serde(default)]
            no_op_manual: bool,
            #[serde(default)]
            goal_mode: bool,
            #[serde(default)]
            last_goal_user_input: String,
            #[serde(default)]
            context_tokens: ContextTokenStats,
            #[serde(default)]
            runtime_usage: serde_json::Value,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            lifecycle: SessionAggregate {
                session_id: wire.session_id,
                state: wire.state,
                parent_id: None,
                task_plan: wire.task_plan.clone(),
                pending_user_inputs: Vec::new(),
                cancelled: false,
                runtime_ids: Vec::new(),
                active_runtime_id: None,
            },
            session_name: wire.session_name,
            auto_session_name: wire.auto_session_name,
            session_directory: wire.session_directory,
            session_uses_docker: wire.session_uses_docker,
            task_type: wire.task_type,
            session_capabilities: wire.session_capabilities,
            session_current_turn: wire.session_current_turn,
            session_log: wire.session_log,
            session_log_retention: wire.session_log_retention,
            session_created_at: wire.session_created_at,
            session_last_update_at: wire.session_last_update_at,
            session_last_user_message_at: wire.session_last_user_message_at,
            session_started_at: wire.session_started_at,
            input: wire.input,
            user_goal: wire.user_goal,
            current_objective: wire.current_objective,
            use_last_tool_call_response: wire.use_last_tool_call_response,
            is_child_session: wire.is_child_session,
            disable_permission_restrictions: wire.disable_permission_restrictions,
            planning_enabled: wire.planning_enabled,
            reflection_enabled: wire.reflection_enabled,
            op_manual_enabled: wire.op_manual_enabled,
            no_op_manual: wire.no_op_manual,
            goal_mode: wire.goal_mode,
            last_goal_user_input: wire.last_goal_user_input,
            context_tokens: wire.context_tokens,
            runtime_usage: wire.runtime_usage,
        })
    }
}

impl Serialize for SessionManagement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Keep the established SessionManagement wire field order. The lifecycle
        // aggregate owns task_plan internally, but aggregate-only fields are not
        // part of this runtime persistence projection.
        let mut state = serializer.serialize_struct("SessionManagement", 30)?;
        state.serialize_field("session_id", &self.lifecycle.session_id)?;
        state.serialize_field("state", &self.lifecycle.state)?;
        state.serialize_field("session_name", &self.session_name)?;
        state.serialize_field("auto_session_name", &self.auto_session_name)?;
        state.serialize_field("session_directory", &self.session_directory)?;
        state.serialize_field("session_uses_docker", &self.session_uses_docker)?;
        state.serialize_field("task_type", &self.task_type)?;
        state.serialize_field("session_capabilities", &self.session_capabilities)?;
        state.serialize_field("session_current_turn", &self.session_current_turn)?;
        state.serialize_field("session_log", &self.session_log)?;
        state.serialize_field("session_log_retention", &self.session_log_retention)?;
        state.serialize_field("session_created_at", &self.session_created_at)?;
        state.serialize_field("session_last_update_at", &self.session_last_update_at)?;
        state.serialize_field(
            "session_last_user_message_at",
            &self.session_last_user_message_at,
        )?;
        state.serialize_field("session_started_at", &self.session_started_at)?;
        state.serialize_field("input", &self.input)?;
        state.serialize_field("user_goal", &self.user_goal)?;
        state.serialize_field("current_objective", &self.current_objective)?;
        state.serialize_field("task_plan", &self.lifecycle.task_plan)?;
        state.serialize_field(
            "use_last_tool_call_response",
            &self.use_last_tool_call_response,
        )?;
        state.serialize_field("is_child_session", &self.is_child_session)?;
        state.serialize_field(
            "disable_permission_restrictions",
            &self.disable_permission_restrictions,
        )?;
        state.serialize_field("planning_enabled", &self.planning_enabled)?;
        state.serialize_field("reflection_enabled", &self.reflection_enabled)?;
        state.serialize_field("op_manual_enabled", &self.op_manual_enabled)?;
        state.serialize_field("no_op_manual", &self.no_op_manual)?;
        state.serialize_field("goal_mode", &self.goal_mode)?;
        state.serialize_field("last_goal_user_input", &self.last_goal_user_input)?;
        state.serialize_field("context_tokens", &self.context_tokens)?;
        state.serialize_field("runtime_usage", &self.runtime_usage)?;
        state.end()
    }
}

impl Deref for SessionManagement {
    type Target = SessionAggregate;

    fn deref(&self) -> &Self::Target {
        &self.lifecycle
    }
}

impl DerefMut for SessionManagement {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.lifecycle
    }
}

fn default_use_last_tool_call_response() -> bool {
    true
}

fn default_auto_session_name() -> bool {
    true
}

fn default_op_manual_enabled() -> bool {
    true
}

fn no_op_manual_enabled_from_env() -> bool {
    std::env::var("TURA_NO_OP_MANUAL")
        .ok()
        .is_some_and(|value| env_bool_flag(&value))
}

impl SessionManagement {
    pub fn persistence_delta(
        previous: Option<&SessionManagement>,
        current: &SessionManagement,
    ) -> SessionManagementDelta {
        macro_rules! changed {
            ($field:ident) => {
                previous
                    .filter(|previous| previous.$field == current.$field)
                    .map(|_| None)
                    .unwrap_or_else(|| Some(current.$field.clone()))
            };
        }

        SessionManagementDelta {
            session_name: changed!(session_name),
            auto_session_name: changed!(auto_session_name),
            task_type: changed!(task_type),
            session_capabilities: changed!(session_capabilities),
            session_current_turn: changed!(session_current_turn),
            session_log_retention: changed!(session_log_retention),
            session_last_update_at: changed!(session_last_update_at),
            session_last_user_message_at: changed!(session_last_user_message_at),
            session_started_at: changed!(session_started_at),
            input: changed!(input),
            user_goal: changed!(user_goal),
            current_objective: changed!(current_objective),
            use_last_tool_call_response: changed!(use_last_tool_call_response),
            is_child_session: changed!(is_child_session),
            disable_permission_restrictions: changed!(disable_permission_restrictions),
            planning_enabled: changed!(planning_enabled),
            reflection_enabled: changed!(reflection_enabled),
            op_manual_enabled: changed!(op_manual_enabled),
            no_op_manual: changed!(no_op_manual),
            goal_mode: changed!(goal_mode),
            last_goal_user_input: changed!(last_goal_user_input),
            context_tokens: changed!(context_tokens),
            runtime_usage: changed!(runtime_usage),
        }
    }

    pub fn apply_persistence_delta(&mut self, delta: SessionManagementDelta) {
        macro_rules! apply {
            ($field:ident) => {
                if let Some(value) = delta.$field {
                    self.$field = value;
                }
            };
        }

        apply!(session_name);
        apply!(auto_session_name);
        apply!(task_type);
        apply!(session_capabilities);
        apply!(session_current_turn);
        apply!(session_log_retention);
        apply!(session_last_update_at);
        apply!(session_last_user_message_at);
        apply!(session_started_at);
        apply!(input);
        apply!(user_goal);
        apply!(current_objective);
        apply!(use_last_tool_call_response);
        apply!(is_child_session);
        apply!(disable_permission_restrictions);
        apply!(planning_enabled);
        apply!(reflection_enabled);
        apply!(op_manual_enabled);
        apply!(no_op_manual);
        apply!(goal_mode);
        apply!(last_goal_user_input);
        apply!(context_tokens);
        apply!(runtime_usage);
    }

    pub fn lifecycle_projection(&self) -> SessionProjection {
        self.lifecycle.query(SessionQuery::Lifecycle)
    }

    /// Replaces the local read model without executing a transition locally.
    pub fn replace_lifecycle_projection(&mut self, projection: SessionProjection) {
        self.lifecycle = SessionAggregate {
            session_id: projection.session_id,
            state: projection.state,
            parent_id: projection.parent_id,
            task_plan: projection.task_plan,
            pending_user_inputs: projection.pending_user_inputs,
            cancelled: projection.cancelled,
            runtime_ids: projection.runtime_ids,
            active_runtime_id: projection.active_runtime_id,
        };
    }

    pub fn rebind_session_id(&mut self, session_id: SessionId) {
        self.lifecycle.session_id = session_id;
    }

    pub fn restore_state(&mut self, state: SessionState) {
        self.lifecycle.state = state;
    }

    pub fn interrupt(&mut self, now: UtcDateTimeMs) {
        self.lifecycle
            .execute(SessionCommand::InterruptSession)
            .expect("interrupting a session is always valid");
        self.session_last_update_at = now;
    }

    /// Creates a new session in `Created` state.
    #[expect(
        clippy::too_many_arguments,
        reason = "session state construction mirrors the serialized state-machine fields"
    )]
    pub fn new(
        session_id: SessionId,
        session_name: SessionName,
        session_directory: PathBuf,
        session_uses_docker: bool,
        task_type: impl IntoSessionTaskType,
        input: SessionInput,
        user_goal: UserGoal,
        now: UtcDateTimeMs,
    ) -> Self {
        let goal_mode = goal_mode_enabled_from_env();
        let no_op_manual = no_op_manual_enabled_from_env();
        let current_objective = input.user_input.trim().to_string();
        let last_goal_user_input = if goal_mode {
            current_objective.clone()
        } else {
            String::new()
        };
        Self {
            lifecycle: SessionAggregate::new(session_id),
            session_name,
            auto_session_name: true,
            session_directory,
            session_uses_docker,
            task_type: task_type.into_session_task_type(),
            session_capabilities: Vec::new(),
            session_current_turn: 0,
            session_log: Vec::new(),
            session_log_retention: SessionLogRetention::default(),
            session_created_at: now,
            session_last_update_at: now,
            session_last_user_message_at: now,
            session_started_at: now,
            current_objective,
            input,
            user_goal,
            use_last_tool_call_response: true,
            is_child_session: false,
            disable_permission_restrictions: false,
            planning_enabled: false,
            reflection_enabled: false,
            op_manual_enabled: true,
            no_op_manual,
            goal_mode,
            last_goal_user_input,
            context_tokens: ContextTokenStats::default(),
            runtime_usage: serde_json::Value::Null,
        }
    }

    /// Applies a validated state transition and refreshes `session_last_update_at`.
    pub fn transition(&mut self, next: SessionState, now: UtcDateTimeMs) -> Result<(), String> {
        self.lifecycle
            .execute(SessionCommand::ApplyRuntimeState { state: next })
            .map_err(|error| error.to_string())?;
        self.session_last_update_at = now;
        Ok(())
    }

    /// Prepares an existing conversation session for a new user turn.
    ///
    /// `Completed`, `Failed`, and `Cancelled` describe the previous runtime turn,
    /// not the lifetime of the conversation. Reusing a session after switching
    /// back to it should keep its history but start the next run from `Created`.
    pub fn prepare_for_new_user_turn(&mut self, input: SessionInput, now: UtcDateTimeMs) {
        let current_objective = input.user_input.trim().to_string();
        self.current_objective = current_objective.clone();
        if goal_mode_enabled_from_env() {
            self.goal_mode = true;
            self.last_goal_user_input = current_objective;
        }
        self.no_op_manual = no_op_manual_enabled_from_env();
        self.input = input;
        self.session_last_user_message_at = now;
        let previous = self.state;
        self.lifecycle
            .execute(SessionCommand::SubmitUserInput)
            .expect("preparing a user turn is always valid");
        if previous != self.state {
            self.session_started_at = now;
        }
        self.session_last_update_at = now;
    }

    /// Replaces the active task-type list and returns ids that were not present
    /// in the previous state.
    pub fn replace_task_type(&mut self, task_type: impl IntoSessionTaskType) -> Vec<String> {
        let next = task_type.into_session_task_type();
        let previous = self.task_type.iter().cloned().collect::<HashSet<_>>();
        let added = next
            .iter()
            .filter(|task_type| !previous.contains(*task_type))
            .cloned()
            .collect::<Vec<_>>();
        self.task_type = next;
        added
    }

    /// Records capabilities loaded into the current context. Existing entries are never removed.
    pub fn record_session_capabilities<I, S>(&mut self, capabilities: I) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.record_session_capabilities_at(capabilities, Utc::now())
    }

    /// Records capabilities loaded into the current context at a known timestamp.
    pub fn record_session_capabilities_at<I, S>(
        &mut self,
        capabilities: I,
        now: UtcDateTimeMs,
    ) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut seen = self
            .session_capabilities
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let mut added = Vec::new();
        for capability in capabilities {
            let Some(capability) = normalize_session_capability(capability.as_ref()) else {
                continue;
            };
            if seen.insert(capability.clone()) {
                self.session_capabilities.push(capability.clone());
                added.push(capability);
            }
        }
        if !added.is_empty() {
            self.session_last_update_at = now;
        }
        added
    }

    /// Rebuilds the loaded capability set after context compaction.
    pub fn reset_session_capabilities_at<I, S>(&mut self, capabilities: I, now: UtcDateTimeMs)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.session_capabilities = normalize_session_capabilities(capabilities);
        self.session_last_update_at = now;
    }

    pub fn has_session_capability(&self, capability: &str) -> bool {
        normalize_session_capability(capability)
            .is_some_and(|capability| self.session_capabilities.contains(&capability))
    }

    /// Appends a log entry and refreshes the update timestamp.
    pub fn push_log(&mut self, entry: impl Into<String>, now: UtcDateTimeMs) {
        self.session_log.push(entry.into());
        self.session_last_update_at = now;
    }

    /// Records a compact boundary and drops log entries before the retained slice.
    pub fn record_context_compaction_point(
        &mut self,
        retained_from_index: usize,
        compact_entry_index: usize,
        now: UtcDateTimeMs,
    ) {
        let retained_from_index = retained_from_index.min(self.session_log.len());
        let compact_entry_index = compact_entry_index.min(self.session_log.len().saturating_sub(1));
        let previous_omitted = self.session_log_retention.omitted_entries;
        let retained_from_absolute = previous_omitted.saturating_add(retained_from_index as u64);
        let compact_entry_absolute = previous_omitted.saturating_add(compact_entry_index as u64);

        if retained_from_index > 0 {
            self.session_log.drain(0..retained_from_index);
            self.session_log_retention.omitted_entries = retained_from_absolute;
        }

        self.session_log_retention.last_compaction = Some(SessionLogCompactionPoint {
            compact_entry_index: compact_entry_absolute,
            retained_before: self.session_log_retention.omitted_entries,
            retained_from_index: retained_from_absolute,
            compacted_at: now,
        });
        self.session_last_update_at = now;
    }

    pub fn absolute_session_log_index(&self, local_index: usize) -> u64 {
        self.session_log_retention
            .omitted_entries
            .saturating_add(local_index as u64)
    }

    /// Records the timestamp of a user-authored message without coupling it to
    /// assistant/tool updates.
    pub fn record_user_message_at(&mut self, now: UtcDateTimeMs) {
        self.session_last_user_message_at = now;
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
        crate::session_projection::task_plan_summary_json(self)
    }

    pub fn task_plan_detail_json(&self) -> serde_json::Value {
        crate::session_projection::task_plan_detail_json(self)
    }

    pub fn task_management_json(&self) -> serde_json::Value {
        crate::session_projection::task_management_json(self)
    }
}

pub trait IntoSessionTaskType {
    fn into_session_task_type(self) -> SessionTaskType;
}

impl IntoSessionTaskType for SessionTaskType {
    fn into_session_task_type(self) -> SessionTaskType {
        normalize_task_type_values(self)
    }
}

impl IntoSessionTaskType for String {
    fn into_session_task_type(self) -> SessionTaskType {
        normalize_task_type_values([self])
    }
}

impl IntoSessionTaskType for &str {
    fn into_session_task_type(self) -> SessionTaskType {
        normalize_task_type_values([self.to_string()])
    }
}

fn normalize_task_type_values(values: impl IntoIterator<Item = String>) -> SessionTaskType {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() || is_legacy_session_kind(value) {
            continue;
        }
        if seen.insert(value.to_string()) {
            out.push(value.to_string());
        }
    }
    out
}

fn normalize_session_capability(value: &str) -> Option<String> {
    let capability = canonical_capability(value);
    if capability.is_empty() || capability == "command_run" {
        return None;
    }
    Some(capability)
}

fn canonical_capability(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase().replace('-', "_");
    match value.as_str() {
        "bash" | "zsh" | "shell" | "shells" | "shell_command" | "shll" | "shall" => {
            active_shell_command_name().to_string()
        }
        "read_media" | "view_media" | "inspect_media" => "read_media".to_string(),
        "web_discover" | "web_search" | "web_fetch" | "discover_web" | "search_web" => {
            "web_discover".to_string()
        }
        "generate_media" | "image_gen" | "generate_image" | "text_to_image" | "t2i" => {
            "generate_media".to_string()
        }
        other => other.to_string(),
    }
}

fn active_shell_command_name() -> &'static str {
    match std::env::var("TURA_COMMAND_RUN_SHELL")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("bash") => "bash",
        Some("zsh") => "zsh",
        Some("shell") | Some("shell_command") | Some("shll") | Some("shall") => "shell_command",
        _ if cfg!(windows) => "shell_command",
        _ if cfg!(target_os = "macos") => "zsh",
        _ => "bash",
    }
}

fn normalize_session_capabilities<I, S>(values: I) -> SessionCapabilities
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for value in values {
        let Some(capability) = normalize_session_capability(value.as_ref()) else {
            continue;
        };
        if seen.insert(capability.clone()) {
            out.push(capability);
        }
    }
    out
}

fn is_legacy_session_kind(value: &str) -> bool {
    matches!(
        value,
        "coding" | "general" | "programming" | "development" | "testing"
    )
}

fn deserialize_session_capabilities<'de, D>(
    deserializer: D,
) -> Result<SessionCapabilities, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Array(values) => Ok(normalize_session_capabilities(
            values
                .into_iter()
                .filter_map(|value| value.as_str().map(ToString::to_string)),
        )),
        serde_json::Value::String(value) => Ok(normalize_session_capabilities([value])),
        serde_json::Value::Null => Ok(Vec::new()),
        other => Err(serde::de::Error::custom(format!(
            "session_capabilities must be a string array, got {other}"
        ))),
    }
}

fn goal_mode_enabled_from_env() -> bool {
    std::env::var("TURA_GOAL_MODE")
        .ok()
        .is_some_and(|value| env_bool_flag(&value) || value.trim().eq_ignore_ascii_case("goal"))
}

fn env_bool_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "enabled"
    )
}

fn deserialize_task_plan<'de, D>(deserializer: D) -> Result<TaskPlan, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if value.is_array() {
        let detailed_tasks: Vec<TaskStep> =
            serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        return Ok(TaskPlan {
            plan_summary: String::new(),
            detailed_tasks,
        });
    }
    serde_json::from_value(value).map_err(serde::de::Error::custom)
}

fn deserialize_task_type<'de, D>(deserializer: D) -> Result<SessionTaskType, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(value) => Ok(normalize_task_type_values([value])),
        serde_json::Value::Array(values) => {
            Ok(normalize_task_type_values(values.into_iter().filter_map(
                |value| value.as_str().map(ToString::to_string),
            )))
        }
        serde_json::Value::Null => Ok(Vec::new()),
        other => Err(serde::de::Error::custom(format!(
            "task_type must be a string array, got {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{PlanStatus, SessionInput, SessionManagement, TaskStep};
    use crate::{SessionState, StartCondition};
    use chrono::Utc;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn restore_env(key: &str, value: Option<OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

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
                planning_mode_override: None,
            },
            "goal".to_string(),
            now,
        );
        session.restore_state(state);
        session
    }

    #[test]
    fn completed_session_can_prepare_for_another_user_turn() {
        let now = Utc::now();
        let mut session = session_in_state(SessionState::Completed);
        assert_eq!(session.current_objective, "first");

        session.prepare_for_new_user_turn(
            SessionInput {
                user_input: "second".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            now,
        );

        assert_eq!(session.state, SessionState::Created);
        assert_eq!(session.input.user_input, "second");
        assert_eq!(session.current_objective, "second");
        assert!(session.transition(SessionState::Running, now).is_ok());
    }

    #[test]
    fn current_objective_serializes_as_session_state_field() {
        let mut session = session_in_state(SessionState::Running);
        session.current_objective = "focused objective".to_string();

        let value = serde_json::to_value(&session).expect("session should serialize");
        assert_eq!(value["current_objective"], "focused objective");

        let decoded: SessionManagement =
            serde_json::from_value(value).expect("session should deserialize");
        assert_eq!(decoded.current_objective, "focused objective");
    }

    #[test]
    fn session_capabilities_append_only_until_compact_rebuild() {
        let now = Utc::now();
        let mut session = session_in_state(SessionState::Running);

        assert_eq!(
            session.record_session_capabilities_at(
                ["shell_command", "command_run", "read_media", "read_media"],
                now,
            ),
            vec![
                super::active_shell_command_name().to_string(),
                "read_media".to_string()
            ]
        );
        assert_eq!(
            session.session_capabilities,
            vec![
                super::active_shell_command_name().to_string(),
                "read_media".to_string()
            ]
        );
        assert!(session.has_session_capability("shell_command"));
        assert!(session.has_session_capability("read_media"));

        session.replace_task_type(Vec::<String>::new());
        assert_eq!(
            session.session_capabilities,
            vec![
                super::active_shell_command_name().to_string(),
                "read_media".to_string()
            ],
            "task_type changes must not remove capabilities from the active context"
        );

        session.reset_session_capabilities_at(["apply_patch", "apply_patch"], now);
        assert_eq!(
            session.session_capabilities,
            vec!["apply_patch".to_string()]
        );
        assert!(!session.has_session_capability("read_media"));
    }

    #[test]
    fn session_capabilities_persist_and_normalize_legacy_shapes() {
        let mut value =
            serde_json::to_value(session_in_state(SessionState::Running)).expect("serialize");
        value["session_capabilities"] =
            serde_json::json!(["shell_command", "command_run", "read_media", "read_media"]);

        let decoded: SessionManagement =
            serde_json::from_value(value).expect("session capabilities should deserialize");

        assert_eq!(
            decoded.session_capabilities,
            vec![
                super::active_shell_command_name().to_string(),
                "read_media".to_string()
            ]
        );
    }

    #[test]
    fn goal_mode_records_last_goal_user_input_from_env() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let previous = std::env::var_os("TURA_GOAL_MODE");
        std::env::set_var("TURA_GOAL_MODE", "1");

        let mut session = session_in_state(SessionState::Completed);

        assert!(session.goal_mode);
        assert_eq!(session.last_goal_user_input, "first");

        session.prepare_for_new_user_turn(
            SessionInput {
                user_input: "second goal".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            Utc::now(),
        );

        assert!(session.goal_mode);
        assert_eq!(session.last_goal_user_input, "second goal");
        restore_env("TURA_GOAL_MODE", previous);
    }

    #[test]
    fn non_goal_turn_does_not_overwrite_recorded_goal_input() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let previous = std::env::var_os("TURA_GOAL_MODE");
        std::env::set_var("TURA_GOAL_MODE", "1");
        let mut session = session_in_state(SessionState::Completed);
        assert_eq!(session.last_goal_user_input, "first");

        std::env::remove_var("TURA_GOAL_MODE");
        session.prepare_for_new_user_turn(
            SessionInput {
                user_input: "ordinary follow-up".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            Utc::now(),
        );

        assert!(session.goal_mode);
        assert_eq!(session.current_objective, "ordinary follow-up");
        assert_eq!(session.last_goal_user_input, "first");
        restore_env("TURA_GOAL_MODE", previous);
    }

    #[test]
    fn session_state_uses_snake_case_internal_persistence() {
        assert_eq!(
            serde_json::to_value(SessionState::Running).expect("state should serialize"),
            serde_json::json!("running")
        );
        assert_eq!(
            serde_json::from_value::<SessionState>(serde_json::json!("interrupted"))
                .expect("interrupted is a first-class state"),
            SessionState::Interrupted
        );
        assert!(
            serde_json::from_value::<SessionState>(serde_json::json!("Running")).is_err(),
            "internal state persistence must not accept PascalCase aliases"
        );
    }

    #[test]
    fn interrupted_session_can_prepare_for_another_user_turn() {
        let now = Utc::now();
        let mut session = session_in_state(SessionState::Interrupted);

        session.prepare_for_new_user_turn(
            SessionInput {
                user_input: "resume".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            now,
        );

        assert_eq!(session.state, SessionState::Created);
        assert_eq!(session.input.user_input, "resume");
    }

    #[test]
    fn task_management_json_single_task_is_object() {
        let mut session = session_in_state(SessionState::Running);
        session.task_plan.plan_summary = "Fix issue".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "task-1".to_string(),
            step: 1,
            start_condition: StartCondition::SessionIdle,
            task_summary: "Fix issue".to_string(),
            step_deliverable_description: "Verified patch".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });

        let value = session.task_management_json();

        assert!(value.is_object());
        assert_eq!(value["task_id"], "task-1");
        assert_eq!(value["step"], 1);
        assert_eq!(value["plan_summary"], "Fix issue");
        assert_eq!(value["task_summary"], "Fix issue");
        assert_eq!(value["start_condition"], "session_idle");
        assert_eq!(value["status"], "doing");
    }

    #[test]
    fn task_management_json_multi_task_includes_start_conditions() {
        let mut session = session_in_state(SessionState::Running);
        session.task_plan.plan_summary = "Release plan".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "idle".to_string(),
            step: 1,
            start_condition: StartCondition::SessionIdle,
            task_summary: "Wait for idle".to_string(),
            ..TaskStep::default()
        });
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "timer".to_string(),
            step: 2,
            start_condition: StartCondition::ScheduledTask,
            task_summary: "Run later".to_string(),
            ..TaskStep::default()
        });

        let value = session.task_management_json();
        let tasks = value["tasks"]
            .as_array()
            .expect("multi task management should serialize a task list");

        assert_eq!(tasks[0]["start_condition"], "session_idle");
        assert_eq!(tasks[1]["start_condition"], "scheduled_task");
    }

    #[test]
    fn plan_status_rejects_non_canonical_internal_names() {
        assert_eq!(
            serde_json::from_str::<PlanStatus>("\"todo\"").expect("todo should deserialize"),
            PlanStatus::Todo
        );
        assert_eq!(
            serde_json::from_str::<PlanStatus>("\"waiting_user\"")
                .expect("waiting_user should deserialize"),
            PlanStatus::WaitingUser
        );
        assert_eq!(
            serde_json::from_str::<PlanStatus>("\"doing\"").expect("doing should deserialize"),
            PlanStatus::Doing
        );
        assert_eq!(
            serde_json::from_str::<PlanStatus>("\"done\"").expect("done should deserialize"),
            PlanStatus::Done
        );
        assert_eq!(
            serde_json::from_str::<PlanStatus>("\"archived\"")
                .expect("archived should deserialize"),
            PlanStatus::Archived
        );
        for non_canonical in ["pending", "in_progress", "completed", "cancelled"] {
            assert!(
                serde_json::from_str::<PlanStatus>(&format!("\"{non_canonical}\"")).is_err(),
                "{non_canonical} must not be accepted inside persisted task state"
            );
        }
    }
}
