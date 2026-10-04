use async_trait::async_trait;
use core_application::strategy_earnings_target::{
    StrategyEarningsTarget, StrategyEarningsTargetRepository, StrategyEarningsTargetRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::strategy_earnings_target;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref;

#[derive(Clone)]
pub struct PostgresStrategyEarningsTargetRepository {
    db: DatabaseHandle,
}

impl PostgresStrategyEarningsTargetRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl StrategyEarningsTargetRepository for PostgresStrategyEarningsTargetRepository {
    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
        ref_kind: &str,
        ref_id: &str,
    ) -> Result<bool, StrategyEarningsTargetRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(StrategyEarningsTargetRepositoryError::InvalidTransaction)?;
        strategy_earnings_target::Entity::insert(strategy_earnings_target::ActiveModel {
            strategy_id: Set(strategy_id),
            ref_kind: Set(ref_kind.to_owned()),
            ref_id: Set(ref_id.to_owned()),
            created_at: NotSet,
        })
        .on_conflict(
            OnConflict::columns([
                strategy_earnings_target::Column::StrategyId,
                strategy_earnings_target::Column::RefKind,
                strategy_earnings_target::Column::RefId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(transaction)
        .await
        .map(|rows_affected| rows_affected > 0)
        .map_err(repository_error)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
        ref_kind: &str,
        ref_id: &str,
    ) -> Result<bool, StrategyEarningsTargetRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(StrategyEarningsTargetRepositoryError::InvalidTransaction)?;
        strategy_earnings_target::Entity::delete_many()
            .filter(strategy_earnings_target::Column::StrategyId.eq(strategy_id))
            .filter(strategy_earnings_target::Column::RefKind.eq(ref_kind))
            .filter(strategy_earnings_target::Column::RefId.eq(ref_id))
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }

    async fn list(
        &self,
        strategy_id: Uuid,
    ) -> Result<Vec<StrategyEarningsTarget>, StrategyEarningsTargetRepositoryError> {
        strategy_earnings_target::Entity::find()
            .filter(strategy_earnings_target::Column::StrategyId.eq(strategy_id))
            .order_by_asc(strategy_earnings_target::Column::RefKind)
            .order_by_asc(strategy_earnings_target::Column::RefId)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_domain).collect())
            .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> StrategyEarningsTargetRepositoryError {
    StrategyEarningsTargetRepositoryError::Database(persistence_error(error))
}

fn to_domain(row: strategy_earnings_target::Model) -> StrategyEarningsTarget {
    StrategyEarningsTarget {
        ref_kind: row.ref_kind,
        ref_id: row.ref_id,
        created_at: row.created_at,
    }
}
