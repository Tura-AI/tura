use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

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
    RuntimeStarted,
    RuntimeCompleted,
    RuntimeFailed,
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
        state: SessionState,
    },
    RuntimeCompleted {
        state: SessionState,
    },
    RuntimeFailed {
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

    pub fn decide(&self, command: SessionCommand) -> Result<SessionEvent, SessionTransitionError> {
        let previous = self.state;
        match command {
            SessionCommand::CreateSession { task_plan } => {
                Ok(SessionEvent::SessionCreated { task_plan })
            }
            SessionCommand::ApplyRuntimeState { state: next } => {
                if !previous.can_transition_to(next) {
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
                if !matches!(previous, SessionState::Running | SessionState::Paused)
                    || input.trim().is_empty()
                {
                    return Err(SessionTransitionError {
                        previous,
                        next: previous,
                    });
                }
                Ok(SessionEvent::UserInputQueued { input })
            }
            SessionCommand::ConsumeQueuedUserInputs => Ok(SessionEvent::QueuedUserInputsConsumed {
                inputs: self.pending_user_inputs.clone(),
            }),
            SessionCommand::RuntimeStarted => {
                let next = SessionState::Running;
                if !previous.can_transition_to(next) {
                    return Err(SessionTransitionError { previous, next });
                }
                Ok(SessionEvent::RuntimeStarted { state: next })
            }
            SessionCommand::RuntimeCompleted => {
                let next = match previous {
                    SessionState::Created | SessionState::Completed => previous,
                    _ => SessionState::Completed,
                };
                if !previous.can_transition_to(next) {
                    return Err(SessionTransitionError { previous, next });
                }
                Ok(SessionEvent::RuntimeCompleted { state: next })
            }
            SessionCommand::RuntimeFailed => {
                let next = SessionState::Failed;
                if previous != next && !previous.can_transition_to(next) {
                    return Err(SessionTransitionError { previous, next });
                }
                Ok(SessionEvent::RuntimeFailed { state: next })
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
            SessionCommand::StartScheduledTask { now } => {
                let Some(index) = self
                    .task_plan
                    .detailed_tasks
                    .iter()
                    .position(|task| task.scheduler_eligible(now))
                else {
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
            SessionEvent::RuntimeStarted { state }
            | SessionEvent::RuntimeCompleted { state }
            | SessionEvent::RuntimeFailed { state }
            | SessionEvent::RuntimeStateApplied { state } => self.state = *state,
            SessionEvent::SessionInterrupted { state, task_plan } => {
                self.state = *state;
                self.task_plan.clone_from(task_plan);
                self.pending_user_inputs.clear();
            }
            SessionEvent::SessionCancelled { state } => {
                self.state = *state;
                self.cancelled = true;
                self.pending_user_inputs.clear();
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
            },
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
}

#[cfg(test)]
mod tests {
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
            }
        );
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

        for input in ["first", "second"] {
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
            .execute(SessionCommand::RuntimeStarted)
            .expect("created session may start a runtime");
        assert_eq!(aggregate.state, SessionState::Running);
        aggregate
            .execute(SessionCommand::RuntimeCompleted)
            .expect("running runtime may complete");
        assert_eq!(aggregate.state, SessionState::Completed);
        assert!(aggregate.execute(SessionCommand::RuntimeFailed).is_err());

        let mut failed = aggregate_in_state(SessionState::Failed);
        failed
            .execute(SessionCommand::RuntimeFailed)
            .expect("duplicate runtime failure notification should be idempotent");
        assert_eq!(failed.state, SessionState::Failed);
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
            .execute(SessionCommand::StartScheduledTask { now })
            .expect("due task should be claimed");

        assert!(matches!(event, SessionEvent::ScheduledTaskClaimed { .. }));
        assert_eq!(aggregate.state, SessionState::Running);
        assert_eq!(
            aggregate.task_plan.detailed_tasks[0].status,
            PlanStatus::Doing
        );
    }
}
