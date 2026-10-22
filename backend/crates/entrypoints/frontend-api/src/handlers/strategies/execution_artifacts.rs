use axum::Json;
use axum::extract::State;
use core_application::strategy_execution::{
    DEFAULT_PAYLOAD_MANIFEST_PAGE_ITEMS, DEFAULT_PAYLOAD_PAGE_ITEMS, ListPayloadManifestsQuery,
    PayloadManifestCursor, ReadPayloadQuery, StrategyExecutionRepositoryError,
    StrategyExecutionUseCaseError,
};
use core_application::strategy_task::StrategyTaskRepositoryError;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonPath, JsonQuery};
use crate::handlers::strategies::strategy_scope_or_404;
use crate::models::{
    ExecutionArtifactsResponse, ExecutionPayloadManifestPageResponse, ExecutionPayloadResponse,
};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListExecutionPayloadManifestsQuery {
    pub cursor: Option<String>,
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ReadExecutionPayloadQuery {
    pub pointer: Option<String>,
    pub cursor: Option<u64>,
    #[param(minimum = 1, maximum = 1000)]
    pub limit: Option<usize>,
}

fn map_execution_read_error(error: StrategyExecutionUseCaseError) -> AppError {
    match error {
        StrategyExecutionUseCaseError::TaskNotFound
        | StrategyExecutionUseCaseError::StrategyMismatch
        | StrategyExecutionUseCaseError::PayloadNotFound => {
            AppError::NotFound("task execution artifact not found".into())
        }
        StrategyExecutionUseCaseError::InvalidPayloadPointer
        | StrategyExecutionUseCaseError::InvalidPageLimit
        | StrategyExecutionUseCaseError::InvalidPayloadManifestPageLimit
        | StrategyExecutionUseCaseError::PayloadPageTooLarge => {
            AppError::Validation("invalid execution payload page request".into())
        }
        StrategyExecutionUseCaseError::Repository(StrategyExecutionRepositoryError::Database(
            error,
        ))
        | StrategyExecutionUseCaseError::TaskRepository(StrategyTaskRepositoryError::Database(
            error,
        )) => error.into(),
        error => AppError::Internal(error.to_string()),
    }
}

fn parse_manifest_cursor(
    cursor: Option<String>,
) -> Result<Option<PayloadManifestCursor>, AppError> {
    cursor
        .map(|cursor| {
            let (created_at, id) = cursor.split_once('|').ok_or_else(|| {
                AppError::Validation("invalid execution payload manifest cursor".into())
            })?;
            let created_at = chrono::DateTime::parse_from_rfc3339(created_at).map_err(|_| {
                AppError::Validation("invalid execution payload manifest cursor".into())
            })?;
            let id = Uuid::parse_str(id).map_err(|_| {
                AppError::Validation("invalid execution payload manifest cursor".into())
            })?;
            Ok(PayloadManifestCursor { created_at, id })
        })
        .transpose()
}

/// task 開始時点で固定した定義を取得する。
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/tasks/{task_id}/execution-artifacts",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("task_id" = Uuid, Path, description = "戦略タスク ID"),
    ),
    responses(
        (status = 200, body = ExecutionArtifactsResponse),
        (status = 400, description = "パスパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_task_execution_artifacts(
    State(state): State<FrontendApiState>,
    JsonPath((strategy_id, task_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<Json<ExecutionArtifactsResponse>, AppError> {
    let scope = strategy_scope_or_404(&state, strategy_id).await?;
    let snapshots = state
        .strategy_execution_use_cases
        .list_task_execution_definitions(scope, task_id)
        .await
        .map_err(map_execution_read_error)?;
    Ok(Json(ExecutionArtifactsResponse::new(task_id, snapshots)))
}

/// task に紐づく payload の固定 manifest と有限 page を取得する。
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/tasks/{task_id}/execution-artifacts/payloads/{payload_id}",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("task_id" = Uuid, Path, description = "戦略タスク ID"),
        ("payload_id" = Uuid, Path, description = "payload ID"),
        ReadExecutionPayloadQuery,
    ),
    responses(
        (status = 200, body = ExecutionPayloadResponse),
        (status = 400, description = "パスまたは page パラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_task_execution_payload(
    State(state): State<FrontendApiState>,
    JsonPath((strategy_id, task_id, payload_id)): JsonPath<(Uuid, Uuid, Uuid)>,
    JsonQuery(query): JsonQuery<ReadExecutionPayloadQuery>,
) -> Result<Json<ExecutionPayloadResponse>, AppError> {
    let scope = strategy_scope_or_404(&state, strategy_id).await?;
    let payload = state
        .strategy_execution_use_cases
        .read_task_execution_payload(
            scope,
            task_id,
            ReadPayloadQuery {
                payload_id,
                pointer: query.pointer.unwrap_or_default(),
                cursor: query.cursor.unwrap_or_default(),
                limit: query.limit.unwrap_or(DEFAULT_PAYLOAD_PAGE_ITEMS),
            },
        )
        .await
        .map_err(map_execution_read_error)?;
    Ok(Json(ExecutionPayloadResponse::from(payload)))
}

/// task に属する payload manifest を安定した cursor で一覧取得する。
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/tasks/{task_id}/execution-artifacts/payloads",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("task_id" = Uuid, Path, description = "戦略タスク ID"),
        ListExecutionPayloadManifestsQuery,
    ),
    responses(
        (status = 200, body = ExecutionPayloadManifestPageResponse),
        (status = 400, description = "パスまたは page パラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_task_execution_payload_manifests(
    State(state): State<FrontendApiState>,
    JsonPath((strategy_id, task_id)): JsonPath<(Uuid, Uuid)>,
    JsonQuery(query): JsonQuery<ListExecutionPayloadManifestsQuery>,
) -> Result<Json<ExecutionPayloadManifestPageResponse>, AppError> {
    let scope = strategy_scope_or_404(&state, strategy_id).await?;
    let cursor = parse_manifest_cursor(query.cursor)?;
    let page = state
        .strategy_execution_use_cases
        .list_task_execution_payload_manifests(
            scope,
            task_id,
            ListPayloadManifestsQuery {
                cursor,
                limit: query.limit.unwrap_or(DEFAULT_PAYLOAD_MANIFEST_PAGE_ITEMS),
            },
        )
        .await
        .map_err(map_execution_read_error)?;
    Ok(Json(ExecutionPayloadManifestPageResponse::new(
        task_id, page,
    )))
}
