use core_application::trade::TradeUseCases;
use gateway_postgres::DatabaseHandle;

pub fn build_use_cases(db: DatabaseHandle) -> TradeUseCases {
    gateway_postgres::postgres_trade_use_cases(db)
}
