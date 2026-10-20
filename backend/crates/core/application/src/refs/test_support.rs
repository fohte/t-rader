use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::RefRepositoryError;
use super::repository::RefRepository;
use super::types::{IndicatorRef, RefSearchMatch, RefTerm, StockRef};
use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

pub(crate) struct FakeRefRepository {
    names: HashMap<String, String>,
    fail_stock_lookup: bool,
    stock_requests: Mutex<Vec<Vec<String>>>,
}

impl FakeRefRepository {
    pub(crate) fn new(names: HashMap<String, String>, fail_stock_lookup: bool) -> Self {
        Self {
            names,
            fail_stock_lookup,
            stock_requests: Mutex::default(),
        }
    }

    pub(crate) async fn stock_requests(&self) -> Vec<Vec<String>> {
        self.stock_requests.lock().await.clone()
    }
}

#[async_trait]
impl RefRepository for FakeRefRepository {
    async fn list_stocks(&self, _: Option<&str>) -> Result<Vec<StockRef>, RefRepositoryError> {
        Ok(Vec::new())
    }

    async fn find_stock(&self, _: &str) -> Result<Option<StockRef>, RefRepositoryError> {
        Ok(None)
    }

    async fn stock_sectors(
        &self,
        _: &[String],
    ) -> Result<HashMap<String, Option<String>>, RefRepositoryError> {
        Ok(HashMap::new())
    }

    async fn list_indicators(
        &self,
        _: Option<&str>,
    ) -> Result<Vec<IndicatorRef>, RefRepositoryError> {
        Ok(Vec::new())
    }

    async fn find_indicator(&self, _: &str) -> Result<Option<IndicatorRef>, RefRepositoryError> {
        Ok(None)
    }

    async fn search_all(&self, _: &str, _: u64) -> Result<Vec<RefSearchMatch>, RefRepositoryError> {
        Ok(Vec::new())
    }

    async fn stock_names(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError> {
        self.stock_requests.lock().await.push(ids.to_vec());
        if self.fail_stock_lookup {
            return Err(RefRepositoryError::Database(PersistenceError::Database(
                "reference lookup failed".into(),
            )));
        }
        Ok(ids
            .iter()
            .filter_map(|id| self.names.get(id).map(|name| (id.clone(), name.clone())))
            .collect())
    }

    async fn indicator_names(
        &self,
        _: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError> {
        Ok(HashMap::new())
    }

    async fn group_names(
        &self,
        _: &[String],
    ) -> Result<HashMap<String, String>, RefRepositoryError> {
        Ok(HashMap::new())
    }

    async fn list_terms(&self, _: &str) -> Result<Vec<RefTerm>, RefRepositoryError> {
        Ok(Vec::new())
    }

    async fn insert_term(
        &self,
        _: &UnitOfWorkTransaction,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<bool, RefRepositoryError> {
        Ok(false)
    }

    async fn delete_term(
        &self,
        _: &UnitOfWorkTransaction,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<bool, RefRepositoryError> {
        Ok(false)
    }
}
