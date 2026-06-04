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
