use async_trait::async_trait;
use core_application::strategy::{
    InvestableAmount, NewInvestableAmount, NewStrategy, Strategy, StrategyRepository,
    StrategyRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::{strategy as strategy_entity, strategy_investable_amount};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

#[derive(Clone)]
pub struct PostgresStrategyRepository {
    db: DatabaseHandle,
}

impl PostgresStrategyRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl StrategyRepository for PostgresStrategyRepository {
    async fn list(&self) -> Result<Vec<Strategy>, StrategyRepositoryError> {
        strategy_entity::Entity::find()
            .order_by_asc(strategy_entity::Column::SortOrder)
            .order_by_asc(strategy_entity::Column::CreatedAt)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_strategy).collect())
            .map_err(repository_error)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Strategy>, StrategyRepositoryError> {
        strategy_entity::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map(|row| row.map(to_strategy))
            .map_err(repository_error)
    }

    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<Strategy>, StrategyRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        strategy_entity::Entity::find_by_id(id)
            .one(transaction)
            .await
            .map(|row| row.map(to_strategy))
            .map_err(repository_error)
    }

    async fn create(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy: NewStrategy,
    ) -> Result<Strategy, StrategyRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = strategy_entity::ActiveModel {
            id: Set(strategy.id),
            name: Set(strategy.name),
            description: Set(strategy.description),
            sort_order: Set(strategy.sort_order),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        };
        strategy_entity::Entity::insert(model)
            .exec_with_returning(transaction)
            .await
            .map(to_strategy)
            .map_err(repository_error)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy: Strategy,
    ) -> Result<Strategy, StrategyRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = strategy_entity::ActiveModel {
            id: sea_orm::ActiveValue::Unchanged(strategy.id),
            name: Set(strategy.name),
            description: Set(strategy.description),
            sort_order: Set(strategy.sort_order),
            created_at: sea_orm::ActiveValue::Unchanged(strategy.created_at),
            updated_at: Set(strategy.updated_at),
        };
        model
            .update(transaction)
            .await
            .map(to_strategy)
            .map_err(repository_error)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, StrategyRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        strategy_entity::Entity::delete_by_id(id)
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }

    async fn find_current_investable_amount(
        &self,
        strategy_id: Uuid,
        as_of: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<Option<InvestableAmount>, StrategyRepositoryError> {
        strategy_investable_amount::Entity::find()
            .filter(strategy_investable_amount::Column::StrategyId.eq(strategy_id))
            .filter(strategy_investable_amount::Column::EffectiveAt.lte(as_of))
            .order_by_desc(strategy_investable_amount::Column::EffectiveAt)
            .order_by_desc(strategy_investable_amount::Column::CreatedAt)
            .one(&self.db)
            .await
            .map(|row| row.map(to_investable_amount))
            .map_err(repository_error)
    }

    async fn record_investable_amount(
        &self,
        transaction: &UnitOfWorkTransaction,
        amount: NewInvestableAmount,
    ) -> Result<InvestableAmount, StrategyRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = strategy_investable_amount::ActiveModel {
            id: Set(amount.id),
            strategy_id: Set(amount.strategy_id),
            amount_jpy: Set(amount.amount_jpy),
            effective_at: Set(amount.effective_at),
            created_at: sea_orm::ActiveValue::NotSet,
        };
        strategy_investable_amount::Entity::insert(model)
            .exec_with_returning(transaction)
            .await
            .map(to_investable_amount)
            .map_err(repository_error)
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, StrategyRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(StrategyRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> StrategyRepositoryError {
    StrategyRepositoryError::Database(persistence_error(error))
}

fn to_strategy(model: strategy_entity::Model) -> Strategy {
    Strategy {
        id: model.id,
        name: model.name,
        description: model.description,
        sort_order: model.sort_order,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}

fn to_investable_amount(model: strategy_investable_amount::Model) -> InvestableAmount {
    InvestableAmount {
        id: model.id,
        strategy_id: model.strategy_id,
        amount_jpy: model.amount_jpy,
        effective_at: model.effective_at,
        created_at: model.created_at,
    }
}
