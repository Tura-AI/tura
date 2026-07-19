use serde::{Deserialize, Serialize};

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
    use super::SessionState;

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
}
