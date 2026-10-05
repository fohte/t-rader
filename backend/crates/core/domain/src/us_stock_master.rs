use crate::stock_id::ForeignStockId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsStockMasterEntry {
    pub id: ForeignStockId,
    pub name: String,
    pub exchange: Option<String>,
}
