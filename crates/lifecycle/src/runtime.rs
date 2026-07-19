use crate::session::SessionId;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

pub type RuntimeId = String;

/// Canonical runtime lifecycle projection used by the existing wire payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeCallResultStatus {
    Pending,
    Streaming,
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeState {
    Created,
    Dispatching,
    WaitingFirstToken,
    Streaming,
    Finished,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeAggregate {
    pub runtime_id: RuntimeId,
    pub session_id: SessionId,
    pub state: RuntimeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeCommand {
    Transition { next: RuntimeState },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeEvent {
    StateChanged { state: RuntimeState },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeQuery {
    Lifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProjection {
    pub runtime_id: RuntimeId,
    pub state: RuntimeState,
    pub call_result_status: RuntimeCallResultStatus,
    pub live: bool,
    pub session_db_refresh_required: bool,
}

impl RuntimeProjection {
    pub fn new(runtime_id: RuntimeId, state: RuntimeState) -> Self {
        Self {
            runtime_id,
            state,
            call_result_status: state.call_result_status(),
            live: state.is_live(),
            session_db_refresh_required: !state.is_live(),
        }
    }

    pub fn call_result_status(&self) -> RuntimeCallResultStatus {
        self.call_result_status
    }

    pub fn live_overlay_active(&self) -> bool {
        self.live
    }

    pub fn should_refresh_session_db(&self) -> bool {
        self.session_db_refresh_required
    }
}

impl<'de> Deserialize<'de> for RuntimeProjection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            runtime_id: RuntimeId,
            state: RuntimeState,
            call_result_status: RuntimeCallResultStatus,
            live: bool,
            session_db_refresh_required: bool,
        }

        let wire = Wire::deserialize(deserializer)?;
        let projection = RuntimeProjection::new(wire.runtime_id, wire.state);
        if wire.call_result_status != projection.call_result_status
            || wire.live != projection.live
            || wire.session_db_refresh_required != projection.session_db_refresh_required
        {
            return Err(D::Error::custom(
                "runtime lifecycle projection contradicts runtime state",
            ));
        }
        Ok(projection)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTransitionError {
    pub previous: RuntimeState,
    pub next: RuntimeState,
}

impl fmt::Display for RuntimeTransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid runtime state transition: {:?} -> {:?}",
            self.previous, self.next
        )
    }
}

impl std::error::Error for RuntimeTransitionError {}

impl RuntimeAggregate {
    pub fn new(runtime_id: RuntimeId, session_id: SessionId) -> Self {
        Self {
            runtime_id,
            session_id,
            state: RuntimeState::Created,
        }
    }

    pub fn execute(
        &mut self,
        command: RuntimeCommand,
    ) -> Result<RuntimeEvent, RuntimeTransitionError> {
        let event = self.decide(command)?;
        self.apply(&event);
        Ok(event)
    }

    pub fn decide(&self, command: RuntimeCommand) -> Result<RuntimeEvent, RuntimeTransitionError> {
        let RuntimeCommand::Transition { next } = command;
        let previous = self.state;
        if !previous.can_transition_to(next) {
            return Err(RuntimeTransitionError { previous, next });
        }
        Ok(RuntimeEvent::StateChanged { state: next })
    }

    pub fn apply(&mut self, event: &RuntimeEvent) {
        let RuntimeEvent::StateChanged { state } = event;
        self.state = *state;
    }

    pub fn query(&self, query: RuntimeQuery) -> RuntimeProjection {
        match query {
            RuntimeQuery::Lifecycle => RuntimeProjection::new(self.runtime_id.clone(), self.state),
        }
    }
}

impl RuntimeState {
    pub fn can_transition_to(self, next: Self) -> bool {
        use RuntimeState::*;

        match (self, next) {
            (Created, Dispatching | Failed | TimedOut | Cancelled) => true,
            (Dispatching, WaitingFirstToken | Failed | TimedOut | Cancelled) => true,
            (WaitingFirstToken, Streaming | Finished | Failed | TimedOut | Cancelled) => true,
            (Streaming, Finished | Failed | TimedOut | Cancelled) => true,
            (Finished | Failed | TimedOut | Cancelled, _) => false,
            _ if self == next => true,
            _ => false,
        }
    }

    pub fn call_result_status(self) -> RuntimeCallResultStatus {
        match self {
            Self::Created | Self::Dispatching | Self::WaitingFirstToken => {
                RuntimeCallResultStatus::Pending
            }
            Self::Streaming => RuntimeCallResultStatus::Streaming,
            Self::Finished => RuntimeCallResultStatus::Succeeded,
            Self::Failed => RuntimeCallResultStatus::Failed,
            Self::TimedOut => RuntimeCallResultStatus::TimedOut,
            Self::Cancelled => RuntimeCallResultStatus::Cancelled,
        }
    }

    pub fn is_live(self) -> bool {
        matches!(
            self,
            Self::Created | Self::Dispatching | Self::WaitingFirstToken | Self::Streaming
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeAggregate, RuntimeCallResultStatus, RuntimeCommand, RuntimeEvent, RuntimeProjection,
        RuntimeQuery, RuntimeState,
    };

    #[test]
    fn transition_matrix_matches_the_reference_runtime() {
        use RuntimeState::*;

        let states = [
            Created,
            Dispatching,
            WaitingFirstToken,
            Streaming,
            Finished,
            Failed,
            TimedOut,
            Cancelled,
        ];
        for from in states {
            for to in states {
                let expected = matches!(
                    (from, to),
                    (
                        Created,
                        Created | Dispatching | Failed | TimedOut | Cancelled
                    ) | (
                        Dispatching,
                        Dispatching | WaitingFirstToken | Failed | TimedOut | Cancelled
                    ) | (
                        WaitingFirstToken,
                        WaitingFirstToken | Streaming | Finished | Failed | TimedOut | Cancelled
                    ) | (
                        Streaming,
                        Streaming | Finished | Failed | TimedOut | Cancelled
                    )
                );
                assert_eq!(
                    from.can_transition_to(to),
                    expected,
                    "unexpected RuntimeState transition for {from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn serde_preserves_the_current_runtime_value_exactly() {
        for (state, encoded) in [
            (RuntimeState::Created, "\"Created\""),
            (RuntimeState::Dispatching, "\"Dispatching\""),
            (RuntimeState::WaitingFirstToken, "\"WaitingFirstToken\""),
            (RuntimeState::Streaming, "\"Streaming\""),
            (RuntimeState::Finished, "\"Finished\""),
            (RuntimeState::Failed, "\"Failed\""),
            (RuntimeState::TimedOut, "\"TimedOut\""),
            (RuntimeState::Cancelled, "\"Cancelled\""),
        ] {
            assert_eq!(serde_json::to_string(&state).expect("serialize"), encoded);
            assert_eq!(
                serde_json::from_str::<RuntimeState>(encoded).expect("deserialize"),
                state
            );
        }

        for invalid in ["\"created\"", "\"waiting_first_token\"", "\"Unknown\""] {
            assert!(serde_json::from_str::<RuntimeState>(invalid).is_err());
        }
    }

    #[test]
    fn timeout_and_cancelled_runtime_states_are_canonical() {
        for encoded in ["\"TimedOut\"", "\"Cancelled\""] {
            let state = serde_json::from_str::<RuntimeState>(encoded)
                .expect("terminal runtime state should deserialize");
            assert_eq!(
                serde_json::to_string(&state).expect("serialize terminal runtime state"),
                encoded
            );
        }
    }

    #[test]
    fn call_result_status_is_derived_from_runtime_state() {
        for (state, expected) in [
            (RuntimeState::Created, RuntimeCallResultStatus::Pending),
            (RuntimeState::Dispatching, RuntimeCallResultStatus::Pending),
            (
                RuntimeState::WaitingFirstToken,
                RuntimeCallResultStatus::Pending,
            ),
            (RuntimeState::Streaming, RuntimeCallResultStatus::Streaming),
            (RuntimeState::Finished, RuntimeCallResultStatus::Succeeded),
            (RuntimeState::Failed, RuntimeCallResultStatus::Failed),
            (RuntimeState::TimedOut, RuntimeCallResultStatus::TimedOut),
            (RuntimeState::Cancelled, RuntimeCallResultStatus::Cancelled),
        ] {
            assert_eq!(state.call_result_status(), expected);
        }
    }

    #[test]
    fn liveness_is_derived_from_runtime_state() {
        for state in [
            RuntimeState::Created,
            RuntimeState::Dispatching,
            RuntimeState::WaitingFirstToken,
            RuntimeState::Streaming,
        ] {
            assert!(state.is_live(), "{state:?} should be live");
        }
        for state in [
            RuntimeState::Finished,
            RuntimeState::Failed,
            RuntimeState::TimedOut,
            RuntimeState::Cancelled,
        ] {
            assert!(!state.is_live(), "{state:?} should be terminal");
        }
    }

    #[test]
    fn aggregate_command_covers_the_complete_transition_table() {
        use RuntimeState::*;

        let states = [
            Created,
            Dispatching,
            WaitingFirstToken,
            Streaming,
            Finished,
            Failed,
            TimedOut,
            Cancelled,
        ];
        for previous in states {
            for next in states {
                let mut aggregate = RuntimeAggregate {
                    runtime_id: "runtime-fixed".to_string(),
                    session_id: "session-fixed".to_string(),
                    state: previous,
                };
                let result = aggregate.execute(RuntimeCommand::Transition { next });
                assert_eq!(result.is_ok(), previous.can_transition_to(next));
                if previous.can_transition_to(next) {
                    assert_eq!(aggregate.state, next);
                    assert_eq!(
                        result.expect("valid transition event"),
                        RuntimeEvent::StateChanged { state: next }
                    );
                } else {
                    assert_eq!(aggregate.state, previous);
                }
            }
        }
    }

    #[test]
    fn runtime_protocol_is_strict_and_projection_is_derived() {
        let aggregate =
            RuntimeAggregate::new("runtime-fixed".to_string(), "session-fixed".to_string());
        let projection = aggregate.query(RuntimeQuery::Lifecycle);
        assert_eq!(
            projection,
            RuntimeProjection {
                runtime_id: "runtime-fixed".to_string(),
                state: RuntimeState::Created,
                call_result_status: RuntimeCallResultStatus::Pending,
                live: true,
                session_db_refresh_required: false,
            }
        );
        assert!(serde_json::from_str::<RuntimeCommand>(
            r#"{"command":"transition","next":"Dispatching","extra":true}"#
        )
        .is_err());
        assert!(serde_json::from_str::<RuntimeProjection>(
            r#"{"runtime_id":"runtime-fixed","state":"Created","call_result_status":"Pending","live":true,"session_db_refresh_required":false,"extra":true}"#
        )
        .is_err());
    }
}
