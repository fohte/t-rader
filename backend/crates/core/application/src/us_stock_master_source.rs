use std::sync::Arc;

use async_trait::async_trait;
use core_domain::us_stock_master::UsStockMasterEntry;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum UsStockMasterSourceError {
    #[error("US stock master source error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait UsStockMasterSource: Send + Sync {
    async fn fetch_all_us_stocks(
        &self,
    ) -> Result<Vec<UsStockMasterEntry>, UsStockMasterSourceError>;
}

pub type SharedUsStockMasterSource = Arc<dyn UsStockMasterSource>;
