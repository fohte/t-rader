use async_trait::async_trait;

use crate::unit_of_work::UnitOfWorkTransaction;

use super::error::AgentConfigRepositoryError;
use super::types::{AgentConfig, NewAgentConfig};

#[async_trait]
pub trait AgentConfigRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<AgentConfig>, AgentConfigRepositoryError>;

    async fn find_by_purpose(
        &self,
        purpose: &str,
    ) -> Result<Option<AgentConfig>, AgentConfigRepositoryError>;

    async fn find_by_purpose_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        purpose: &str,
    ) -> Result<Option<AgentConfig>, AgentConfigRepositoryError>;

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        agent_config: NewAgentConfig,
    ) -> Result<AgentConfig, AgentConfigRepositoryError>;

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        agent_config: AgentConfig,
    ) -> Result<AgentConfig, AgentConfigRepositoryError>;

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        purpose: &str,
    ) -> Result<bool, AgentConfigRepositoryError>;
}

pub type SharedAgentConfigRepository = std::sync::Arc<dyn AgentConfigRepository + Send + Sync>;
