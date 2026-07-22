use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

use crate::RuntimeId;

pub type SessionId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    #[default]
    Todo,
    WaitingUser,
    Doing,
    Question,
    Done,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StartCondition {
    SessionIdle,
    #[default]
    UserAction,
    ScheduledTask,
    PollingTask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PollInterval {
    #[serde(default)]
    pub m: u64,
    #[serde(default)]
    pub d: u64,
    #[serde(default)]
    pub h: u64,
    #[serde(default)]
    pub s: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskStep {
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub step: u64,
    #[serde(default)]
    pub sub_session_id: String,
    #[serde(default = "Utc::now")]
    pub start_at: DateTime<Utc>,
    #[serde(default)]
    pub poll_interval: PollInterval,
    #[serde(default)]
    pub start_condition: StartCondition,
    #[serde(default)]
    pub status: PlanStatus,
    #[serde(default)]
    pub task_summary: String,
    #[serde(default)]
    pub step_task: String,
    #[serde(default)]
    pub step_turn: u64,
    #[serde(default)]
    pub step_tool: String,
    #[serde(default)]
    pub step_context: String,
    #[serde(default)]
    pub step_agent_name: String,
    #[serde(default)]
    pub step_deliverable_description: String,
    #[serde(default)]
    pub step_deliverable_path: PathBuf,
}

impl Default for TaskStep {
    fn default() -> Self {
        Self {
            task_id: String::new(),
            step: 0,
            sub_session_id: String::new(),
            start_at: Utc::now(),
            poll_interval: PollInterval::default(),
            start_condition: StartCondition::default(),
            status: PlanStatus::default(),
            task_summary: String::new(),
            step_task: String::new(),
            step_turn: 0,
            step_tool: String::new(),
            step_context: String::new(),
            step_agent_name: String::new(),
            step_deliverable_description: String::new(),
            step_deliverable_path: PathBuf::new(),
        }
    }
}

impl TaskStep {
    pub fn scheduler_eligible(&self, now: DateTime<Utc>) -> bool {
        if matches!(
            self.status,
            PlanStatus::WaitingUser | PlanStatus::Done | PlanStatus::Archived
        ) {
            return false;
        }
        match self.start_condition {
            StartCondition::ScheduledTask | StartCondition::PollingTask => {
                matches!(self.status, PlanStatus::Todo | PlanStatus::Question)
                    && self.start_at <= now
            }
            StartCondition::SessionIdle => {
                matches!(self.status, PlanStatus::Todo | PlanStatus::Question)
            }
            StartCondition::UserAction => false,
        }
    }

    pub fn display_summary(&self, plan_summary: &str) -> String {
        [
            self.task_summary.as_str(),
            self.step_task.as_str(),
            plan_summary,
        ]
        .into_iter()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .unwrap_or("Continue planned task")
        .to_string()
    }

    pub fn advance_polling_start(&mut self, now: DateTime<Utc>) {
        let seconds = self
            .poll_interval
            .s
            .saturating_add(self.poll_interval.m.saturating_mul(60))
            .saturating_add(self.poll_interval.h.saturating_mul(60 * 60))
            .saturating_add(self.poll_interval.d.saturating_mul(24 * 60 * 60))
            .max(1);
        let step = Duration::seconds(seconds.min(i64::MAX as u64) as i64);
        let mut next = self.start_at + step;
        while next <= now {
            next += step;
        }
        self.start_at = next;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TaskPlan {
    #[serde(default)]
    pub plan_summary: String,
    #[serde(default)]
    pub detailed_tasks: Vec<TaskStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct SessionTaskPatch {
    pub task_id: Option<String>,
    pub step: Option<u64>,
    pub task_summary: Option<String>,
    pub deliverable: Option<String>,
    pub sub_session_id: Option<String>,
    pub start_condition: Option<StartCondition>,
    pub start_at: Option<DateTime<Utc>>,
    pub poll_interval: Option<PollInterval>,
    pub status: Option<PlanStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionTaskPlanPatch {
    pub plan_summary: Option<String>,
    pub tasks: Option<Vec<SessionTaskPatch>>,
    pub task: Option<SessionTaskPatch>,
    pub generated_task_ids: Vec<String>,
    pub generated_task_id: String,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Created,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionAggregate {
    pub session_id: SessionId,
    pub state: SessionState,
    pub parent_id: Option<SessionId>,
    pub task_plan: TaskPlan,
    pub pending_user_inputs: Vec<String>,
    pub cancelled: bool,
    pub runtime_ids: Vec<RuntimeId>,
    pub active_runtime_id: Option<RuntimeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionCommand {
    CreateSession {
        task_plan: TaskPlan,
    },
    SubmitUserInput,
    StartUserTurn,
    QueueUserInputWhileBusy {
        input: String,
    },
    ConsumeQueuedUserInputs,
    RuntimeStarted {
        runtime_id: RuntimeId,
    },
    RuntimeRetried {
        runtime_id: RuntimeId,
        fallback_from_id: RuntimeId,
    },
    RuntimeCompleted {
        runtime_id: RuntimeId,
    },
    RuntimeFailed {
        runtime_id: RuntimeId,
    },
    RuntimeCancelled {
        runtime_id: RuntimeId,
    },
    RuntimeEnded {
        runtime_id: RuntimeId,
    },
    ApplyRuntimeState {
        state: SessionState,
    },
    InterruptSession,
    CancelSession,
    RegisterChildSession {
        parent_id: SessionId,
    },
    ForkSession {
        parent_id: SessionId,
    },
    ApplyTaskStatus {
        task_plan: TaskPlan,
    },
    ApplyTaskPatch {
        patch: SessionTaskPatch,
        generated_task_id: String,
        now: DateTime<Utc>,
    },
    ApplyTaskPatches {
        tasks: Vec<SessionTaskPatch>,
        generated_task_ids: Vec<String>,
        now: DateTime<Utc>,
    },
    ApplyTaskPlanPatch {
        patch: SessionTaskPlanPatch,
    },
    StartScheduledTask {
        task_id: String,
        task_summary: String,
        start_condition: StartCondition,
        now: DateTime<Utc>,
    },
    DeleteSession,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionEvent {
    SessionCreated {
        task_plan: TaskPlan,
    },
    UserInputAccepted {
        state: SessionState,
    },
    UserTurnStarted {
        state: SessionState,
    },
    UserInputQueued {
        input: String,
    },
    QueuedUserInputsConsumed {
        inputs: Vec<String>,
    },
    RuntimeStarted {
        runtime_id: RuntimeId,
        state: SessionState,
    },
    RuntimeCompleted {
        runtime_id: RuntimeId,
        state: SessionState,
    },
    RuntimeFailed {
        runtime_id: RuntimeId,
        state: SessionState,
    },
    RuntimeCancelled {
        runtime_id: RuntimeId,
        state: SessionState,
    },
    RuntimeEnded {
        runtime_id: RuntimeId,
        state: SessionState,
    },
    RuntimeStateApplied {
        state: SessionState,
    },
    SessionInterrupted {
        state: SessionState,
        task_plan: TaskPlan,
    },
    SessionCancelled {
        state: SessionState,
    },
    ChildSessionRegistered {
        parent_id: SessionId,
        state: SessionState,
    },
    SessionForked {
        parent_id: SessionId,
    },
    TaskPlanChanged {
        task_plan: TaskPlan,
    },
    ScheduledTaskClaimed {
        task_plan: TaskPlan,
        task_id: String,
        task_summary: String,
        start_condition: StartCondition,
        state: SessionState,
    },
    SessionDeleted,
}

impl SessionEvent {
    fn as_command(&self, aggregate: &SessionAggregate) -> Result<Option<SessionCommand>, String> {
        let command = match self {
            Self::SessionCreated { .. } | Self::SessionForked { .. } => return Ok(None),
            Self::UserInputAccepted { .. } => SessionCommand::SubmitUserInput,
            Self::UserTurnStarted { .. } => SessionCommand::StartUserTurn,
            Self::UserInputQueued { input } => SessionCommand::QueueUserInputWhileBusy {
                input: input.clone(),
            },
            Self::QueuedUserInputsConsumed { .. } => SessionCommand::ConsumeQueuedUserInputs,
            Self::RuntimeStarted { runtime_id, .. } => {
                let started = SessionCommand::RuntimeStarted {
                    runtime_id: runtime_id.clone(),
                };
                if aggregate
                    .decide(started.clone())
                    .is_ok_and(|expected| expected == *self)
                {
                    started
                } else if let Some(fallback_from_id) = aggregate.runtime_ids.last() {
                    SessionCommand::RuntimeRetried {
                        runtime_id: runtime_id.clone(),
                        fallback_from_id: fallback_from_id.clone(),
                    }
                } else {
                    started
                }
            }
            Self::RuntimeCompleted { runtime_id, .. } => SessionCommand::RuntimeCompleted {
                runtime_id: runtime_id.clone(),
            },
            Self::RuntimeFailed { runtime_id, .. } => SessionCommand::RuntimeFailed {
                runtime_id: runtime_id.clone(),
            },
            Self::RuntimeCancelled { runtime_id, .. } => SessionCommand::RuntimeCancelled {
                runtime_id: runtime_id.clone(),
            },
            Self::RuntimeEnded { runtime_id, .. } => SessionCommand::RuntimeEnded {
                runtime_id: runtime_id.clone(),
            },
            Self::RuntimeStateApplied { state } => {
                SessionCommand::ApplyRuntimeState { state: *state }
            }
            Self::SessionInterrupted { .. } => SessionCommand::InterruptSession,
            Self::SessionCancelled { .. } => SessionCommand::CancelSession,
            Self::ChildSessionRegistered { parent_id, .. } => {
                SessionCommand::RegisterChildSession {
                    parent_id: parent_id.clone(),
                }
            }
            Self::TaskPlanChanged { task_plan } => SessionCommand::ApplyTaskStatus {
                task_plan: task_plan.clone(),
            },
            Self::ScheduledTaskClaimed { .. } => {
                return aggregate
                    .scheduled_replay_command(self)
                    .map(Some)
                    .ok_or_else(|| {
                        "scheduled_task_claimed does not match a canonical scheduler result"
                            .to_string()
                    });
            }
            Self::SessionDeleted => SessionCommand::DeleteSession,
        };
        Ok(Some(command))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionQuery {
    Lifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionProjection {
    pub session_id: SessionId,
    pub state: SessionState,
    pub parent_id: Option<SessionId>,
    pub task_plan: TaskPlan,
    pub pending_user_inputs: Vec<String>,
    pub cancelled: bool,
    pub runtime_ids: Vec<RuntimeId>,
    pub active_runtime_id: Option<RuntimeId>,
}

impl SessionProjection {
    pub fn task_management_json(&self, session_started_at: DateTime<Utc>) -> serde_json::Value {
        crate::session_projection::task_management_json(&self.task_plan, session_started_at)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionTransitionError {
    pub previous: SessionState,
    pub next: SessionState,
}

impl fmt::Display for SessionTransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid session state transition: {:?} -> {:?}",
            self.previous, self.next
        )
    }
}

impl std::error::Error for SessionTransitionError {}

impl SessionAggregate {
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            state: SessionState::Created,
            parent_id: None,
            task_plan: TaskPlan::default(),
            pending_user_inputs: Vec::new(),
            cancelled: false,
            runtime_ids: Vec::new(),
            active_runtime_id: None,
        }
    }

    pub fn execute(
        &mut self,
        command: SessionCommand,
    ) -> Result<SessionEvent, SessionTransitionError> {
        let event = self.decide(command)?;
        self.apply(&event);
        Ok(event)
    }

    /// Rebuilds canonical session state from its ordered event stream.
    pub fn replay(
        session_id: SessionId,
        events: impl IntoIterator<Item = SessionEvent>,
    ) -> Result<Self, String> {
        let mut events = events.into_iter();
        let first = events
            .next()
            .ok_or_else(|| format!("session {session_id} has no creation event"))?;
        let creation_command = match &first {
            SessionEvent::SessionCreated { task_plan } => SessionCommand::CreateSession {
                task_plan: task_plan.clone(),
            },
            SessionEvent::ChildSessionRegistered { parent_id, .. } => {
                SessionCommand::RegisterChildSession {
                    parent_id: parent_id.clone(),
                }
            }
            SessionEvent::SessionForked { parent_id } => SessionCommand::ForkSession {
                parent_id: parent_id.clone(),
            },
            _ => return Err("first session event is not a creation event".to_string()),
        };
        let mut aggregate = Self::new(session_id);
        let expected = aggregate
            .decide(creation_command)
            .map_err(|error| error.to_string())?;
        if expected != first {
            return Err(
                "session creation event does not match the canonical reducer result".into(),
            );
        }
        aggregate.apply(&first);
        for event in events {
            aggregate.apply_committed(&event)?;
        }
        Ok(aggregate)
    }

    /// Applies one event received from the canonical ordered stream.
    pub fn apply_committed(&mut self, event: &SessionEvent) -> Result<(), String> {
        if matches!(event, SessionEvent::SessionDeleted) {
            return Err("session_deleted cannot appear in a retained session event stream".into());
        }
        let command = event.as_command(self)?.ok_or_else(|| {
            "session creation events may only be the first session event".to_string()
        })?;
        let expected = self.decide(command).map_err(|error| error.to_string())?;
        if expected != *event {
            return Err("session event does not match the canonical reducer result".to_string());
        }
        self.apply(event);
        Ok(())
    }

    pub fn decide(&self, command: SessionCommand) -> Result<SessionEvent, SessionTransitionError> {
        let previous = self.state;
        match command {
            SessionCommand::CreateSession { task_plan } => {
                Ok(SessionEvent::SessionCreated { task_plan })
            }
            SessionCommand::ApplyRuntimeState { state: next } => {
                let active_runtime_state_transition = self.active_runtime_id.is_some()
                    && matches!(previous, SessionState::Running | SessionState::Paused)
                    && matches!(next, SessionState::Running | SessionState::Paused);
                let execution_terminal_transition = self.active_runtime_id.is_some()
                    && matches!(previous, SessionState::Running | SessionState::Paused)
                    && next.is_terminal();
                if (self.active_runtime_id.is_some()
                    && !active_runtime_state_transition
                    && !execution_terminal_transition)
                    || !previous.can_transition_to(next)
                {
                    return Err(SessionTransitionError { previous, next });
                }
                Ok(SessionEvent::RuntimeStateApplied { state: next })
            }
            SessionCommand::SubmitUserInput => Ok(SessionEvent::UserInputAccepted {
                state: match previous {
                    SessionState::Completed
                    | SessionState::Failed
                    | SessionState::Cancelled
                    | SessionState::Interrupted => SessionState::Created,
                    state => state,
                },
            }),
            SessionCommand::StartUserTurn => {
                if matches!(previous, SessionState::Running | SessionState::Paused) {
                    return Err(SessionTransitionError {
                        previous,
                        next: SessionState::Running,
                    });
                }
                Ok(SessionEvent::UserTurnStarted {
                    state: SessionState::Running,
                })
            }
            SessionCommand::QueueUserInputWhileBusy { input } => {
                let input = input.trim();
                if !matches!(previous, SessionState::Running | SessionState::Paused)
                    || input.is_empty()
                {
                    return Err(SessionTransitionError {
                        previous,
                        next: previous,
                    });
                }
                Ok(SessionEvent::UserInputQueued {
                    input: input.to_string(),
                })
            }
            SessionCommand::ConsumeQueuedUserInputs => Ok(SessionEvent::QueuedUserInputsConsumed {
                inputs: self.pending_user_inputs.clone(),
            }),
            SessionCommand::RuntimeStarted { runtime_id } => {
                let next = SessionState::Running;
                if runtime_id.trim().is_empty()
                    || self
                        .active_runtime_id
                        .as_ref()
                        .is_some_and(|active| active != &runtime_id)
                    || !previous.can_transition_to(next)
                {
                    return Err(SessionTransitionError { previous, next });
                }
                Ok(SessionEvent::RuntimeStarted {
                    runtime_id,
                    state: next,
                })
            }
            SessionCommand::RuntimeRetried {
                runtime_id,
                fallback_from_id,
            } => {
                let next = SessionState::Running;
                if previous != SessionState::Failed
                    || runtime_id.trim().is_empty()
                    || fallback_from_id.trim().is_empty()
                    || runtime_id == fallback_from_id
                    || self.active_runtime_id.is_some()
                    || self.runtime_ids.last() != Some(&fallback_from_id)
                {
                    return Err(SessionTransitionError { previous, next });
                }
                Ok(SessionEvent::RuntimeStarted {
                    runtime_id,
                    state: next,
                })
            }
            SessionCommand::RuntimeCompleted { runtime_id } => {
                let next = SessionState::Completed;
                if !self.runtime_terminal_matches(&runtime_id, next)
                    || !previous.can_transition_to(next)
                {
                    return Err(SessionTransitionError { previous, next });
                }
                Ok(SessionEvent::RuntimeCompleted {
                    runtime_id,
                    state: next,
                })
            }
            SessionCommand::RuntimeFailed { runtime_id } => {
                let next = SessionState::Failed;
                if !self.runtime_terminal_matches(&runtime_id, next)
                    || (previous != next && !previous.can_transition_to(next))
                {
                    return Err(SessionTransitionError { previous, next });
                }
                Ok(SessionEvent::RuntimeFailed {
                    runtime_id,
                    state: next,
                })
            }
            SessionCommand::RuntimeCancelled { runtime_id } => {
                let next = SessionState::Cancelled;
                if !self.runtime_terminal_matches(&runtime_id, next)
                    || (previous != next && !previous.can_transition_to(next))
                {
                    return Err(SessionTransitionError { previous, next });
                }
                Ok(SessionEvent::RuntimeCancelled {
                    runtime_id,
                    state: next,
                })
            }
            SessionCommand::RuntimeEnded { runtime_id } => {
                if runtime_id.trim().is_empty()
                    || self.active_runtime_id.as_ref() != Some(&runtime_id)
                    || !matches!(previous, SessionState::Running | SessionState::Paused)
                {
                    return Err(SessionTransitionError {
                        previous,
                        next: previous,
                    });
                }
                Ok(SessionEvent::RuntimeEnded {
                    runtime_id,
                    state: previous,
                })
            }
            SessionCommand::InterruptSession => {
                let mut task_plan = self.task_plan.clone();
                if !matches!(
                    previous,
                    SessionState::Completed
                        | SessionState::Failed
                        | SessionState::Cancelled
                        | SessionState::Interrupted
                ) {
                    for task in &mut task_plan.detailed_tasks {
                        if task.status == PlanStatus::Doing {
                            task.status = PlanStatus::WaitingUser;
                        }
                    }
                }
                Ok(SessionEvent::SessionInterrupted {
                    state: match previous {
                        SessionState::Completed
                        | SessionState::Failed
                        | SessionState::Cancelled
                        | SessionState::Interrupted => previous,
                        _ => SessionState::Interrupted,
                    },
                    task_plan,
                })
            }
            SessionCommand::CancelSession => Ok(SessionEvent::SessionCancelled {
                state: SessionState::Cancelled,
            }),
            SessionCommand::RegisterChildSession { parent_id } => {
                if !previous.can_transition_to(SessionState::Running) {
                    return Err(SessionTransitionError {
                        previous,
                        next: SessionState::Running,
                    });
                }
                Ok(SessionEvent::ChildSessionRegistered {
                    parent_id,
                    state: SessionState::Running,
                })
            }
            SessionCommand::ForkSession { parent_id } => {
                Ok(SessionEvent::SessionForked { parent_id })
            }
            SessionCommand::ApplyTaskStatus { task_plan } => {
                Ok(SessionEvent::TaskPlanChanged { task_plan })
            }
            SessionCommand::ApplyTaskPatch {
                patch,
                generated_task_id,
                now,
            } => {
                let mut task_plan = self.task_plan.clone();
                patch_one_task(&mut task_plan, patch, generated_task_id, now)?;
                Ok(SessionEvent::TaskPlanChanged { task_plan })
            }
            SessionCommand::ApplyTaskPatches {
                tasks,
                generated_task_ids,
                now,
            } => {
                let mut task_plan = self.task_plan.clone();
                patch_task_list(&mut task_plan, tasks, generated_task_ids, now)?;
                Ok(SessionEvent::TaskPlanChanged { task_plan })
            }
            SessionCommand::ApplyTaskPlanPatch { patch } => {
                let mut task_plan = self.task_plan.clone();
                if apply_task_plan_patch(&mut task_plan, self.state, patch).is_err() {
                    task_plan.clone_from(&self.task_plan);
                }
                Ok(SessionEvent::TaskPlanChanged { task_plan })
            }
            SessionCommand::StartScheduledTask {
                task_id,
                task_summary,
                start_condition,
                now,
            } => {
                let Some(index) = self.task_plan.detailed_tasks.iter().position(|task| {
                    task.task_id == task_id
                        && task.start_condition == start_condition
                        && task.display_summary(&self.task_plan.plan_summary) == task_summary
                        && task.scheduler_eligible(now)
                }) else {
                    return Err(SessionTransitionError {
                        previous,
                        next: previous,
                    });
                };
                if !previous.can_transition_to(SessionState::Running) {
                    return Err(SessionTransitionError {
                        previous,
                        next: SessionState::Running,
                    });
                }
                let mut task_plan = self.task_plan.clone();
                let plan_summary = task_plan.plan_summary.clone();
                let task = &mut task_plan.detailed_tasks[index];
                let task_id = task.task_id.clone();
                let task_summary = task.display_summary(&plan_summary);
                let start_condition = task.start_condition;
                task.status = PlanStatus::Doing;
                if matches!(start_condition, StartCondition::PollingTask) {
                    task.advance_polling_start(now);
                }
                Ok(SessionEvent::ScheduledTaskClaimed {
                    task_plan,
                    task_id,
                    task_summary,
                    start_condition,
                    state: SessionState::Running,
                })
            }
            SessionCommand::DeleteSession => Ok(SessionEvent::SessionDeleted),
        }
    }

    pub fn apply(&mut self, event: &SessionEvent) {
        match event {
            SessionEvent::SessionCreated { task_plan } => {
                self.task_plan.clone_from(task_plan);
            }
            SessionEvent::SessionDeleted => {}
            SessionEvent::UserInputAccepted { state } => {
                self.state = *state;
                self.cancelled = false;
            }
            SessionEvent::UserTurnStarted { state } => {
                self.state = *state;
                self.cancelled = false;
            }
            SessionEvent::RuntimeStarted { runtime_id, state } => {
                self.state = *state;
                if !self.runtime_ids.contains(runtime_id) {
                    self.runtime_ids.push(runtime_id.clone());
                }
                self.active_runtime_id = Some(runtime_id.clone());
            }
            SessionEvent::RuntimeCompleted { runtime_id, state }
            | SessionEvent::RuntimeFailed { runtime_id, state } => {
                self.state = *state;
                if !self.runtime_ids.contains(runtime_id) {
                    self.runtime_ids.push(runtime_id.clone());
                }
                self.active_runtime_id = None;
            }
            SessionEvent::RuntimeCancelled { runtime_id, state } => {
                self.state = *state;
                if !self.runtime_ids.contains(runtime_id) {
                    self.runtime_ids.push(runtime_id.clone());
                }
                self.active_runtime_id = None;
                self.cancelled = true;
                self.pending_user_inputs.clear();
            }
            SessionEvent::RuntimeEnded { runtime_id, state } => {
                self.state = *state;
                if !self.runtime_ids.contains(runtime_id) {
                    self.runtime_ids.push(runtime_id.clone());
                }
                self.active_runtime_id = None;
            }
            SessionEvent::RuntimeStateApplied { state } => {
                self.state = *state;
                if state.is_terminal() {
                    self.active_runtime_id = None;
                }
            }
            SessionEvent::SessionInterrupted { state, task_plan } => {
                self.state = *state;
                self.task_plan.clone_from(task_plan);
                self.pending_user_inputs.clear();
                self.active_runtime_id = None;
            }
            SessionEvent::SessionCancelled { state } => {
                self.state = *state;
                self.cancelled = true;
                self.pending_user_inputs.clear();
                self.active_runtime_id = None;
            }
            SessionEvent::ChildSessionRegistered { parent_id, state } => {
                self.parent_id = Some(parent_id.clone());
                self.state = *state;
            }
            SessionEvent::SessionForked { parent_id } => {
                self.parent_id = Some(parent_id.clone());
            }
            SessionEvent::TaskPlanChanged { task_plan } => {
                self.task_plan.clone_from(task_plan);
            }
            SessionEvent::UserInputQueued { input } => {
                self.pending_user_inputs.push(input.clone());
                self.cancelled = false;
            }
            SessionEvent::QueuedUserInputsConsumed { .. } => self.pending_user_inputs.clear(),
            SessionEvent::ScheduledTaskClaimed {
                task_plan, state, ..
            } => {
                self.task_plan.clone_from(task_plan);
                self.state = *state;
            }
        }
    }

    pub fn query(&self, query: SessionQuery) -> SessionProjection {
        match query {
            SessionQuery::Lifecycle => SessionProjection {
                session_id: self.session_id.clone(),
                state: self.state,
                parent_id: self.parent_id.clone(),
                task_plan: self.task_plan.clone(),
                pending_user_inputs: self.pending_user_inputs.clone(),
                cancelled: self.cancelled,
                runtime_ids: self.runtime_ids.clone(),
                active_runtime_id: self.active_runtime_id.clone(),
            },
        }
    }

    fn runtime_terminal_matches(&self, runtime_id: &RuntimeId, next: SessionState) -> bool {
        if runtime_id.trim().is_empty() {
            return false;
        }
        match self.active_runtime_id.as_ref() {
            Some(active) => active == runtime_id,
            None if self.runtime_ids.contains(runtime_id) => self.state == next,
            None => false,
        }
    }

    fn scheduled_replay_command(&self, event: &SessionEvent) -> Option<SessionCommand> {
        let SessionEvent::ScheduledTaskClaimed {
            task_plan, task_id, ..
        } = event
        else {
            return None;
        };
        let task = self
            .task_plan
            .detailed_tasks
            .iter()
            .find(|task| task.task_id == *task_id)?;
        let now = match task.start_condition {
            StartCondition::ScheduledTask => task.start_at,
            StartCondition::PollingTask => {
                let result_task = task_plan
                    .detailed_tasks
                    .iter()
                    .find(|task| task.task_id == *task_id)?;
                let seconds = task
                    .poll_interval
                    .s
                    .saturating_add(task.poll_interval.m.saturating_mul(60))
                    .saturating_add(task.poll_interval.h.saturating_mul(60 * 60))
                    .saturating_add(task.poll_interval.d.saturating_mul(24 * 60 * 60))
                    .max(1);
                result_task
                    .start_at
                    .checked_sub_signed(Duration::seconds(seconds.min(i64::MAX as u64) as i64))?
            }
            StartCondition::SessionIdle => DateTime::<Utc>::MIN_UTC,
            StartCondition::UserAction => return None,
        };
        let command = SessionCommand::StartScheduledTask {
            task_id: task_id.clone(),
            task_summary: task.display_summary(&self.task_plan.plan_summary),
            start_condition: task.start_condition,
            now,
        };
        if self
            .decide(command.clone())
            .is_ok_and(|expected| expected == *event)
        {
            Some(command)
        } else {
            None
        }
    }
}

fn patch_one_task(
    task_plan: &mut TaskPlan,
    patch: SessionTaskPatch,
    generated_task_id: String,
    now: DateTime<Utc>,
) -> Result<(), SessionTransitionError> {
    let task_id = patch
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if task_id.is_none() && task_plan.detailed_tasks.len() > 1 {
        return Err(task_patch_error());
    }

    let index = task_id.as_ref().and_then(|id| {
        task_plan
            .detailed_tasks
            .iter()
            .position(|task| &task.task_id == id)
    });
    let index = match index {
        Some(index) => index,
        None if task_id.is_some() => {
            task_plan.detailed_tasks.push(TaskStep {
                task_id: task_id.unwrap_or(generated_task_id),
                step: patch
                    .step
                    .unwrap_or(task_plan.detailed_tasks.len() as u64 + 1),
                start_at: now,
                ..TaskStep::default()
            });
            task_plan.detailed_tasks.len() - 1
        }
        None if task_plan.detailed_tasks.is_empty() => {
            let summary = task_plan.plan_summary.clone();
            task_plan.detailed_tasks.push(TaskStep {
                task_id: generated_task_id,
                step: 1,
                start_at: now,
                task_summary: summary.clone(),
                step_task: summary,
                ..TaskStep::default()
            });
            0
        }
        None => 0,
    };
    apply_task_patch(&mut task_plan.detailed_tasks[index], patch);
    if task_plan.plan_summary.trim().is_empty() {
        task_plan.plan_summary = task_plan.detailed_tasks[index].task_summary.clone();
    }
    Ok(())
}

fn apply_task_plan_patch(
    task_plan: &mut TaskPlan,
    state: SessionState,
    patch: SessionTaskPlanPatch,
) -> Result<(), SessionTransitionError> {
    let existing_ids = task_plan
        .detailed_tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect::<Vec<_>>();
    let doing_ids = task_plan
        .detailed_tasks
        .iter()
        .filter(|task| task.status == PlanStatus::Doing)
        .map(|task| task.task_id.clone())
        .collect::<Vec<_>>();
    if let Some(tasks) = patch.tasks {
        patch_task_list(task_plan, tasks, patch.generated_task_ids, patch.now)?;
    }
    if let Some(summary) = patch
        .plan_summary
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        task_plan.plan_summary = summary;
    }
    if let Some(task) = patch.task {
        patch_one_task(task_plan, task, patch.generated_task_id, patch.now)?;
    }
    if matches!(state, SessionState::Running | SessionState::Paused) {
        for task in &mut task_plan.detailed_tasks {
            if doing_ids.contains(&task.task_id)
                && matches!(task.status, PlanStatus::Todo | PlanStatus::Question)
            {
                task.status = PlanStatus::Doing;
            }
        }
    } else {
        for task in &mut task_plan.detailed_tasks {
            if !existing_ids.contains(&task.task_id)
                && task.start_condition == StartCondition::SessionIdle
                && matches!(task.status, PlanStatus::Todo | PlanStatus::Question)
            {
                task.status = PlanStatus::WaitingUser;
            }
        }
    }
    Ok(())
}

fn patch_task_list(
    task_plan: &mut TaskPlan,
    patches: Vec<SessionTaskPatch>,
    generated_task_ids: Vec<String>,
    now: DateTime<Utc>,
) -> Result<(), SessionTransitionError> {
    if generated_task_ids.len() != patches.len() {
        return Err(task_patch_error());
    }
    let mut requested_order = Vec::new();
    for (patch, generated_task_id) in patches.into_iter().zip(generated_task_ids) {
        let task_id = patch
            .task_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or(generated_task_id);
        let existing = task_plan
            .detailed_tasks
            .iter()
            .position(|task| task.task_id == task_id);
        if existing.is_some() && !requested_order.contains(&task_id) {
            requested_order.push(task_id.clone());
        }
        let index = existing.unwrap_or_else(|| {
            task_plan.detailed_tasks.push(TaskStep {
                task_id: task_id.clone(),
                step: patch
                    .step
                    .unwrap_or(task_plan.detailed_tasks.len() as u64 + 1),
                start_at: now,
                ..TaskStep::default()
            });
            task_plan.detailed_tasks.len() - 1
        });
        apply_task_patch(&mut task_plan.detailed_tasks[index], patch);
    }
    task_plan.detailed_tasks.sort_by_key(|task| {
        requested_order
            .iter()
            .position(|task_id| task_id == &task.task_id)
            .unwrap_or(usize::MAX)
    });
    for (index, task) in task_plan.detailed_tasks.iter_mut().enumerate() {
        task.step = index as u64 + 1;
    }
    Ok(())
}

fn apply_task_patch(task: &mut TaskStep, patch: SessionTaskPatch) {
    if let Some(task_id) = patch.task_id.filter(|value| !value.trim().is_empty()) {
        task.task_id = task_id;
    }
    if let Some(step) = patch.step {
        task.step = step;
    }
    if let Some(summary) = patch.task_summary.filter(|value| !value.trim().is_empty()) {
        task.task_summary.clone_from(&summary);
        if task.step_task.trim().is_empty() {
            task.step_task = summary;
        }
    }
    if let Some(deliverable) = patch.deliverable.filter(|value| !value.trim().is_empty()) {
        task.step_deliverable_description = deliverable;
    }
    if let Some(sub_session_id) = patch
        .sub_session_id
        .filter(|value| !value.trim().is_empty())
    {
        task.sub_session_id = sub_session_id;
    }
    if let Some(status) = patch.status {
        task.status = status;
    }
    if let Some(poll_interval) = patch.poll_interval {
        task.poll_interval = poll_interval;
        if poll_interval != PollInterval::default() {
            task.start_condition = StartCondition::PollingTask;
        } else if matches!(task.start_condition, StartCondition::PollingTask) {
            task.start_condition = StartCondition::UserAction;
        }
    }
    if let Some(start_at) = patch.start_at {
        task.start_at = start_at;
        if !matches!(task.start_condition, StartCondition::PollingTask) {
            task.start_condition = StartCondition::ScheduledTask;
        }
    }
    if let Some(start_condition) = patch.start_condition {
        task.start_condition = start_condition;
    }
}

fn task_patch_error() -> SessionTransitionError {
    SessionTransitionError {
        previous: SessionState::Created,
        next: SessionState::Created,
    }
}

impl SessionState {
    pub fn can_transition_to(self, next: Self) -> bool {
        use SessionState::*;

        match (self, next) {
            (Created, Running | Cancelled) => true,
            (Running, Paused | Completed | Failed | Cancelled | Interrupted) => true,
            (Paused, Running | Cancelled | Failed | Interrupted) => true,
            (Completed, Created | Running) => true,
            (Failed | Cancelled | Interrupted, _) => false,
            _ if self == next => true,
            _ => false,
        }
    }

    pub fn ui_status(self) -> &'static str {
        match self {
            Self::Created | Self::Completed => "idle",
            Self::Running | Self::Paused => "busy",
            Self::Failed | Self::Cancelled | Self::Interrupted => "error",
        }
    }

    pub fn is_recoverable_running(self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{
        PlanStatus, PollInterval, SessionAggregate, SessionCommand, SessionEvent,
        SessionProjection, SessionQuery, SessionState, SessionTaskPatch, StartCondition, TaskPlan,
        TaskStep,
    };

    fn aggregate_in_state(state: SessionState) -> SessionAggregate {
        let mut aggregate = SessionAggregate::new("session-fixed".to_string());
        aggregate.state = state;
        aggregate
    }

    #[test]
    fn replay_rejects_noncanonical_session_history() {
        let created = SessionEvent::SessionCreated {
            task_plan: TaskPlan::default(),
        };
        assert!(SessionAggregate::replay("session-fixed".to_string(), []).is_err());
        assert!(SessionAggregate::replay(
            "session-fixed".to_string(),
            [SessionEvent::RuntimeStarted {
                runtime_id: "runtime-fixed".to_string(),
                state: SessionState::Running,
            }],
        )
        .is_err());
        assert!(SessionAggregate::replay(
            "session-fixed".to_string(),
            [created.clone(), created.clone()],
        )
        .is_err());
        assert!(SessionAggregate::replay(
            "session-fixed".to_string(),
            [
                created,
                SessionEvent::RuntimeStarted {
                    runtime_id: "runtime-fixed".to_string(),
                    state: SessionState::Paused,
                },
            ],
        )
        .is_err());
    }

    #[test]
    fn replay_accepts_creation_variants_retry_and_scheduled_claim() {
        for first in [
            SessionEvent::SessionForked {
                parent_id: "parent-fixed".to_string(),
            },
            SessionEvent::ChildSessionRegistered {
                parent_id: "parent-fixed".to_string(),
                state: SessionState::Running,
            },
        ] {
            SessionAggregate::replay("session-fixed".to_string(), [first])
                .expect("canonical creation variant should replay");
        }

        let child_events = [
            SessionEvent::SessionCreated {
                task_plan: TaskPlan::default(),
            },
            SessionEvent::ChildSessionRegistered {
                parent_id: "parent-fixed".to_string(),
                state: SessionState::Running,
            },
        ];
        let child = SessionAggregate::replay("session-fixed".to_string(), child_events)
            .expect("existing session may register as a child");
        assert_eq!(child.parent_id.as_deref(), Some("parent-fixed"));

        let mut retry = SessionAggregate::new("session-fixed".to_string());
        let mut retry_events = vec![
            retry
                .execute(SessionCommand::CreateSession {
                    task_plan: TaskPlan::default(),
                })
                .expect("create session"),
            retry
                .execute(SessionCommand::RuntimeStarted {
                    runtime_id: "runtime-first".to_string(),
                })
                .expect("start runtime"),
            retry
                .execute(SessionCommand::RuntimeFailed {
                    runtime_id: "runtime-first".to_string(),
                })
                .expect("fail runtime"),
        ];
        retry_events.push(
            retry
                .execute(SessionCommand::RuntimeRetried {
                    runtime_id: "runtime-retry".to_string(),
                    fallback_from_id: "runtime-first".to_string(),
                })
                .expect("retry runtime"),
        );
        assert_eq!(
            SessionAggregate::replay("session-fixed".to_string(), retry_events)
                .expect("retry history should replay"),
            retry
        );

        let now = Utc::now();
        let task_plan = TaskPlan {
            plan_summary: "Scheduled plan".to_string(),
            detailed_tasks: vec![TaskStep {
                task_id: "task-fixed".to_string(),
                start_at: now,
                start_condition: StartCondition::PollingTask,
                poll_interval: PollInterval {
                    m: 5,
                    ..PollInterval::default()
                },
                task_summary: "Run now".to_string(),
                ..TaskStep::default()
            }],
        };
        let mut scheduled = SessionAggregate::new("session-fixed".to_string());
        let scheduled_events = vec![
            scheduled
                .execute(SessionCommand::CreateSession { task_plan })
                .expect("create scheduled session"),
            scheduled
                .execute(SessionCommand::StartScheduledTask {
                    task_id: "task-fixed".to_string(),
                    task_summary: "Run now".to_string(),
                    start_condition: StartCondition::PollingTask,
                    now,
                })
                .expect("claim scheduled task"),
        ];
        assert_eq!(
            SessionAggregate::replay("session-fixed".to_string(), scheduled_events)
                .expect("scheduled history should replay"),
            scheduled
        );
    }

    #[test]
    fn transition_matrix_matches_the_reference_session() {
        use SessionState::*;

        let states = [
            Created,
            Running,
            Paused,
            Completed,
            Failed,
            Cancelled,
            Interrupted,
        ];
        for from in states {
            for to in states {
                let expected = matches!(
                    (from, to),
                    (Created, Created | Running | Cancelled)
                        | (
                            Running,
                            Running | Paused | Completed | Failed | Cancelled | Interrupted
                        )
                        | (Paused, Paused | Running | Cancelled | Failed | Interrupted)
                        | (Completed, Completed | Created | Running)
                );
                assert_eq!(
                    from.can_transition_to(to),
                    expected,
                    "unexpected SessionState transition for {from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn serde_accepts_only_the_current_canonical_names() {
        for (state, encoded) in [
            (SessionState::Created, "\"created\""),
            (SessionState::Running, "\"running\""),
            (SessionState::Paused, "\"paused\""),
            (SessionState::Completed, "\"completed\""),
            (SessionState::Failed, "\"failed\""),
            (SessionState::Cancelled, "\"cancelled\""),
            (SessionState::Interrupted, "\"interrupted\""),
        ] {
            assert_eq!(serde_json::to_string(&state).expect("serialize"), encoded);
            assert_eq!(
                serde_json::from_str::<SessionState>(encoded).expect("deserialize"),
                state
            );
        }

        for invalid in ["\"Created\"", "\"busy\"", "\"cancelled_by_user\""] {
            assert!(serde_json::from_str::<SessionState>(invalid).is_err());
        }
    }

    #[test]
    fn aggregate_command_covers_the_complete_transition_table() {
        use SessionState::*;

        let states = [
            Created,
            Running,
            Paused,
            Completed,
            Failed,
            Cancelled,
            Interrupted,
        ];
        for previous in states {
            for next in states {
                let mut aggregate = aggregate_in_state(previous);
                let result = aggregate.execute(SessionCommand::ApplyRuntimeState { state: next });
                assert_eq!(result.is_ok(), previous.can_transition_to(next));
                if previous.can_transition_to(next) {
                    assert_eq!(aggregate.state, next);
                    assert_eq!(
                        result.expect("valid transition event"),
                        SessionEvent::RuntimeStateApplied { state: next }
                    );
                } else {
                    assert_eq!(aggregate.state, previous);
                }
            }
        }
    }

    #[test]
    fn active_runtime_state_application_accepts_live_and_terminal_transitions() {
        for terminal in [
            SessionState::Completed,
            SessionState::Failed,
            SessionState::Cancelled,
            SessionState::Interrupted,
        ] {
            let mut aggregate = SessionAggregate::new("session-fixed".to_string());
            aggregate
                .execute(SessionCommand::RuntimeStarted {
                    runtime_id: "runtime-fixed".to_string(),
                })
                .expect("runtime should start");

            for next in [SessionState::Paused, SessionState::Running] {
                aggregate
                    .execute(SessionCommand::ApplyRuntimeState { state: next })
                    .expect("active runtime may pause or resume");
                assert_eq!(aggregate.state, next);
                assert_eq!(
                    aggregate.active_runtime_id.as_deref(),
                    Some("runtime-fixed")
                );
            }

            aggregate
                .execute(SessionCommand::ApplyRuntimeState { state: terminal })
                .expect("active runtime may apply a legal terminal state");
            assert_eq!(aggregate.state, terminal);
            assert_eq!(aggregate.active_runtime_id, None);
        }

        let mut aggregate = SessionAggregate::new("session-fixed".to_string());
        aggregate
            .execute(SessionCommand::RuntimeStarted {
                runtime_id: "runtime-fixed".to_string(),
            })
            .expect("runtime should start");
        assert!(aggregate
            .decide(SessionCommand::ApplyRuntimeState {
                state: SessionState::Created,
            })
            .is_err());
    }

    #[test]
    fn session_protocol_is_strict_and_projection_is_derived() {
        let aggregate = SessionAggregate::new("session-fixed".to_string());
        assert_eq!(
            aggregate.query(SessionQuery::Lifecycle),
            SessionProjection {
                session_id: "session-fixed".to_string(),
                state: SessionState::Created,
                parent_id: None,
                task_plan: TaskPlan::default(),
                pending_user_inputs: Vec::new(),
                cancelled: false,
                runtime_ids: Vec::new(),
                active_runtime_id: None,
            }
        );
        let retry = SessionCommand::RuntimeRetried {
            runtime_id: "runtime-retry".to_string(),
            fallback_from_id: "runtime-failed".to_string(),
        };
        let retry_json = serde_json::to_value(&retry).expect("serialize runtime retry command");
        assert_eq!(
            retry_json,
            serde_json::json!({
                "command": "runtime_retried",
                "runtime_id": "runtime-retry",
                "fallback_from_id": "runtime-failed"
            })
        );
        assert_eq!(
            serde_json::from_value::<SessionCommand>(retry_json)
                .expect("deserialize runtime retry command"),
            retry
        );
        assert!(serde_json::from_value::<SessionCommand>(serde_json::json!({
            "command": "runtime_retried",
            "runtime_id": "runtime-retry"
        }))
        .is_err());
        assert!(serde_json::from_str::<SessionCommand>(
            r#"{"command":"apply_runtime_state","state":"running","extra":true}"#
        )
        .is_err());
        assert!(serde_json::from_str::<SessionProjection>(
            r#"{"session_id":"session-fixed","state":"created","extra":true}"#
        )
        .is_err());
    }

    #[test]
    fn prepare_user_turn_preserves_live_states_and_reopens_terminal_states() {
        for (previous, expected) in [
            (SessionState::Created, SessionState::Created),
            (SessionState::Running, SessionState::Running),
            (SessionState::Paused, SessionState::Paused),
            (SessionState::Completed, SessionState::Created),
            (SessionState::Failed, SessionState::Created),
            (SessionState::Cancelled, SessionState::Created),
            (SessionState::Interrupted, SessionState::Created),
        ] {
            let mut aggregate = aggregate_in_state(previous);
            let event = aggregate
                .execute(SessionCommand::SubmitUserInput)
                .expect("preparing a user turn is always valid");
            assert_eq!(aggregate.state, expected);
            assert_eq!(event, SessionEvent::UserInputAccepted { state: expected });
        }
    }

    #[test]
    fn interrupt_cancel_and_child_commands_cover_business_boundaries() {
        let mut aggregate = SessionAggregate::new("session-fixed".to_string());
        aggregate
            .execute(SessionCommand::InterruptSession)
            .expect("created session can be interrupted");
        assert_eq!(aggregate.state, SessionState::Interrupted);

        aggregate
            .execute(SessionCommand::SubmitUserInput)
            .expect("interrupted session should reopen");
        aggregate
            .execute(SessionCommand::RegisterChildSession {
                parent_id: "parent-fixed".to_string(),
            })
            .expect("reopened child session should start");
        assert_eq!(aggregate.state, SessionState::Running);
        assert_eq!(aggregate.parent_id.as_deref(), Some("parent-fixed"));

        aggregate
            .execute(SessionCommand::CancelSession)
            .expect("active session can be cancelled");
        assert_eq!(aggregate.state, SessionState::Cancelled);
        assert!(aggregate.cancelled);

        let mut completed = aggregate_in_state(SessionState::Completed);
        completed.task_plan.detailed_tasks.push(TaskStep {
            task_id: "completed-task".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        let completed_projection = completed.query(SessionQuery::Lifecycle);
        completed
            .execute(SessionCommand::InterruptSession)
            .expect("duplicate interruption after completion is harmless");
        assert_eq!(
            completed.query(SessionQuery::Lifecycle),
            completed_projection
        );
    }

    #[test]
    fn task_patch_and_pending_input_commands_update_one_projection() {
        let now = chrono::Utc::now();
        let mut aggregate = SessionAggregate::new("session-fixed".to_string());
        aggregate
            .execute(SessionCommand::ApplyTaskPatch {
                patch: SessionTaskPatch {
                    task_summary: Some("Ship Phase 2".to_string()),
                    poll_interval: Some(PollInterval {
                        m: 5,
                        ..PollInterval::default()
                    }),
                    status: Some(PlanStatus::Question),
                    ..SessionTaskPatch::default()
                },
                generated_task_id: "task-fixed".to_string(),
                now,
            })
            .expect("task patch should apply");
        aggregate
            .execute(SessionCommand::StartUserTurn)
            .expect("user turn should start before an input is queued");
        aggregate
            .execute(SessionCommand::QueueUserInputWhileBusy {
                input: "continue".to_string(),
            })
            .expect("user input should queue");

        let projection = aggregate.query(SessionQuery::Lifecycle);
        assert_eq!(projection.task_plan.plan_summary, "Ship Phase 2");
        assert_eq!(projection.task_plan.detailed_tasks.len(), 1);
        assert_eq!(
            projection.task_plan.detailed_tasks[0].start_condition,
            StartCondition::PollingTask
        );
        assert_eq!(projection.pending_user_inputs, vec!["continue"]);
    }

    #[test]
    fn user_turn_queue_consume_and_cancel_are_one_canonical_lifecycle() {
        let mut aggregate = SessionAggregate::new("session-fixed".to_string());
        aggregate
            .execute(SessionCommand::CancelSession)
            .expect("created session should cancel");
        assert!(aggregate.cancelled);

        aggregate
            .execute(SessionCommand::StartUserTurn)
            .expect("new input should atomically reopen and start a cancelled session");
        assert_eq!(aggregate.state, SessionState::Running);
        assert!(!aggregate.cancelled);

        for input in [" first ", "second"] {
            aggregate
                .execute(SessionCommand::QueueUserInputWhileBusy {
                    input: input.to_string(),
                })
                .expect("busy input should queue");
        }
        let consumed = aggregate
            .execute(SessionCommand::ConsumeQueuedUserInputs)
            .expect("queued inputs should be consumed atomically");
        assert_eq!(
            consumed,
            SessionEvent::QueuedUserInputsConsumed {
                inputs: vec!["first".to_string(), "second".to_string()]
            }
        );
        assert!(aggregate.pending_user_inputs.is_empty());

        aggregate
            .execute(SessionCommand::QueueUserInputWhileBusy {
                input: "discard on cancel".to_string(),
            })
            .expect("busy input should queue");
        aggregate
            .execute(SessionCommand::CancelSession)
            .expect("running session should cancel");
        assert!(aggregate.pending_user_inputs.is_empty());
    }

    #[test]
    fn busy_queue_and_runtime_result_commands_reject_invalid_states() {
        let mut aggregate = SessionAggregate::new("session-fixed".to_string());
        assert!(aggregate
            .execute(SessionCommand::QueueUserInputWhileBusy {
                input: "not busy".to_string()
            })
            .is_err());

        aggregate
            .execute(SessionCommand::RuntimeStarted {
                runtime_id: "runtime-fixed".to_string(),
            })
            .expect("created session may start a runtime");
        assert_eq!(aggregate.state, SessionState::Running);
        assert_eq!(aggregate.runtime_ids, ["runtime-fixed"]);
        assert_eq!(
            aggregate.active_runtime_id.as_deref(),
            Some("runtime-fixed")
        );
        assert!(aggregate
            .execute(SessionCommand::RuntimeCompleted {
                runtime_id: "runtime-stale".to_string(),
            })
            .is_err());
        assert_eq!(aggregate.state, SessionState::Running);
        assert_eq!(
            aggregate.active_runtime_id.as_deref(),
            Some("runtime-fixed")
        );
        aggregate
            .execute(SessionCommand::RuntimeCompleted {
                runtime_id: "runtime-fixed".to_string(),
            })
            .expect("running runtime may complete");
        assert_eq!(aggregate.state, SessionState::Completed);
        assert_eq!(aggregate.active_runtime_id, None);
        assert!(aggregate
            .execute(SessionCommand::RuntimeFailed {
                runtime_id: "runtime-fixed".to_string(),
            })
            .is_err());

        let mut failed = aggregate_in_state(SessionState::Running);
        failed
            .execute(SessionCommand::RuntimeStarted {
                runtime_id: "runtime-failed".to_string(),
            })
            .expect("runtime should start");
        failed
            .execute(SessionCommand::RuntimeFailed {
                runtime_id: "runtime-failed".to_string(),
            })
            .expect("active runtime failure should apply");
        assert_eq!(failed.state, SessionState::Failed);
        assert_eq!(failed.active_runtime_id, None);
        assert!(failed
            .decide(SessionCommand::RuntimeStarted {
                runtime_id: "runtime-unrelated".to_string(),
            })
            .is_err());
        assert!(failed
            .decide(SessionCommand::RuntimeRetried {
                runtime_id: "runtime-retry".to_string(),
                fallback_from_id: "runtime-stale".to_string(),
            })
            .is_err());
        assert!(aggregate
            .decide(SessionCommand::RuntimeRetried {
                runtime_id: "runtime-retry".to_string(),
                fallback_from_id: "runtime-fixed".to_string(),
            })
            .is_err());
        assert!(failed
            .decide(SessionCommand::RuntimeRetried {
                runtime_id: "runtime-failed".to_string(),
                fallback_from_id: "runtime-failed".to_string(),
            })
            .is_err());
        failed
            .execute(SessionCommand::RuntimeRetried {
                runtime_id: "runtime-retry".to_string(),
                fallback_from_id: "runtime-failed".to_string(),
            })
            .expect("failed session may start a retry of its latest runtime");
        assert_eq!(failed.state, SessionState::Running);
        assert_eq!(
            failed.runtime_ids,
            ["runtime-failed".to_string(), "runtime-retry".to_string()]
        );
        assert_eq!(failed.active_runtime_id.as_deref(), Some("runtime-retry"));

        let mut cancelled = aggregate_in_state(SessionState::Running);
        cancelled
            .execute(SessionCommand::RuntimeStarted {
                runtime_id: "runtime-cancelled".to_string(),
            })
            .expect("runtime should start before its terminal callback");
        cancelled
            .execute(SessionCommand::RuntimeCancelled {
                runtime_id: "runtime-cancelled".to_string(),
            })
            .expect("active runtime cancellation should apply");
        assert_eq!(cancelled.state, SessionState::Cancelled);
        assert_eq!(cancelled.runtime_ids, ["runtime-cancelled"]);
        assert_eq!(cancelled.active_runtime_id, None);
        assert!(cancelled.cancelled);
    }

    #[test]
    fn scheduler_claim_is_atomic_with_task_and_lifecycle_state() {
        let now = chrono::Utc::now();
        let mut aggregate = SessionAggregate::new("session-fixed".to_string());
        aggregate.task_plan = TaskPlan {
            plan_summary: "Scheduled work".to_string(),
            detailed_tasks: vec![TaskStep {
                task_id: "task-fixed".to_string(),
                start_at: now,
                start_condition: StartCondition::ScheduledTask,
                status: PlanStatus::Todo,
                task_summary: "Run now".to_string(),
                ..TaskStep::default()
            }],
        };

        let event = aggregate
            .execute(SessionCommand::StartScheduledTask {
                task_id: "task-fixed".to_string(),
                task_summary: "Run now".to_string(),
                start_condition: StartCondition::ScheduledTask,
                now,
            })
            .expect("due task should be claimed");

        assert!(matches!(event, SessionEvent::ScheduledTaskClaimed { .. }));
        assert_eq!(aggregate.state, SessionState::Running);
        assert_eq!(
            aggregate.task_plan.detailed_tasks[0].status,
            PlanStatus::Doing
        );
    }

    #[test]
    fn scheduler_claim_rejects_stale_task_preconditions_without_mutation() {
        let now = chrono::Utc::now();
        let mut aggregate = SessionAggregate::new("session-fixed".to_string());
        aggregate.task_plan = TaskPlan {
            plan_summary: "Scheduled work".to_string(),
            detailed_tasks: vec![TaskStep {
                task_id: "task-due".to_string(),
                start_at: now,
                start_condition: StartCondition::ScheduledTask,
                status: PlanStatus::Todo,
                task_summary: "Run now".to_string(),
                ..TaskStep::default()
            }],
        };
        let before = aggregate.clone();

        assert!(aggregate
            .execute(SessionCommand::StartScheduledTask {
                task_id: "task-stale".to_string(),
                task_summary: "Run now".to_string(),
                start_condition: StartCondition::ScheduledTask,
                now,
            })
            .is_err());
        assert_eq!(aggregate, before);

        assert!(aggregate
            .execute(SessionCommand::StartScheduledTask {
                task_id: "task-due".to_string(),
                task_summary: "Stale summary".to_string(),
                start_condition: StartCondition::ScheduledTask,
                now,
            })
            .is_err());
        assert_eq!(aggregate, before);

        assert!(aggregate
            .execute(SessionCommand::StartScheduledTask {
                task_id: "task-due".to_string(),
                task_summary: "Run now".to_string(),
                start_condition: StartCondition::PollingTask,
                now,
            })
            .is_err());
        assert_eq!(aggregate, before);
    }
}
