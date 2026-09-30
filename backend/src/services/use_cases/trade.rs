use std::sync::Arc;

use core_application::trade::TradeUseCases;
use gateway_postgres::PostgresTradeRepository;

use super::UseCases;

impl UseCases {
    pub fn trades(&self) -> TradeUseCases {
        TradeUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresTradeRepository::new(self.db.clone())),
            self.strategy_existence.clone(),
            self.change_history.clone(),
        )
    }
}
