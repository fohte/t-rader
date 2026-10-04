use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::persistence::PersistenceError;
use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{StockRegistrationRepository, StockRegistrationRepositoryError};
use super::types::NewStockRegistration;

#[derive(Default)]
pub struct FakeStockRegistrationRepository {
    stocks: Mutex<HashMap<String, NewStockRegistration>>,
}

impl FakeStockRegistrationRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn find(&self, stock_id: &str) -> Option<NewStockRegistration> {
        self.stocks.lock().await.get(stock_id).cloned()
    }

    pub async fn is_empty(&self) -> bool {
        self.stocks.lock().await.is_empty()
    }
}

#[async_trait]
impl StockRegistrationRepository for FakeStockRegistrationRepository {
    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        stock: NewStockRegistration,
    ) -> Result<(), StockRegistrationRepositoryError> {
        transaction
            .downcast_ref::<FakeTransaction>()
            .ok_or(StockRegistrationRepositoryError::InvalidTransaction)?;

        let mut stocks = self.stocks.lock().await;
        if stocks.contains_key(stock.id.as_str()) {
            return Err(StockRegistrationRepositoryError::Database(
                PersistenceError::Conflict("stock already exists".into()),
            ));
        }
        stocks.insert(stock.id.as_str().to_owned(), stock);
        Ok(())
    }
}
