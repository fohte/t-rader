use async_trait::async_trait;
use core_application::unit_of_work::UnitOfWorkTransaction;
use core_application::us_stock_master::{UsStockMasterRepository, UsStockMasterRepositoryError};
use core_domain::us_stock_master::UsStockMasterEntry;
use sea_orm::ActiveValue::Set;
use sea_orm::EntityTrait;
use sea_orm::sea_query::OnConflict;

use crate::entities::{instruments, stock};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

const UPSERT_BATCH_SIZE: usize = 5_000;

#[derive(Clone, Default)]
pub struct PostgresUsStockMasterRepository;

#[async_trait]
impl UsStockMasterRepository for PostgresUsStockMasterRepository {
    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        entries: &[UsStockMasterEntry],
    ) -> Result<usize, UsStockMasterRepositoryError> {
        let transaction = postgres_transaction_ref(transaction)
            .ok_or(UsStockMasterRepositoryError::InvalidTransaction)?;
        let now = chrono::Utc::now().fixed_offset();
        for batch in entries.chunks(UPSERT_BATCH_SIZE) {
            let stock_models = batch.iter().map(|entry| stock::ActiveModel {
                id: Set(entry.id.as_str().to_owned()),
                name: Set(entry.name.clone()),
                market: Set(entry.exchange.clone()),
                product_category: Set(None),
                created_at: Set(now),
                updated_at: Set(now),
            });
            stock::Entity::insert_many(stock_models)
                .on_conflict(
                    OnConflict::column(stock::Column::Id)
                        .update_columns([
                            stock::Column::Name,
                            stock::Column::Market,
                            stock::Column::UpdatedAt,
                        ])
                        .to_owned(),
                )
                .exec_without_returning(transaction)
                .await
                .map_err(repository_error)?;
        }

        for batch in entries.chunks(UPSERT_BATCH_SIZE) {
            let instrument_models = batch.iter().map(|entry| instruments::ActiveModel {
                id: Set(entry.id.as_str().to_owned()),
                name: Set(entry.name.clone()),
                market: Set(entry.id.market().to_string()),
                sector: Set(None),
            });
            instruments::Entity::insert_many(instrument_models)
                .on_conflict(
                    OnConflict::column(instruments::Column::Id)
                        .update_columns([instruments::Column::Name, instruments::Column::Market])
                        .to_owned(),
                )
                .exec_without_returning(transaction)
                .await
                .map_err(repository_error)?;
        }

        Ok(entries.len())
    }
}

fn repository_error(error: sea_orm::DbErr) -> UsStockMasterRepositoryError {
    UsStockMasterRepositoryError::Database(persistence_error(error))
}
