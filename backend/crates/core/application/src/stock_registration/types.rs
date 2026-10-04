use core_domain::instrument::Market;
use core_domain::stock_id::ForeignStockId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewStockRegistration {
    pub id: ForeignStockId,
    pub name: String,
    pub exchange: String,
    pub instrument_market: Market,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterStockCommand {
    pub country: String,
    pub code: String,
    pub name: String,
    pub exchange: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredStock {
    pub id: String,
    pub name: String,
    pub exchange: String,
}
