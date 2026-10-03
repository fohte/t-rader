use std::sync::Arc;

use core_application::agent_task_client::{FakeAgentTaskClient, SharedAgentTaskClient};
use gateway_postgres::entities::sea_orm_active_enums::StrategyTaskPhase;
use gateway_postgres::entities::strategy_task;
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TaskShape {
    pub(crate) strategy_id: Uuid,
    pub(crate) source: String,
    pub(crate) prompt: String,
    pub(crate) phase: StrategyTaskPhase,
}

impl TaskShape {
    pub(crate) fn from(row: &strategy_task::Model) -> Self {
        Self {
            strategy_id: row.strategy_id,
            source: row.source.clone(),
            prompt: row.prompt.clone(),
            phase: row.phase.clone(),
        }
    }
}

pub(crate) fn fake_agent_task_client() -> SharedAgentTaskClient {
    Arc::new(FakeAgentTaskClient::new())
}
