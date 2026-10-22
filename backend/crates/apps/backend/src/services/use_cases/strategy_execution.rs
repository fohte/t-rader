use std::sync::Arc;

use core_application::strategy_execution::StrategyExecutionUseCases;
use gateway_postgres::{PostgresStrategyExecutionRepository, PostgresStrategyTaskRepository};

use super::UseCases;

impl UseCases {
    pub fn strategy_executions(&self) -> StrategyExecutionUseCases {
        StrategyExecutionUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresStrategyExecutionRepository::new(self.db.clone())),
            Arc::new(PostgresStrategyTaskRepository::new(self.db.clone())),
            self.custom_indicators(),
            self.custom_indicator_evaluations(),
            self.subagent_definitions(),
            self.agent_configs(),
        )
    }
}
