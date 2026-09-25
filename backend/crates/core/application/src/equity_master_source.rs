use std::sync::Arc;

use async_trait::async_trait;
use core_domain::equity_master::EquityMasterEntry;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum EquityMasterSourceError {
    #[error("equity master source error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait EquityMasterSource: Send + Sync {
    /// 実行日時点の全上場銘柄のマスタを取得する。
    async fn fetch_all_equities_master(
        &self,
    ) -> Result<Vec<EquityMasterEntry>, EquityMasterSourceError>;
}

pub type SharedEquityMasterSource = Arc<dyn EquityMasterSource>;
