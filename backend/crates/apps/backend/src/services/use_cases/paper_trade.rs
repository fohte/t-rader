use std::sync::Arc;

use core_application::paper_trade::PaperTradeUseCases;
use gateway_postgres::{PostgresBarsRepository, PostgresPaperTradeRepository};

use super::UseCases;

impl UseCases {
    pub fn paper_trade(&self) -> PaperTradeUseCases {
        PaperTradeUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresPaperTradeRepository::new()),
            Arc::new(PostgresBarsRepository::new(self.db.clone())),
        )
    }
}
