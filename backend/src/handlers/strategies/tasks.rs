use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::strategy_scope::{
    StrategyScope, StrategyScopeError, StrategyScopeSourceError,
};
use core_application::strategy_task::{
    GetTaskError, ListTasksError, StrategyTaskRepositoryError, SubmitTaskError, TaskSource,
};
use core_application::unit_of_work::UnitOfWorkError;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{
    StrategyChatRequest, StrategyChatResponse, StrategyTaskStatusResponse, StrategyTaskSummary,
};
use core_application::agent_task_client::AgentTaskError;
pub(crate) fn map_submit_error(err: SubmitTaskError) -> AppError {
    match err {
        SubmitTaskError::EmptyPrompt => AppError::Validation("prompt must not be empty".into()),
        SubmitTaskError::StrategyNotFound(id) => {
            AppError::NotFound(format!("strategy {id} not found"))
        }
        SubmitTaskError::PurposeNotFound(purpose) => {
            AppError::ServiceUnavailable(format!("agent_config for purpose '{purpose}' not found"))
        }
        SubmitTaskError::Repository(StrategyTaskRepositoryError::Database(error))
        | SubmitTaskError::UnitOfWork(UnitOfWorkError::Begin(error))
        | SubmitTaskError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
        SubmitTaskError::Repository(StrategyTaskRepositoryError::InvalidTransaction)
        | SubmitTaskError::UnitOfWork(UnitOfWorkError::InvalidTransaction) => {
            AppError::Internal("invalid strategy task transaction".into())
        }
        SubmitTaskError::AgentTask(AgentTaskError::NotConfigured) => {
            AppError::ServiceUnavailable("agent task client is not configured".into())
        }
        SubmitTaskError::AgentTask(agent_err) => {
            AppError::Config(format!("agent task error: {agent_err}"))
        }
    }
}

pub(crate) fn map_list_task_error(error: ListTasksError) -> AppError {
    match error {
        ListTasksError::Repository(StrategyTaskRepositoryError::Database(error)) => error.into(),
        ListTasksError::Repository(StrategyTaskRepositoryError::InvalidTransaction) => {
            AppError::Internal("invalid strategy task transaction".into())
        }
    }
}

fn map_get_task_error(error: GetTaskError) -> AppError {
    match error {
        GetTaskError::NotFound(id) => AppError::NotFound(format!("strategy task {id} not found")),
        GetTaskError::StrategyMismatch { task_id, .. } => {
            AppError::NotFound(format!("strategy task {task_id} not found"))
        }
        GetTaskError::Repository(StrategyTaskRepositoryError::Database(error)) => error.into(),
        GetTaskError::Repository(StrategyTaskRepositoryError::InvalidTransaction) => {
            AppError::Internal("invalid strategy task transaction".into())
        }
    }
}

async fn verify_strategy_scope(
    state: &AppState,
    strategy_id: Uuid,
    not_found_message: String,
) -> Result<StrategyScope, AppError> {
    state
        .strategy_scope_use_cases
        .verify(strategy_id)
        .await
        .map_err(|error| match error {
            StrategyScopeError::NotFound(_) => AppError::NotFound(not_found_message),
            StrategyScopeError::Source(StrategyScopeSourceError::QueryFailed(message)) => {
                AppError::Internal(message)
            }
        })
}

/// フローティングチャットから戦略 Agent にタスクを投入する
#[utoipa::path(
    post,
    path = "/api/strategies/{id}/chat",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = StrategyChatRequest,
    responses(
        (status = 202, body = StrategyChatResponse),
        (status = 400, description = "prompt が空 (空白のみを含む)、または指定された purpose の agent_config が存在しない", body = ErrorResponse),
        (status = 404, description = "戦略が存在しない", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
        (status = 503, description = "agent task client が未設定、または既定の agent_config が見つからない", body = ErrorResponse),
    )
)]
pub async fn submit_strategy_chat(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<StrategyChatRequest>,
) -> Result<(StatusCode, Json<StrategyChatResponse>), AppError> {
    let requested_purpose = payload.purpose;
    let submitted = state
        .strategy_task_use_cases
        .submit_task(
            state.agent_task_client.as_ref(),
            id,
            &payload.prompt,
            TaskSource::Frontend,
            requested_purpose.clone(),
        )
        .await
        .map_err(|error| match error {
            err @ SubmitTaskError::PurposeNotFound(_) if requested_purpose.is_some() => {
                AppError::Validation(err.to_string())
            }
            error => map_submit_error(error),
        })?;
    Ok((
        StatusCode::ACCEPTED,
        Json(StrategyChatResponse {
            task_id: submitted.task_id,
            a2a_task_id: submitted.a2a_task_id,
        }),
    ))
}

/// 投入済み戦略タスクの phase / error_summary を取得する
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/tasks/{task_id}",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("task_id" = Uuid, Path, description = "戦略タスク ID"),
    ),
    responses(
        (status = 200, body = StrategyTaskStatusResponse),
        (status = 400, description = "パスパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_strategy_task(
    State(state): State<AppState>,
    JsonPath((strategy_id, task_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<Json<StrategyTaskStatusResponse>, AppError> {
    let scope = verify_strategy_scope(
        &state,
        strategy_id,
        format!("strategy task {task_id} not found"),
    )
    .await?;
    let view = state
        .strategy_task_use_cases
        .get_for_strategy(scope, task_id)
        .await
        .map_err(map_get_task_error)?;
    Ok(Json(StrategyTaskStatusResponse {
        task_id: view.task_id,
        strategy_id: view.strategy_id,
        a2a_task_id: view.a2a_task_id,
        source: view.source,
        prompt: view.prompt,
        phase: view.phase.as_str().to_string(),
        error_summary: view.error_summary,
        result_text: view.result_text,
        created_at: view.created_at,
        updated_at: view.updated_at,
        steps: view.steps,
        purpose: view.purpose,
        as_of: view.as_of,
    }))
}

/// 戦略の過去タスクを新しい順に一覧取得する
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/tasks",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = Vec<StrategyTaskSummary>),
        (status = 400, description = "パスパラメータが不正", body = ErrorResponse),
        (status = 404, description = "戦略が存在しない", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_tasks(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
) -> Result<Json<Vec<StrategyTaskSummary>>, AppError> {
    let scope = verify_strategy_scope(
        &state,
        strategy_id,
        format!("strategy {strategy_id} not found"),
    )
    .await?;
    let views = state
        .strategy_task_use_cases
        .list_for_strategy(scope, None)
        .await
        .map_err(map_list_task_error)?;
    Ok(Json(
        views.into_iter().map(StrategyTaskSummary::from).collect(),
    ))
}
