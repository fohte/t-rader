use core_application::strategy_execution::{
    DatasetCaptureOptions, MAX_DATASET_PAGE_SIZE, StrategyExecutionUseCaseError,
};
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::dto::{
    CaptureDatasetParams, CaptureDatasetResult, ExecutionPayloadRefDto,
    ReadTaskExecutionSnapshotResult,
};
use super::{StrategyServer, internal_error, invalid_params};

impl StrategyServer {
    pub(crate) async fn capture_dataset_inner(
        &self,
        scope: StrategyScope,
        a2a_task_id: &str,
        execution_step_id: uuid::Uuid,
        params: CaptureDatasetParams,
        cancellation: &CancellationToken,
    ) -> Result<CaptureDatasetResult, McpError> {
        let provider_name = params.provider.trim();
        if provider_name.is_empty() {
            return Err(invalid_params("provider must not be empty"));
        }
        let provider = self
            .dependencies
            .dataset_providers
            .get(provider_name)
            .ok_or_else(|| invalid_params("dataset provider is not registered"))?;
        let access = self
            .dependencies
            .strategy_executions
            .authorize_execution(scope, a2a_task_id, Some(execution_step_id))
            .await
            .map_err(strategy_execution_error)?;
        let payload = self
            .dependencies
            .strategy_executions
            .capture_dataset(
                scope,
                &access,
                provider_name.to_string(),
                provider.as_ref(),
                Value::Object(params.params),
                DatasetCaptureOptions {
                    page_size: params.page_size.unwrap_or(MAX_DATASET_PAGE_SIZE),
                    cancellation,
                },
            )
            .await
            .map_err(strategy_execution_error)?;
        Ok(CaptureDatasetResult {
            payload: ExecutionPayloadRefDto::from(payload),
        })
    }

    pub(crate) async fn read_task_execution_snapshot_inner(
        &self,
        scope: StrategyScope,
        a2a_task_id: &str,
    ) -> Result<ReadTaskExecutionSnapshotResult, McpError> {
        let access = self
            .dependencies
            .strategy_executions
            .authorize_execution(scope, a2a_task_id, None)
            .await
            .map_err(strategy_execution_error)?;
        let snapshot = self
            .dependencies
            .strategy_executions
            .read_task_start_snapshot(&access)
            .await
            .map_err(strategy_execution_error)?;
        Ok(match snapshot {
            Some(snapshot) => ReadTaskExecutionSnapshotResult {
                definition: Some(snapshot.definition),
                digest: Some(snapshot.digest),
            },
            None => ReadTaskExecutionSnapshotResult {
                definition: None,
                digest: None,
            },
        })
    }
}

fn strategy_execution_error(error: StrategyExecutionUseCaseError) -> McpError {
    match error {
        error @ (StrategyExecutionUseCaseError::TaskNotFound
        | StrategyExecutionUseCaseError::StrategyMismatch
        | StrategyExecutionUseCaseError::TaskNotRunning
        | StrategyExecutionUseCaseError::InvalidPageLimit
        | StrategyExecutionUseCaseError::InvalidDatasetCursor
        | StrategyExecutionUseCaseError::DatasetCaptureIncomplete
        | StrategyExecutionUseCaseError::SourceRevisionChanged
        | StrategyExecutionUseCaseError::InvalidDatasetCompletion) => {
            invalid_params(error.to_string())
        }
        error => {
            tracing::error!(error = %error, "strategy graph execution failed");
            internal_error("strategy graph execution failed")
        }
    }
}
