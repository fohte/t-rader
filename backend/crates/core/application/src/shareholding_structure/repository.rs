use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::holdings::{
    CrossShareholdingDocument, LargeVolumeShareholdingDocument, MajorShareholderDocument,
};
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::ShareholdingStructureBySymbol;

#[derive(Debug, Error)]
pub enum ShareholdingStructureRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
    #[error("malformed {document_kind} details {document_id}: {message}")]
    MalformedDocument {
        document_kind: &'static str,
        document_id: String,
        message: String,
    },
}

#[async_trait]
pub trait ShareholdingStructureRepository: Send + Sync {
    async fn latest_large_volume_submitted_on(
        &self,
    ) -> Result<Option<NaiveDate>, ShareholdingStructureRepositoryError>;
    async fn upsert_large_volume(
        &self,
        transaction: &UnitOfWorkTransaction,
        documents: Vec<LargeVolumeShareholdingDocument>,
    ) -> Result<usize, ShareholdingStructureRepositoryError>;
    async fn latest_major_shareholder_submitted_on(
        &self,
    ) -> Result<Option<NaiveDate>, ShareholdingStructureRepositoryError>;
    async fn upsert_major_shareholders(
        &self,
        transaction: &UnitOfWorkTransaction,
        documents: Vec<MajorShareholderDocument>,
    ) -> Result<usize, ShareholdingStructureRepositoryError>;
    async fn latest_cross_shareholding_submitted_on(
        &self,
    ) -> Result<Option<NaiveDate>, ShareholdingStructureRepositoryError>;
    async fn upsert_cross_shareholdings(
        &self,
        transaction: &UnitOfWorkTransaction,
        documents: Vec<CrossShareholdingDocument>,
    ) -> Result<usize, ShareholdingStructureRepositoryError>;
    async fn find_for_symbol(
        &self,
        symbol: &str,
        limit: u64,
    ) -> Result<ShareholdingStructureBySymbol, ShareholdingStructureRepositoryError>;
}

pub type SharedShareholdingStructureRepository = Arc<dyn ShareholdingStructureRepository>;
