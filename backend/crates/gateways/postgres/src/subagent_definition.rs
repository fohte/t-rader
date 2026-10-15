use async_trait::async_trait;
use core_application::subagent_definition::{
    SubagentDefinition, SubagentDefinitionRepository, SubagentDefinitionRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{Set, Unchanged};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

use crate::DatabaseHandle;
use crate::entities::subagent_definition;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

#[derive(Clone)]
pub struct PostgresSubagentDefinitionRepository {
    db: DatabaseHandle,
}

impl PostgresSubagentDefinitionRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SubagentDefinitionRepository for PostgresSubagentDefinitionRepository {
    async fn list(&self) -> Result<Vec<SubagentDefinition>, SubagentDefinitionRepositoryError> {
        let models = subagent_definition::Entity::find()
            .order_by_asc(subagent_definition::Column::Key)
            .all(&self.db)
            .await
            .map_err(repository_error)?;
        models.into_iter().map(to_domain).collect()
    }

    async fn find_by_key(
        &self,
        key: &str,
    ) -> Result<Option<SubagentDefinition>, SubagentDefinitionRepositoryError> {
        let model = subagent_definition::Entity::find()
            .filter(subagent_definition::Column::Key.eq(key))
            .one(&self.db)
            .await
            .map_err(repository_error)?;
        model.map(to_domain).transpose()
    }

    async fn find_by_key_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<Option<SubagentDefinition>, SubagentDefinitionRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = subagent_definition::Entity::find()
            .filter(subagent_definition::Column::Key.eq(key))
            .one(transaction)
            .await
            .map_err(repository_error)?;
        model.map(to_domain).transpose()
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        definition: SubagentDefinition,
    ) -> Result<SubagentDefinition, SubagentDefinitionRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = subagent_definition::ActiveModel {
            id: Set(definition.id),
            key: Set(definition.key),
            description: Set(definition.description),
            model: Set(definition.model),
            reasoning_effort: Set(definition.reasoning_effort),
            prompt: Set(definition.prompt),
            tools: Set(to_json("tools", &definition.tools)?),
            timeout_minutes: Set(definition.timeout_minutes),
            max_model_calls: Set(definition.max_model_calls),
            tool_limits: Set(to_json("tool_limits", &definition.tool_limits)?),
            output: Set(to_json("output", &definition.output)?),
            created_at: Set(definition.created_at),
            updated_at: Set(definition.updated_at),
        };
        let model = subagent_definition::Entity::insert(model)
            .exec_with_returning(transaction)
            .await
            .map_err(repository_error)?;
        to_domain(model)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        definition: SubagentDefinition,
    ) -> Result<SubagentDefinition, SubagentDefinitionRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = subagent_definition::ActiveModel {
            id: Unchanged(definition.id),
            key: Unchanged(definition.key),
            description: Set(definition.description),
            model: Set(definition.model),
            reasoning_effort: Set(definition.reasoning_effort),
            prompt: Set(definition.prompt),
            tools: Set(to_json("tools", &definition.tools)?),
            timeout_minutes: Set(definition.timeout_minutes),
            max_model_calls: Set(definition.max_model_calls),
            tool_limits: Set(to_json("tool_limits", &definition.tool_limits)?),
            output: Set(to_json("output", &definition.output)?),
            created_at: Unchanged(definition.created_at),
            updated_at: Set(definition.updated_at),
        };
        let model = model.update(transaction).await.map_err(repository_error)?;
        to_domain(model)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<bool, SubagentDefinitionRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        subagent_definition::Entity::delete_many()
            .filter(subagent_definition::Column::Key.eq(key))
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, SubagentDefinitionRepositoryError> {
    postgres_transaction_ref(transaction)
        .ok_or(SubagentDefinitionRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> SubagentDefinitionRepositoryError {
    persistence_error(error).into()
}

fn to_json<T: Serialize>(
    field: &'static str,
    value: &T,
) -> Result<Value, SubagentDefinitionRepositoryError> {
    serde_json::to_value(value)
        .map_err(|source| SubagentDefinitionRepositoryError::InvalidStoredValue { field, source })
}

fn from_json<T: DeserializeOwned>(
    field: &'static str,
    value: Value,
) -> Result<T, SubagentDefinitionRepositoryError> {
    serde_json::from_value(value)
        .map_err(|source| SubagentDefinitionRepositoryError::InvalidStoredValue { field, source })
}

fn to_domain(
    model: subagent_definition::Model,
) -> Result<SubagentDefinition, SubagentDefinitionRepositoryError> {
    Ok(SubagentDefinition {
        id: model.id,
        key: model.key,
        description: model.description,
        model: model.model,
        reasoning_effort: model.reasoning_effort,
        prompt: model.prompt,
        tools: from_json("tools", model.tools)?,
        timeout_minutes: model.timeout_minutes,
        max_model_calls: model.max_model_calls,
        tool_limits: from_json("tool_limits", model.tool_limits)?,
        output: from_json::<Map<String, Value>>("output", model.output)?,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}
