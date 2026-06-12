use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type PersonaId = String;
pub type PersonaName = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersonaState {
    Draft,
    Active,
    Archived,
    Error,
}

impl PersonaState {
    pub fn can_transition_to(self, next: PersonaState) -> bool {
        use PersonaState::*;
        match (self, next) {
            (Draft, Active | Archived | Error) => true,
            (Active, Archived | Error) => true,
            (Error, Draft | Archived) => true,
            (Archived, _) => false,
            _ if self == next => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonaManagement {
    pub persona_id: PersonaId,
    pub persona_name: PersonaName,
    pub persona_directory: PathBuf,
    pub default_config: bool,
    pub state: PersonaState,
}

impl PersonaManagement {
    pub fn new(
        persona_id: PersonaId,
        persona_name: PersonaName,
        persona_directory: PathBuf,
        default_config: bool,
    ) -> Self {
        Self {
            persona_id,
            persona_name,
            persona_directory,
            default_config,
            state: PersonaState::Draft,
        }
    }

    pub fn transition(&mut self, next: PersonaState) -> Result<(), String> {
        if !self.state.can_transition_to(next) {
            return Err(format!(
                "invalid persona state transition: {:?} -> {:?}",
                self.state, next
            ));
        }
        self.state = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{PersonaManagement, PersonaState};
    use std::path::PathBuf;

    #[test]
    fn persona_state_transition_matrix_rejects_illegal_and_terminal_edges() {
        use PersonaState::*;

        let states = [Draft, Active, Archived, Error];
        for from in states {
            for to in states {
                let expected = matches!(
                    (from, to),
                    (Draft, Draft | Active | Archived | Error)
                        | (Active, Active | Archived | Error)
                        | (Error, Error | Draft | Archived)
                );
                assert_eq!(
                    from.can_transition_to(to),
                    expected,
                    "unexpected PersonaState transition verdict for {from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn persona_management_transition_updates_only_valid_state() {
        let mut management = PersonaManagement::new(
            "tura".to_string(),
            "Tura".to_string(),
            PathBuf::from("personas/src/tura"),
            true,
        );

        assert_eq!(management.state, PersonaState::Draft);
        management
            .transition(PersonaState::Active)
            .expect("Draft -> Active should be valid");
        assert_eq!(management.state, PersonaState::Active);
        management
            .transition(PersonaState::Archived)
            .expect("Active -> Archived should be valid");
        assert_eq!(management.state, PersonaState::Archived);

        let error = management
            .transition(PersonaState::Draft)
            .expect_err("Archived is terminal");
        assert!(error.contains("Archived -> Draft"));
        assert_eq!(management.state, PersonaState::Archived);
    }

    #[test]
    fn persona_state_serde_accepts_only_lowercase_internal_names() {
        assert_eq!(
            serde_json::to_value(PersonaState::Active).expect("serialize"),
            serde_json::json!("active")
        );
        assert_eq!(
            serde_json::from_value::<PersonaState>(serde_json::json!("error"))
                .expect("deserialize lowercase"),
            PersonaState::Error
        );
        assert!(
            serde_json::from_value::<PersonaState>(serde_json::json!("Active")).is_err(),
            "internal persona state must not accept PascalCase aliases"
        );
    }
}
