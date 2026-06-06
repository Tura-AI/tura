#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandExecution {
    InProcess,
    OneShot,
    Persistent,
}

#[derive(Clone, Debug)]
pub struct CommandManifest {
    pub id: String,
    pub core: bool,
    pub execution: CommandExecution,
    pub binary: Option<String>,
}

impl CommandManifest {
    pub fn core(id: &str) -> Self {
        Self {
            id: id.to_string(),
            core: true,
            execution: CommandExecution::InProcess,
            binary: None,
        }
    }

    pub fn external(id: &str, binary: &str) -> Self {
        Self {
            id: id.to_string(),
            core: false,
            execution: CommandExecution::OneShot,
            binary: Some(binary.to_string()),
        }
    }
}
