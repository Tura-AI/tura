use serde::{Deserialize, Serialize};
use std::fmt;

pub type SessionId = String;

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionCommand {
    Transition { next: SessionState },
    PrepareUserTurn,
    Interrupt,
    Restore { state: SessionState },
    RebindIdentity { session_id: SessionId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionEvent {
    StateChanged { state: SessionState },
    UserTurnPrepared { state: SessionState },
    Interrupted { state: SessionState },
    Restored { state: SessionState },
    IdentityRebound { session_id: SessionId },
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
            SessionCommand::Transition { next } => {
                if !previous.can_transition_to(next) {
                    return Err(SessionTransitionError { previous, next });
                }
                Ok(SessionEvent::StateChanged { state: next })
            }
            SessionCommand::PrepareUserTurn => Ok(SessionEvent::UserTurnPrepared {
                state: match previous {
                    SessionState::Completed
                    | SessionState::Failed
                    | SessionState::Cancelled
                    | SessionState::Interrupted => SessionState::Created,
                    state => state,
                },
            }),
            SessionCommand::Interrupt => Ok(SessionEvent::Interrupted {
                state: match previous {
                    SessionState::Failed | SessionState::Cancelled | SessionState::Interrupted => {
                        previous
                    }
                    _ => SessionState::Interrupted,
                },
            }),
            SessionCommand::Restore { state } => Ok(SessionEvent::Restored { state }),
            SessionCommand::RebindIdentity { session_id } => {
                Ok(SessionEvent::IdentityRebound { session_id })
            }
        }
    }

    pub fn apply(&mut self, event: &SessionEvent) {
        match event {
            SessionEvent::StateChanged { state }
            | SessionEvent::UserTurnPrepared { state }
            | SessionEvent::Interrupted { state }
            | SessionEvent::Restored { state } => self.state = *state,
            SessionEvent::IdentityRebound { session_id } => {
                self.session_id.clone_from(session_id);
            }
        }
    }

    pub fn query(&self, query: SessionQuery) -> SessionProjection {
        match query {
            SessionQuery::Lifecycle => SessionProjection {
                session_id: self.session_id.clone(),
                state: self.state,
            },
        }
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
        SessionAggregate, SessionCommand, SessionEvent, SessionProjection, SessionQuery,
        SessionState,
    };

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
                let mut aggregate = SessionAggregate {
                    session_id: "session-fixed".to_string(),
                    state: previous,
                };
                let result = aggregate.execute(SessionCommand::Transition { next });
                assert_eq!(result.is_ok(), previous.can_transition_to(next));
                if previous.can_transition_to(next) {
                    assert_eq!(aggregate.state, next);
                    assert_eq!(
                        result.expect("valid transition event"),
                        SessionEvent::StateChanged { state: next }
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
            }
        );
        assert!(serde_json::from_str::<SessionCommand>(
            r#"{"command":"transition","next":"running","extra":true}"#
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
            let mut aggregate = SessionAggregate {
                session_id: "session-fixed".to_string(),
                state: previous,
            };
            let event = aggregate
                .execute(SessionCommand::PrepareUserTurn)
                .expect("preparing a user turn is always valid");
            assert_eq!(aggregate.state, expected);
            assert_eq!(event, SessionEvent::UserTurnPrepared { state: expected });
        }
    }

    #[test]
    fn interrupt_and_restore_cover_the_existing_recovery_boundaries() {
        let mut aggregate = SessionAggregate::new("session-fixed".to_string());
        aggregate
            .execute(SessionCommand::Interrupt)
            .expect("created session can be interrupted");
        assert_eq!(aggregate.state, SessionState::Interrupted);

        aggregate
            .execute(SessionCommand::Restore {
                state: SessionState::Paused,
            })
            .expect("persisted state can be restored");
        assert_eq!(aggregate.state, SessionState::Paused);

        let event = aggregate
            .execute(SessionCommand::RebindIdentity {
                session_id: "session-restored".to_string(),
            })
            .expect("a persisted session identity can be rebound");
        assert_eq!(aggregate.session_id, "session-restored");
        assert_eq!(aggregate.state, SessionState::Paused);
        assert_eq!(
            event,
            SessionEvent::IdentityRebound {
                session_id: "session-restored".to_string(),
            }
        );
    }
}
