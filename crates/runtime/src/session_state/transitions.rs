use crate::state_machine::session_management::SessionState;

pub(crate) fn can_transition_to(current: SessionState, next: SessionState) -> bool {
    use SessionState::*;

    match (current, next) {
        (Created, Running | Cancelled) => true,
        (Running, Paused | Completed | Failed | Cancelled) => true,
        (Paused, Running | Cancelled | Failed) => true,
        (Completed | Failed | Cancelled, _) => false,
        _ if current == next => true,
        _ => false,
    }
}
