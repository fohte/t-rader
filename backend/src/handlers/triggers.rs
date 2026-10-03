use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::trigger::{
    CreateTriggerCommand, TriggerKind as ApplicationTriggerKind, TriggerUseCaseError,
    UpdateTriggerCommand,
};
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::models::{
    CreateTriggerRequest, ListTriggersQuery, TriggerKind, TriggerResponse, UpdateTriggerRequest,
};

/// 戦略の trigger 一覧
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/triggers",
    tag = "triggers",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("kind" = Option<TriggerKind>, Query, description = "kind フィルタ"),
    ),
    responses(
        (status = 200, body = Vec<TriggerResponse>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_triggers(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonQuery(query): JsonQuery<ListTriggersQuery>,
) -> Result<Json<Vec<TriggerResponse>>, AppError> {
    let scope = super::strategies::strategy_scope_or_404(&state, id).await?;
    let kind = query.kind.map(application_kind);
    let items = state
        .trigger_use_cases
        .list_for_strategy(scope, kind)
        .await
        .map_err(map_trigger_use_case_error)?;
    let items = items.into_iter().map(TriggerResponse::from).collect();
    Ok(Json(items))
}

/// trigger を作成
#[utoipa::path(
    post,
    path = "/api/strategies/{id}/triggers",
    tag = "triggers",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = CreateTriggerRequest,
    responses(
        (status = 201, body = TriggerResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "hook_slug が他 trigger と衝突", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_strategy_trigger(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<CreateTriggerRequest>,
) -> Result<(StatusCode, Json<TriggerResponse>), AppError> {
    let scope = super::strategies::strategy_scope_or_404(&state, strategy_id).await?;
    let created = state
        .trigger_use_cases
        .create(
            scope,
            CreateTriggerCommand {
                purpose: payload.purpose,
                kind: application_kind(payload.kind),
                schedule: payload.schedule,
                hook_slug: payload.hook_slug,
                event_match: payload.event_match,
                prompt_template: payload.prompt_template,
                enabled: payload.enabled,
            },
        )
        .await
        .map_err(map_trigger_use_case_error)?;
    Ok((StatusCode::CREATED, Json(created.into())))
}

/// trigger 詳細
#[utoipa::path(
    get,
    path = "/api/triggers/{trigger_id}",
    tag = "triggers",
    params(("trigger_id" = Uuid, Path, description = "trigger ID")),
    responses(
        (status = 200, body = TriggerResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_trigger(
    State(state): State<AppState>,
    JsonPath(trigger_id): JsonPath<Uuid>,
) -> Result<Json<TriggerResponse>, AppError> {
    let trigger = state
        .trigger_use_cases
        .get(trigger_id)
        .await
        .map_err(map_trigger_use_case_error)?;
    Ok(Json(trigger.into()))
}

/// trigger 更新 (kind / strategy_id は不変)
#[utoipa::path(
    put,
    path = "/api/triggers/{trigger_id}",
    tag = "triggers",
    params(("trigger_id" = Uuid, Path, description = "trigger ID")),
    request_body = UpdateTriggerRequest,
    responses(
        (status = 200, body = TriggerResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "hook_slug が他 trigger と衝突", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_trigger(
    State(state): State<AppState>,
    JsonPath(trigger_id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateTriggerRequest>,
) -> Result<Json<TriggerResponse>, AppError> {
    let current = state
        .trigger_use_cases
        .get(trigger_id)
        .await
        .map_err(map_trigger_use_case_error)?;
    let strategy_id = current
        .strategy_id
        .ok_or_else(|| AppError::NotFound(format!("trigger {trigger_id} not found")))?;
    let scope = super::strategies::strategy_scope_or_404(&state, strategy_id).await?;
    let updated = state
        .trigger_use_cases
        .update(
            scope,
            trigger_id,
            UpdateTriggerCommand {
                purpose: payload.purpose,
                schedule: payload.schedule,
                hook_slug: payload.hook_slug,
                event_match: payload.event_match,
                prompt_template: payload.prompt_template,
                enabled: payload.enabled,
            },
        )
        .await
        .map_err(map_trigger_use_case_error)?;
    Ok(Json(updated.into()))
}

/// trigger 削除
#[utoipa::path(
    delete,
    path = "/api/triggers/{trigger_id}",
    tag = "triggers",
    params(("trigger_id" = Uuid, Path, description = "trigger ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_trigger(
    State(state): State<AppState>,
    JsonPath(trigger_id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let current = state
        .trigger_use_cases
        .get(trigger_id)
        .await
        .map_err(map_trigger_use_case_error)?;
    let strategy_id = current
        .strategy_id
        .ok_or_else(|| AppError::NotFound(format!("trigger {trigger_id} not found")))?;
    let scope = super::strategies::strategy_scope_or_404(&state, strategy_id).await?;
    state
        .trigger_use_cases
        .delete(scope, trigger_id)
        .await
        .map_err(map_trigger_use_case_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn application_kind(kind: TriggerKind) -> ApplicationTriggerKind {
    match kind {
        TriggerKind::Cron => ApplicationTriggerKind::Cron,
        TriggerKind::Hook => ApplicationTriggerKind::Hook,
    }
}

pub(crate) fn map_trigger_use_case_error(error: TriggerUseCaseError) -> AppError {
    match error {
        TriggerUseCaseError::Validation(message) => AppError::Validation(message),
        TriggerUseCaseError::NotFound(id) => AppError::NotFound(format!("trigger {id} not found")),
        TriggerUseCaseError::PurposeNotFound(purpose) => {
            AppError::NotFound(format!("agent_config purpose {purpose} not found"))
        }
        TriggerUseCaseError::HookNotFound(slug) => {
            AppError::NotFound(format!("hook {slug} not found"))
        }
        TriggerUseCaseError::Disabled(id) | TriggerUseCaseError::NoStrategy(id) => {
            AppError::NotFound(format!("trigger {id} not found"))
        }
        TriggerUseCaseError::Submit(error) => super::strategies::map_submit_error(error),
        TriggerUseCaseError::Repository(
            core_application::trigger::TriggerRepositoryError::Database(error),
        )
        | TriggerUseCaseError::StrategyRepository(
            core_application::strategy::StrategyRepositoryError::Database(error),
        )
        | TriggerUseCaseError::StrategyExistence(
            core_application::strategy_existence::StrategyExistenceError::Database(error),
        )
        | TriggerUseCaseError::UnitOfWork(
            core_application::unit_of_work::UnitOfWorkError::Begin(error)
            | core_application::unit_of_work::UnitOfWorkError::Commit(error),
        ) => error.into(),
        other => AppError::Internal(other.to_string()),
    }
}
