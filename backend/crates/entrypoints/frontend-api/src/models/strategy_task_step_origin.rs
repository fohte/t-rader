use core_application::strategy_task::StrategyTaskOrigin;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// `GET /api/strategy-task-steps/:step_id` の戻り値。
#[derive(Debug, Serialize, ToSchema)]
pub struct StrategyTaskStepOriginResponse {
    pub strategy_id: Uuid,
    pub task_id: Uuid,
}

impl From<StrategyTaskOrigin> for StrategyTaskStepOriginResponse {
    fn from(origin: StrategyTaskOrigin) -> Self {
        Self {
            strategy_id: origin.strategy_id,
            task_id: origin.task_id,
        }
    }
}
