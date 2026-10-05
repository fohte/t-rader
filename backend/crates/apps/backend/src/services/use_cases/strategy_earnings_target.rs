use core_application::strategy_earnings_target::StrategyEarningsTargetUseCases;
use gateway_postgres::PostgresStrategyEarningsTargetRepository;
use std::sync::Arc;

use super::UseCases;

impl UseCases {
    pub fn strategy_earnings_targets(&self) -> StrategyEarningsTargetUseCases {
        StrategyEarningsTargetUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresStrategyEarningsTargetRepository::new(self.db.clone())),
            self.refs(),
        )
    }
}
