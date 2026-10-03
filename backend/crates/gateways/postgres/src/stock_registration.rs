use async_trait::async_trait;
use core_application::stock_registration::{
    NewStockRegistration, StockRegistrationRepository, StockRegistrationRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::EntityTrait;

use crate::entities::{instruments, stock};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref;

#[derive(Clone, Default)]
pub struct PostgresStockRegistrationRepository;

impl PostgresStockRegistrationRepository {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StockRegistrationRepository for PostgresStockRegistrationRepository {
    async fn insert(
        &self,
        unit_of_work: &UnitOfWorkTransaction,
        stock_registration: NewStockRegistration,
    ) -> Result<(), StockRegistrationRepositoryError> {
        let transaction = transaction_ref(unit_of_work)
            .ok_or(StockRegistrationRepositoryError::InvalidTransaction)?;
        let stock_id = stock_registration.id.as_str().to_owned();

        stock::Entity::insert(stock::ActiveModel {
            id: Set(stock_id.clone()),
            name: Set(stock_registration.name.clone()),
            market: Set(Some(stock_registration.exchange)),
            created_at: NotSet,
            updated_at: NotSet,
            product_category: Set(None),
        })
        .exec_without_returning(transaction)
        .await
        .map_err(repository_error)?;

        instruments::Entity::insert(instruments::ActiveModel {
            id: Set(stock_id),
            name: Set(stock_registration.name),
            market: Set(stock_registration.instrument_market.to_string()),
            sector: Set(None),
        })
        .exec_without_returning(transaction)
        .await
        .map_err(repository_error)?;

        Ok(())
    }
}

fn repository_error(error: sea_orm::DbErr) -> StockRegistrationRepositoryError {
    StockRegistrationRepositoryError::Database(persistence_error(error))
}
