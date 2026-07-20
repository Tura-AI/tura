use serde::Serialize;
use std::sync::{atomic::AtomicBool, Arc};

use crate::front_lifecycle::FrontLifecycle;
use crate::services::manager::ServiceManager;
use crate::services::{
    command_run::CommandRunService, execution::ExecutionService, session_db::SessionDbService,
};
use tura_router::registry::Registry;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) manager: ServiceManager,
    pub(crate) registry: Registry,
    pub(crate) session_db: SessionDbService,
    pub(crate) execution: ExecutionService,
    pub(crate) command_run: CommandRunService,
    pub(crate) lifecycle: FrontLifecycle,
    pub(crate) shutdown: Arc<AtomicBool>,
}

impl Serialize for AppState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("AppState")
    }
}

pub(crate) fn build_state() -> AppState {
    AppState {
        manager: ServiceManager::new(),
        registry: Registry::from_static(),
        session_db: SessionDbService::new(),
        execution: ExecutionService::new(),
        command_run: CommandRunService::new(),
        lifecycle: FrontLifecycle::new(),
        shutdown: Arc::new(AtomicBool::new(false)),
    }
}
