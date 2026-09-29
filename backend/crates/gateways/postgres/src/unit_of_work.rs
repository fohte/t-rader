use async_trait::async_trait;
use core_application::unit_of_work::{UnitOfWork, UnitOfWorkError, UnitOfWorkTransaction};
use sea_orm::TransactionTrait;

use crate::DatabaseHandle;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresUnitOfWork {
    db: DatabaseHandle,
}

impl PostgresUnitOfWork {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl UnitOfWork for PostgresUnitOfWork {
    async fn begin(&self) -> Result<UnitOfWorkTransaction, UnitOfWorkError> {
        self.db
            .begin()
            .await
            .map(UnitOfWorkTransaction::new)
            .map_err(|error| UnitOfWorkError::Begin(persistence_error(error)))
    }

    async fn commit(&self, transaction: UnitOfWorkTransaction) -> Result<(), UnitOfWorkError> {
        let transaction = transaction
            .downcast::<sea_orm::DatabaseTransaction>()
            .map_err(|_| UnitOfWorkError::InvalidTransaction)?;
        transaction
            .commit()
            .await
            .map_err(|error| UnitOfWorkError::Commit(persistence_error(error)))
    }
}
