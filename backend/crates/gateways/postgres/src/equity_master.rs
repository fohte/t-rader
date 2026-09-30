use std::collections::HashSet;

use async_trait::async_trait;
use chrono::Utc;
use core_application::equity_master::{EquityMasterRepository, EquityMasterRepositoryError};
use core_application::unit_of_work::UnitOfWorkTransaction;
use core_domain::equity_master::EquityMasterEntry;
use sea_orm::ActiveValue::Set;
use sea_orm::EntityTrait;
use sea_orm::sea_query::OnConflict;

use crate::entities::{sector, stock};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

#[derive(Clone)]
pub struct PostgresEquityMasterRepository;

#[async_trait]
impl EquityMasterRepository for PostgresEquityMasterRepository {
    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        entries: &[EquityMasterEntry],
    ) -> Result<usize, EquityMasterRepositoryError> {
        let transaction = postgres_transaction_ref(transaction)
            .ok_or(EquityMasterRepositoryError::InvalidTransaction)?;
        let sectors = entries
            .iter()
            .filter_map(|entry| entry.sector_name.as_deref())
            .collect::<HashSet<_>>();
        if !sectors.is_empty() {
            let models = sectors.into_iter().map(|name| sector::ActiveModel {
                id: Set(name.to_owned()),
                name: Set(name.to_owned()),
            });
            // ON CONFLICT DO NOTHING では RETURNING 行がないため、exec_without_returning を使う。
            sector::Entity::insert_many(models)
                .on_conflict(
                    OnConflict::column(sector::Column::Id)
                        .do_nothing()
                        .to_owned(),
                )
                .exec_without_returning(transaction)
                .await
                .map_err(repository_error)?;
        }

        let now = Utc::now().fixed_offset();
        let models = entries.iter().map(|entry| stock::ActiveModel {
            id: Set(entry.id.clone()),
            name: Set(entry.name.clone()),
            market: Set(entry.market.clone()),
            sector_id: Set(entry.sector_name.clone()),
            product_category: Set(entry.product_category.clone()),
            created_at: Set(now),
            updated_at: Set(now),
        });
        stock::Entity::insert_many(models)
            .on_conflict(
                OnConflict::column(stock::Column::Id)
                    .update_columns([
                        stock::Column::Name,
                        stock::Column::Market,
                        stock::Column::SectorId,
                        stock::Column::ProductCategory,
                        stock::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_without_returning(transaction)
            .await
            .map_err(repository_error)?;

        Ok(entries.len())
    }
}

fn repository_error(error: sea_orm::DbErr) -> EquityMasterRepositoryError {
    EquityMasterRepositoryError::Database(persistence_error(error))
}
