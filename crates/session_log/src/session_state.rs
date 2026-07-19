pub use lifecycle::SessionState;

#[cfg(test)]
mod tests {
    use super::SessionState;

    #[test]
    fn transition_matrix_allows_only_declared_edges_and_self_loops_before_terminal() {
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
                    "unexpected SessionState transition verdict for {from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn failed_cancelled_and_interrupted_reject_every_outbound_transition_including_self_loop() {
        use SessionState::*;

        for terminal in [Failed, Cancelled, Interrupted] {
            for next in [
                Created,
                Running,
                Paused,
                Completed,
                Failed,
                Cancelled,
                Interrupted,
            ] {
                assert!(
                    !terminal.can_transition_to(next),
                    "terminal state {terminal:?} must reject {next:?}"
                );
            }
        }
    }

    #[test]
    fn completed_sessions_can_start_a_follow_up_turn_without_losing_history() {
        use SessionState::*;

        assert!(Completed.can_transition_to(Created));
        assert!(Completed.can_transition_to(Running));
        assert!(!Completed.can_transition_to(Failed));
        assert!(!Completed.can_transition_to(Cancelled));
        assert!(!Completed.can_transition_to(Interrupted));
    }

    #[test]
    fn serde_contract_uses_internal_snake_case_only() {
        use SessionState::*;

        for (state, text) in [
            (Created, "\"created\""),
            (Running, "\"running\""),
            (Paused, "\"paused\""),
            (Completed, "\"completed\""),
            (Failed, "\"failed\""),
            (Cancelled, "\"cancelled\""),
            (Interrupted, "\"interrupted\""),
        ] {
            assert_eq!(serde_json::to_string(&state).expect("serialize"), text);
            assert_eq!(
                serde_json::from_str::<SessionState>(text).expect("deserialize"),
                state
            );
        }

        for invalid in [
            "\"Created\"",
            "\"Running\"",
            "\"Interrupted\"",
            "\"in_progress\"",
            "\"busy\"",
            "\"error\"",
            "\"cancelled_by_user\"",
        ] {
            assert!(
                serde_json::from_str::<SessionState>(invalid).is_err(),
                "internal state spelling must reject {invalid}"
            );
        }
    }

    #[test]
    fn ui_status_is_derived_from_canonical_state() {
        use SessionState::*;

        for state in [Created, Completed] {
            assert_eq!(state.ui_status(), "idle");
        }
        for state in [Running, Paused] {
            assert_eq!(state.ui_status(), "busy");
        }
        for state in [Failed, Cancelled, Interrupted] {
            assert_eq!(state.ui_status(), "error");
        }
    }

    #[test]
    fn only_running_and_paused_are_recoverable_in_flight_states() {
        use SessionState::*;

        for state in [Running, Paused] {
            assert!(state.is_recoverable_running());
        }
        for state in [Created, Completed, Failed, Cancelled, Interrupted] {
            assert!(!state.is_recoverable_running());
        }
    }
}
