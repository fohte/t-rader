use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use core_application::agent_config::{
    AgentConfig, AgentConfigRepository, AgentConfigRepositoryError, NewAgentConfig,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set, Unchanged};
use sea_orm::sea_query::LockType;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::Value;
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::agent_config;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

#[derive(Clone)]
pub struct PostgresAgentConfigRepository {
    db: DatabaseHandle,
}

impl PostgresAgentConfigRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AgentConfigRepository for PostgresAgentConfigRepository {
    async fn list(&self) -> Result<Vec<AgentConfig>, AgentConfigRepositoryError> {
        agent_config::Entity::find()
            .order_by_asc(agent_config::Column::Purpose)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_domain).collect())
            .map_err(repository_error)
    }

    async fn list_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<AgentConfig>, AgentConfigRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        agent_config::Entity::find()
            .order_by_asc(agent_config::Column::Purpose)
            .all(transaction)
            .await
            .map(|rows| rows.into_iter().map(to_domain).collect())
            .map_err(repository_error)
    }

    async fn find_by_purpose(
        &self,
        purpose: &str,
    ) -> Result<Option<AgentConfig>, AgentConfigRepositoryError> {
        agent_config::Entity::find()
            .filter(agent_config::Column::Purpose.eq(purpose))
            .one(&self.db)
            .await
            .map(|row| row.map(to_domain))
            .map_err(repository_error)
    }

    async fn find_by_purpose_for_update_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        purpose: &str,
    ) -> Result<Option<AgentConfig>, AgentConfigRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        agent_config::Entity::find()
            .filter(agent_config::Column::Purpose.eq(purpose))
            .lock(LockType::Update)
            .one(transaction)
            .await
            .map(|row| row.map(to_domain))
            .map_err(repository_error)
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        new_agent_config: NewAgentConfig,
    ) -> Result<AgentConfig, AgentConfigRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = agent_config::ActiveModel {
            id: Set(new_agent_config.id),
            purpose: Set(new_agent_config.purpose),
            agents_md: NotSet,
            skills: NotSet,
            agent_graph: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        };
        agent_config::Entity::insert(model)
            .exec_with_returning(transaction)
            .await
            .map(to_domain)
            .map_err(repository_error)
    }

    async fn update_agents_md(
        &self,
        transaction: &UnitOfWorkTransaction,
        agent_config_id: Uuid,
        agents_md: String,
        updated_at: DateTime<FixedOffset>,
    ) -> Result<AgentConfig, AgentConfigRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = agent_config::ActiveModel {
            id: Unchanged(agent_config_id),
            agents_md: Set(agents_md),
            updated_at: Set(updated_at),
            ..Default::default()
        };
        model
            .update(transaction)
            .await
            .map(to_domain)
            .map_err(repository_error)
    }

    async fn update_skills(
        &self,
        transaction: &UnitOfWorkTransaction,
        agent_config_id: Uuid,
        skills: Value,
        updated_at: DateTime<FixedOffset>,
    ) -> Result<AgentConfig, AgentConfigRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = agent_config::ActiveModel {
            id: Unchanged(agent_config_id),
            skills: Set(skills),
            updated_at: Set(updated_at),
            ..Default::default()
        };
        model
            .update(transaction)
            .await
            .map(to_domain)
            .map_err(repository_error)
    }

    async fn update_agent_graph(
        &self,
        transaction: &UnitOfWorkTransaction,
        agent_config_id: Uuid,
        agent_graph: String,
        updated_at: DateTime<FixedOffset>,
    ) -> Result<AgentConfig, AgentConfigRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = agent_config::ActiveModel {
            id: Unchanged(agent_config_id),
            agent_graph: Set(agent_graph),
            updated_at: Set(updated_at),
            ..Default::default()
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
        purpose: &str,
    ) -> Result<bool, AgentConfigRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        agent_config::Entity::delete_many()
            .filter(agent_config::Column::Purpose.eq(purpose))
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, AgentConfigRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(AgentConfigRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> AgentConfigRepositoryError {
    persistence_error(error).into()
}

fn to_domain(model: agent_config::Model) -> AgentConfig {
    AgentConfig {
        id: model.id,
        purpose: model.purpose,
        agents_md: model.agents_md,
        skills: model.skills,
        agent_graph: model.agent_graph,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}
