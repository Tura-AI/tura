#![deny(clippy::unwrap_used)]
#![deny(unsafe_code)]

mod runtime;
mod session;
mod session_management;
mod session_projection;

pub use runtime::{
    AgentId, ContextTokenStats, DEFAULT_CONTEXT_TOKEN_LIMIT, ProviderConfig, RuntimeAggregate,
    RuntimeCallResultStatus, RuntimeCommand, RuntimeError, RuntimeEvent, RuntimeId,
    RuntimeProjection, RuntimeProviderConfig, RuntimeQuery, RuntimeState, RuntimeTransitionError,
    ToolCallRecord, ToolChoice, UsageReport,
};
pub use session::{
    PlanStatus, PollInterval, SessionAggregate, SessionCommand, SessionEvent, SessionId,
    SessionProjection, SessionQuery, SessionState, SessionTaskPatch, SessionTaskPlanPatch,
    SessionTransitionError, StartCondition, TaskPlan, TaskStep,
};
pub use session_management::{
    AgentName, DeliverableDescription, DeliverablePath, FileInput, IntoSessionTaskType,
    SESSION_CONTEXT_TOKEN_LIMIT, SessionCapabilities, SessionInput, SessionLog,
    SessionLogCompactionPoint, SessionLogEntry, SessionLogRetention, SessionManagement,
    SessionManagementDelta, SessionName, SessionTaskType, StepContext, StepToolJson, TaskStatus,
    UserGoal, UserInputText, UtcDateTimeMs,
};
