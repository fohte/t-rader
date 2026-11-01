use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::strategy_scope::{
    StrategyScope, StrategyScopeError, StrategyScopeSourceError,
};
use core_application::strategy_task::{
    GetTaskError, ListTasksError, StrategyTaskContext, StrategyTaskRepositoryError,
    SubmitTaskError, TaskSource,
};
use core_application::unit_of_work::UnitOfWorkError;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::handlers::notes::map_note_read_error;
use crate::models::{
    NoteResponse, StrategyChatContextRequest, StrategyChatRequest, StrategyChatResponse,
    StrategyTaskContextKind, StrategyTaskStatusResponse, StrategyTaskSummary,
};
use core_application::agent_task_client::AgentTaskError;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListStrategyTasksQuery {
    pub context_kind: Option<StrategyTaskContextKind>,
    pub context_id: Option<String>,
}

fn paired_context(
    kind: Option<StrategyTaskContextKind>,
    id: Option<String>,
) -> Result<Option<StrategyTaskContext>, AppError> {
    match (kind, id) {
        (None, None) => Ok(None),
        (Some(kind), Some(id)) if !id.trim().is_empty() => Ok(Some(StrategyTaskContext {
            kind: kind.as_str().to_string(),
            id: id.trim().to_string(),
        })),
        (Some(_), Some(_)) => Err(AppError::Validation("context_id must not be empty".into())),
        _ => Err(AppError::Validation(
            "context_kind and context_id must be provided together".into(),
        )),
    }
}

async fn chat_prompt_and_context(
    state: &FrontendApiState,
    prompt: &str,
    context: Option<StrategyChatContextRequest>,
) -> Result<(String, Option<StrategyTaskContext>), AppError> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(map_submit_error(SubmitTaskError::EmptyPrompt));
    }
    let Some(context) = context else {
        return Ok((prompt.to_string(), None));
    };
    let id = context.id.trim();
    if id.is_empty() {
        return Err(AppError::Validation("context.id must not be empty".into()));
    }

    let (context_id, reference) = match context.kind {
        StrategyTaskContextKind::Note => {
            let id = Uuid::parse_str(id)
                .map_err(|_| AppError::Validation("context.id must be a note UUID".into()))?;
            (id.to_string(), format!("[[note:{id}]]"))
        }
        StrategyTaskContextKind::Stock => {
            let valid_stock_id = match id.split_once(':') {
                Some((country, code)) => {
                    core_domain::stock_id::ForeignStockId::new(country, code).is_ok()
                }
                None => id
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-'),
            };
            if !valid_stock_id {
                return Err(AppError::Validation(
                    "context.id must be a valid stock ID".into(),
                ));
            }
            (id.to_string(), format!("[[stock:{id}]]"))
        }
        StrategyTaskContextKind::Trade => {
            let id = Uuid::parse_str(id)
                .map_err(|_| AppError::Validation("context.id must be a trade UUID".into()))?;
            let trade = state
                .trade_use_cases
                .get(id)
                .await
                .map_err(crate::handlers::trades::map_trade_error)?;
            let reference = format!(
                "取引 ID={}, 約定日={}, 銘柄={}, 売買={}, 数量={}, 単価={}",
                trade.id, trade.date, trade.symbol, trade.side, trade.qty, trade.price
            );
            (trade.id.to_string(), reference)
        }
    };

    let kind = context.kind.as_str().to_string();
    let prompt = indoc::formatdoc! {"
        文脈: {reference}

        {}
    ", prompt.trim()}
    .trim_end()
    .to_string();
    Ok((
        prompt,
        Some(StrategyTaskContext {
            kind,
            id: context_id,
        }),
    ))
}
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
        SubmitTaskError::ExecutionSnapshot(error) => {
            tracing::error!(error = %error, "failed to capture task execution definitions");
            AppError::Internal("failed to capture task execution definitions".into())
        }
        SubmitTaskError::ExecutionBudget(error) => {
            tracing::error!(error = %error, "failed to initialize task execution budget");
            AppError::Internal("failed to initialize task execution budget".into())
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

pub(crate) fn map_get_task_error(error: GetTaskError) -> AppError {
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
    state: &FrontendApiState,
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
        (status = 400, description = "prompt が空、context が不正、または指定された purpose の agent_config が存在しない", body = ErrorResponse),
        (status = 404, description = "戦略または context の取引が存在しない", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
        (status = 503, description = "agent task client が未設定、または既定の agent_config が見つからない", body = ErrorResponse),
    )
)]
pub async fn submit_strategy_chat(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<StrategyChatRequest>,
) -> Result<(StatusCode, Json<StrategyChatResponse>), AppError> {
    let requested_purpose = payload.purpose;
    let (prompt, context) =
        chat_prompt_and_context(&state, &payload.prompt, payload.context).await?;
    let submitted = state
        .strategy_task_use_cases
        .submit_task_with_context(
            state.agent_task_client.as_ref(),
            id,
            &prompt,
            TaskSource::Frontend,
            requested_purpose.clone(),
            context,
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
    State(state): State<FrontendApiState>,
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
        model_call_limit_reached: view.model_call_limit_reached,
        steps: view.steps,
        purpose: view.purpose,
        as_of: view.as_of,
        context_kind: view.context_kind,
        context_id: view.context_id,
    }))
}

/// 戦略タスクが書いたノートを現行バージョンで返す。
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/tasks/{task_id}/notes",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("task_id" = Uuid, Path, description = "戦略タスク ID"),
    ),
    responses(
        (status = 200, body = Vec<NoteResponse>),
        (status = 400, description = "パスパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_task_notes(
    State(state): State<FrontendApiState>,
    JsonPath((strategy_id, task_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<Json<Vec<NoteResponse>>, AppError> {
    let scope = verify_strategy_scope(
        &state,
        strategy_id,
        format!("strategy task {task_id} not found"),
    )
    .await?;
    state
        .strategy_task_use_cases
        .get_for_strategy(scope, task_id)
        .await
        .map_err(map_get_task_error)?;
    let snapshots = state
        .note_read_use_cases
        .list_notes_written_by_task(task_id)
        .await
        .map_err(map_note_read_error)?;
    Ok(Json(
        snapshots
            .into_iter()
            .map(NoteResponse::from_snapshot)
            .collect(),
    ))
}

/// 戦略の過去タスクを新しい順に一覧取得する
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/tasks",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ListStrategyTasksQuery
    ),
    responses(
        (status = 200, body = Vec<StrategyTaskSummary>),
        (status = 400, description = "パスパラメータまたは文脈フィルターが不正", body = ErrorResponse),
        (status = 404, description = "戦略が存在しない", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_tasks(
    State(state): State<FrontendApiState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
    JsonQuery(query): JsonQuery<ListStrategyTasksQuery>,
) -> Result<Json<Vec<StrategyTaskSummary>>, AppError> {
    let scope = verify_strategy_scope(
        &state,
        strategy_id,
        format!("strategy {strategy_id} not found"),
    )
    .await?;
    let context = paired_context(query.context_kind, query.context_id)?;
    let views = state
        .strategy_task_use_cases
        .list_for_strategy(scope, None, context)
        .await
        .map_err(map_list_task_error)?;
    Ok(Json(
        views.into_iter().map(StrategyTaskSummary::from).collect(),
    ))
}
