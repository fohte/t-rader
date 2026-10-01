use async_trait::async_trait;
use std::collections::HashMap;

use crate::unit_of_work::UnitOfWorkTransaction;

use super::error::RefRepositoryError;
use super::types::{IndicatorRef, RefSearchMatch, RefTerm, StockRef};

#[async_trait]
pub trait RefRepository: Send + Sync {
    async fn list_stocks(&self, query: Option<&str>) -> Result<Vec<StockRef>, RefRepositoryError>;
    async fn find_stock(&self, id: &str) -> Result<Option<StockRef>, RefRepositoryError>;
    async fn stock_sectors(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, Option<String>>, RefRepositoryError>;
    async fn list_indicators(
        &self,
        query: Option<&str>,
    ) -> Result<Vec<IndicatorRef>, RefRepositoryError>;
    async fn find_indicator(&self, id: &str) -> Result<Option<IndicatorRef>, RefRepositoryError>;
    async fn search_all(
        &self,
        pattern: &str,
        limit: u64,
    ) -> Result<Vec<RefSearchMatch>, RefRepositoryError>;
    async fn stock_names(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError>;
    async fn indicator_names(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError>;
    async fn group_names(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError>;
    async fn list_terms(&self, ref_kind: &str) -> Result<Vec<RefTerm>, RefRepositoryError>;
    async fn insert_term(
        &self,
        transaction: &UnitOfWorkTransaction,
        ref_kind: &str,
        ref_id: &str,
        term: &str,
        origin: &str,
    ) -> Result<bool, RefRepositoryError>;
    async fn delete_term(
        &self,
        transaction: &UnitOfWorkTransaction,
        ref_kind: &str,
        ref_id: &str,
        term: &str,
    ) -> Result<bool, RefRepositoryError>;
}

pub type SharedRefRepository = std::sync::Arc<dyn RefRepository + Send + Sync>;
