use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::unit_of_work::UnitOfWorkTransaction;

use super::error::StrategyExecutionRepositoryError;
use super::types::{
    DefinitionSnapshot, PayloadCaptureMetadata, PayloadChunk, PayloadManifest, PayloadSeal,
};

#[async_trait]
pub trait StrategyExecutionRepository: Send + Sync {
    async fn find_definition_snapshot(
        &self,
        task_id: Uuid,
        definition_key: &str,
    ) -> Result<Option<DefinitionSnapshot>, StrategyExecutionRepositoryError>;

    async fn list_definition_snapshots(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<DefinitionSnapshot>, StrategyExecutionRepositoryError>;

    async fn insert_definition_snapshot(
        &self,
        transaction: &UnitOfWorkTransaction,
        snapshot: DefinitionSnapshot,
    ) -> Result<DefinitionSnapshot, StrategyExecutionRepositoryError>;

    async fn insert_payload(
        &self,
        transaction: &UnitOfWorkTransaction,
        payload: PayloadManifest,
    ) -> Result<(), StrategyExecutionRepositoryError>;

    async fn find_payload_for_update(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        payload_id: Uuid,
    ) -> Result<Option<PayloadManifest>, StrategyExecutionRepositoryError>;

    async fn list_payload_chunks_for_update(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        payload_id: Uuid,
    ) -> Result<Vec<PayloadChunk>, StrategyExecutionRepositoryError>;

    async fn append_payload_chunk(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        chunk: PayloadChunk,
    ) -> Result<PayloadManifest, StrategyExecutionRepositoryError>;

    async fn update_payload_capture_metadata(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        payload_id: Uuid,
        metadata: PayloadCaptureMetadata,
    ) -> Result<(), StrategyExecutionRepositoryError>;

    async fn mark_payload_sealed(
        &self,
        transaction: &UnitOfWorkTransaction,
        seal: PayloadSeal,
    ) -> Result<PayloadManifest, StrategyExecutionRepositoryError>;

    async fn mark_payload_incomplete(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        payload_id: Uuid,
        reason: String,
    ) -> Result<(), StrategyExecutionRepositoryError>;

    async fn find_payload(
        &self,
        task_id: Uuid,
        payload_id: Uuid,
    ) -> Result<Option<PayloadManifest>, StrategyExecutionRepositoryError>;

    async fn list_payload_chunks(
        &self,
        task_id: Uuid,
        payload_id: Uuid,
    ) -> Result<Vec<PayloadChunk>, StrategyExecutionRepositoryError>;

    async fn list_payload_chunks_for_range(
        &self,
        task_id: Uuid,
        payload_id: Uuid,
        start_item: u64,
        item_count: usize,
    ) -> Result<Vec<PayloadChunk>, StrategyExecutionRepositoryError>;
}

pub type SharedStrategyExecutionRepository = Arc<dyn StrategyExecutionRepository + Send + Sync>;
