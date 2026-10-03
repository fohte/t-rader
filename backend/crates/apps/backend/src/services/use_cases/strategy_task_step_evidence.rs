use std::sync::Arc;

use core_application::strategy_task_step_evidence::StrategyTaskStepEvidenceUseCases;
use gateway_postgres::PostgresStrategyTaskStepEvidenceRepository;

use super::UseCases;

impl UseCases {
    pub fn strategy_task_step_evidence(&self) -> StrategyTaskStepEvidenceUseCases {
        StrategyTaskStepEvidenceUseCases::new(Arc::new(
            PostgresStrategyTaskStepEvidenceRepository::new(self.db.clone()),
        ))
    }
}
