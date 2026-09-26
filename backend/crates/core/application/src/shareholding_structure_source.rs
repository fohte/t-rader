use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::holdings::{
    CrossShareholdingDocument, LargeVolumeShareholdingDocument, MajorShareholderDocument,
};

use crate::DateRange;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ShareholdingStructureSourceError {
    #[error("shareholding structure source error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait ShareholdingStructureSource: Send + Sync {
    async fn fetch_large_volume_documents(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<LargeVolumeShareholdingDocument>, ShareholdingStructureSourceError>;

    async fn fetch_major_shareholder_documents(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<MajorShareholderDocument>, ShareholdingStructureSourceError>;

    async fn fetch_cross_shareholding_documents(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<CrossShareholdingDocument>, ShareholdingStructureSourceError>;

    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange>;
}

pub type SharedShareholdingStructureSource = Arc<dyn ShareholdingStructureSource>;
