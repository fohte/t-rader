use async_trait::async_trait;

use crate::unit_of_work::UnitOfWorkTransaction;

use super::error::SubagentDefinitionRepositoryError;
use super::types::SubagentDefinition;

#[async_trait]
pub trait SubagentDefinitionRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<SubagentDefinition>, SubagentDefinitionRepositoryError>;

    async fn find_by_key(
        &self,
        key: &str,
    ) -> Result<Option<SubagentDefinition>, SubagentDefinitionRepositoryError>;

    async fn find_by_key_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<Option<SubagentDefinition>, SubagentDefinitionRepositoryError>;

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        definition: SubagentDefinition,
    ) -> Result<SubagentDefinition, SubagentDefinitionRepositoryError>;

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        definition: SubagentDefinition,
    ) -> Result<SubagentDefinition, SubagentDefinitionRepositoryError>;

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<bool, SubagentDefinitionRepositoryError>;
}

pub type SharedSubagentDefinitionRepository =
    std::sync::Arc<dyn SubagentDefinitionRepository + Send + Sync>;
