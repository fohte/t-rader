use core_application::strategy_execution::{
    PayloadCaptureMetadata, PayloadChunk, PayloadKind, PayloadManifest, PayloadStatus,
    StrategyExecutionRepositoryError,
};

use crate::entities::{task_execution_payload, task_execution_payload_chunk};

impl TryFrom<task_execution_payload::Model> for PayloadManifest {
    type Error = StrategyExecutionRepositoryError;

    fn try_from(model: task_execution_payload::Model) -> Result<Self, Self::Error> {
        let kind = match model.kind.as_str() {
            "input" => PayloadKind::Input,
            "output" => PayloadKind::Output,
            _ => return Err(StrategyExecutionRepositoryError::InconsistentPayload),
        };
        let status = match model.status.as_str() {
            "staging" => PayloadStatus::Staging,
            "sealed" => PayloadStatus::Sealed,
            "incomplete" => PayloadStatus::Incomplete,
            _ => return Err(StrategyExecutionRepositoryError::InconsistentPayload),
        };
        Ok(Self {
            id: model.id,
            task_id: model.task_id,
            execution_step_id: model.execution_step_id,
            kind,
            source: model.source,
            source_revision: model.source_revision,
            source_locator: model.source_locator,
            source_metadata: model.source_metadata,
            schema: model.schema,
            capture_metadata: model
                .capture_metadata
                .map(serde_json::from_value::<PayloadCaptureMetadata>)
                .transpose()
                .map_err(|_| StrategyExecutionRepositoryError::InconsistentPayload)?,
            root_is_array: model.root_is_array,
            chunk_count: u64::try_from(model.chunk_count)
                .map_err(|_| StrategyExecutionRepositoryError::InconsistentPayload)?,
            item_count: u64::try_from(model.item_count)
                .map_err(|_| StrategyExecutionRepositoryError::InconsistentPayload)?,
            byte_count: u64::try_from(model.byte_count)
                .map_err(|_| StrategyExecutionRepositoryError::InconsistentPayload)?,
            digest: model.digest,
            status,
            incomplete_reason: model.incomplete_reason,
            created_at: model.created_at,
            sealed_at: model.sealed_at,
        })
    }
}

impl TryFrom<task_execution_payload_chunk::Model> for PayloadChunk {
    type Error = StrategyExecutionRepositoryError;

    fn try_from(model: task_execution_payload_chunk::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            payload_id: model.payload_id,
            sequence: u64::try_from(model.sequence)
                .map_err(|_| StrategyExecutionRepositoryError::InconsistentPayload)?,
            start_item: u64::try_from(model.start_item)
                .map_err(|_| StrategyExecutionRepositoryError::InconsistentPayload)?,
            data: model.data,
            byte_count: u64::try_from(model.byte_count)
                .map_err(|_| StrategyExecutionRepositoryError::InconsistentPayload)?,
            item_count: u64::try_from(model.item_count)
                .map_err(|_| StrategyExecutionRepositoryError::InconsistentPayload)?,
        })
    }
}
