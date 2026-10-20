use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use uuid::Uuid;

use crate::unit_of_work::UnitOfWorkTransaction;

use super::error::AgentConfigRepositoryError;
use super::types::{AgentConfig, NewAgentConfig};

#[async_trait]
pub trait AgentConfigRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<AgentConfig>, AgentConfigRepositoryError>;

    async fn list_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<AgentConfig>, AgentConfigRepositoryError>;

    async fn find_by_purpose(
        &self,
        purpose: &str,
    ) -> Result<Option<AgentConfig>, AgentConfigRepositoryError>;

    async fn find_by_purpose_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        purpose: &str,
    ) -> Result<Option<AgentConfig>, AgentConfigRepositoryError>;

    async fn find_by_purpose_for_update_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        purpose: &str,
    ) -> Result<Option<AgentConfig>, AgentConfigRepositoryError>;

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        agent_config: NewAgentConfig,
    ) -> Result<AgentConfig, AgentConfigRepositoryError>;

    async fn update_agents_md(
        &self,
        transaction: &UnitOfWorkTransaction,
        agent_config_id: Uuid,
        agents_md: String,
        updated_at: DateTime<FixedOffset>,
    ) -> Result<AgentConfig, AgentConfigRepositoryError>;

    async fn update_skills(
        &self,
        transaction: &UnitOfWorkTransaction,
        agent_config_id: Uuid,
        skills: Value,
        updated_at: DateTime<FixedOffset>,
    ) -> Result<AgentConfig, AgentConfigRepositoryError>;

    async fn update_agent_graph(
        &self,
        transaction: &UnitOfWorkTransaction,
        agent_config_id: Uuid,
        agent_graph: String,
        updated_at: DateTime<FixedOffset>,
    ) -> Result<AgentConfig, AgentConfigRepositoryError>;

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        purpose: &str,
    ) -> Result<bool, AgentConfigRepositoryError>;
}

pub type SharedAgentConfigRepository = std::sync::Arc<dyn AgentConfigRepository + Send + Sync>;
