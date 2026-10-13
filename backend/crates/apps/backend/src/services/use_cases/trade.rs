use std::sync::Arc;

use core_application::trade::{TradeNoteUseCases, TradeSummaryUseCases, TradeUseCases};
use gateway_postgres::{PostgresBarsRepository, PostgresTradeRepository};

use super::UseCases;

impl UseCases {
    pub fn trades(&self) -> TradeUseCases {
        TradeUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresTradeRepository::new(self.db.clone())),
            self.strategy_existence.clone(),
            self.change_history.clone(),
            Arc::new(PostgresBarsRepository::new(self.db.clone())),
        )
    }

    pub fn trade_summaries(&self) -> TradeSummaryUseCases {
        TradeSummaryUseCases::new(self.trades(), self.bars())
    }

    pub fn trade_notes(&self) -> TradeNoteUseCases {
        TradeNoteUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresTradeRepository::new(self.db.clone())),
            self.note_reads(),
        )
    }
}
