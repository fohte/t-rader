use std::sync::Arc;

use core_application::portfolio_history::PortfolioHistoryUseCases;
use gateway_postgres::{PostgresBarsRepository, PostgresStrategyRepository, PostgresTradeRepository};

use super::UseCases;

impl UseCases {
    pub fn portfolio_history(&self) -> PortfolioHistoryUseCases {
        PortfolioHistoryUseCases::new(
            Arc::new(PostgresTradeRepository::new(self.db.clone())),
            Arc::new(PostgresBarsRepository::new(self.db.clone())),
            Arc::new(PostgresStrategyRepository::new(self.db.clone())),
        )
    }
}
