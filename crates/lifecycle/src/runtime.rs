use serde::{Deserialize, Serialize};

/// Compatibility projection used by the existing runtime wire payloads.
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
    use super::{RuntimeCallResultStatus, RuntimeState};

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
}
