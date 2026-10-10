use std::sync::Arc;

use core_application::market_movers::MarketMoversUseCases;
use gateway_postgres::PostgresMarketMoversQuerySource;

use super::UseCases;

impl UseCases {
    pub fn market_movers(&self) -> MarketMoversUseCases {
        MarketMoversUseCases::new(Arc::new(PostgresMarketMoversQuerySource::new(
            self.db.clone(),
        )))
    }
}
