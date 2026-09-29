use async_trait::async_trait;
use core_application::custom_indicator::{
    CustomIndicator, CustomIndicatorRepository, CustomIndicatorRepositoryError, NewCustomIndicator,
    SCOPE_GLOBAL, SCOPE_STRATEGY,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::custom_indicator;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

#[derive(Clone)]
pub struct PostgresCustomIndicatorRepository {
    db: DatabaseHandle,
}

impl PostgresCustomIndicatorRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CustomIndicatorRepository for PostgresCustomIndicatorRepository {
    async fn list_global(&self) -> Result<Vec<CustomIndicator>, CustomIndicatorRepositoryError> {
        custom_indicator::Entity::find()
            .filter(custom_indicator::Column::Scope.eq(SCOPE_GLOBAL))
            .order_by_asc(custom_indicator::Column::Name)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_domain).collect())
            .map_err(repository_error)
    }

    async fn list_strategy(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
    ) -> Result<Vec<CustomIndicator>, CustomIndicatorRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        custom_indicator::Entity::find()
            .filter(custom_indicator::Column::Scope.eq(SCOPE_STRATEGY))
            .filter(custom_indicator::Column::StrategyId.eq(strategy_id))
            .order_by_asc(custom_indicator::Column::Name)
            .all(transaction)
            .await
            .map(|rows| rows.into_iter().map(to_domain).collect())
            .map_err(repository_error)
    }

    async fn find_by_id(
        &self,
        indicator_id: Uuid,
    ) -> Result<Option<CustomIndicator>, CustomIndicatorRepositoryError> {
        custom_indicator::Entity::find_by_id(indicator_id)
            .one(&self.db)
            .await
            .map(|row| row.map(to_domain))
            .map_err(repository_error)
    }

    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        indicator_id: Uuid,
    ) -> Result<Option<CustomIndicator>, CustomIndicatorRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        custom_indicator::Entity::find_by_id(indicator_id)
            .one(transaction)
            .await
            .map(|row| row.map(to_domain))
            .map_err(repository_error)
    }

    async fn resolve_for_strategy(
        &self,
        strategy_id: Uuid,
        name: &str,
    ) -> Result<Option<CustomIndicator>, CustomIndicatorRepositoryError> {
        let strategy_scoped = custom_indicator::Entity::find()
            .filter(custom_indicator::Column::Scope.eq(SCOPE_STRATEGY))
            .filter(custom_indicator::Column::StrategyId.eq(strategy_id))
            .filter(custom_indicator::Column::Name.eq(name))
            .one(&self.db)
            .await
            .map_err(repository_error)?;
        if let Some(indicator) = strategy_scoped {
            return Ok(Some(to_domain(indicator)));
        }

        custom_indicator::Entity::find()
            .filter(custom_indicator::Column::Scope.eq(SCOPE_GLOBAL))
            .filter(custom_indicator::Column::Name.eq(name))
            .one(&self.db)
            .await
            .map(|row| row.map(to_domain))
            .map_err(repository_error)
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        indicator: NewCustomIndicator,
    ) -> Result<CustomIndicator, CustomIndicatorRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = custom_indicator::ActiveModel {
            indicator_id: Set(indicator.indicator_id),
            name: Set(indicator.name),
            scope: Set(indicator.scope),
            strategy_id: Set(indicator.strategy_id),
            code: Set(indicator.code),
            input_schema: Set(indicator.input_schema),
            output_schema: Set(indicator.output_schema),
            description: Set(indicator.description),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        };
        custom_indicator::Entity::insert(model)
            .exec_with_returning(transaction)
            .await
            .map(to_domain)
            .map_err(repository_error)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        indicator: CustomIndicator,
    ) -> Result<CustomIndicator, CustomIndicatorRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = custom_indicator::ActiveModel {
            indicator_id: sea_orm::ActiveValue::Unchanged(indicator.indicator_id),
            name: Set(indicator.name),
            scope: Set(indicator.scope),
            strategy_id: Set(indicator.strategy_id),
            code: Set(indicator.code),
            input_schema: Set(indicator.input_schema),
            output_schema: Set(indicator.output_schema),
            description: Set(indicator.description),
            created_at: sea_orm::ActiveValue::Unchanged(indicator.created_at),
            updated_at: Set(indicator.updated_at),
        };
        model
            .update(transaction)
            .await
            .map(to_domain)
            .map_err(repository_error)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        indicator_id: Uuid,
    ) -> Result<bool, CustomIndicatorRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        custom_indicator::Entity::delete_by_id(indicator_id)
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, CustomIndicatorRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(CustomIndicatorRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> CustomIndicatorRepositoryError {
    CustomIndicatorRepositoryError::Database(persistence_error(error))
}

fn to_domain(model: custom_indicator::Model) -> CustomIndicator {
    CustomIndicator {
        indicator_id: model.indicator_id,
        name: model.name,
        scope: model.scope,
        strategy_id: model.strategy_id,
        code: model.code,
        input_schema: model.input_schema,
        output_schema: model.output_schema,
        description: model.description,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}
