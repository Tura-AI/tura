#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolState {
    Discovered,
    Configured,
    Enabled,
    Disabled,
    Unavailable,
    Running,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolStateEvent {
    Discover,
    LoadConfig,
    Enable,
    Disable,
    ResolveBinaryFailed,
    ExecuteStarted,
    ExecuteSucceeded,
    ExecuteFailed,
}

impl ToolState {
    pub fn transition(self, event: ToolStateEvent) -> Self {
        match event {
            ToolStateEvent::Discover => Self::Discovered,
            ToolStateEvent::LoadConfig => Self::Configured,
            ToolStateEvent::Enable => Self::Enabled,
            ToolStateEvent::Disable => Self::Disabled,
            ToolStateEvent::ResolveBinaryFailed => Self::Unavailable,
            ToolStateEvent::ExecuteStarted => Self::Running,
            ToolStateEvent::ExecuteSucceeded => Self::Succeeded,
            ToolStateEvent::ExecuteFailed => Self::Failed,
        }
    }
}
