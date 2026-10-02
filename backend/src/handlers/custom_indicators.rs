use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::change_history::ChangeHistoryError;
use core_application::custom_indicator::{
    CreateCustomIndicatorCommand, CustomIndicatorRepositoryError, CustomIndicatorUseCaseError,
    UpdateCustomIndicatorCommand,
};
use core_application::strategy_existence::StrategyExistenceError;
use core_application::unit_of_work::UnitOfWorkError;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{
    CreateCustomIndicatorRequest, CustomIndicatorResponse, PreviewIndicatorRequest,
    PreviewIndicatorResponse, UpdateCustomIndicatorRequest,
};
use crate::services::custom_indicators::{PreviewInput, run_preview};

fn ensure_json_object(field: &str, value: &serde_json::Value) -> Result<(), AppError> {
    if value.is_object() {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "{field} must be a JSON object"
        )))
    }
}

/// グローバル indicator 一覧
#[utoipa::path(
    get,
    path = "/api/indicators",
    tag = "custom_indicators",
    responses(
        (status = 200, body = Vec<CustomIndicatorResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_global_indicators(
    State(state): State<AppState>,
) -> Result<Json<Vec<CustomIndicatorResponse>>, AppError> {
    let items = state
        .custom_indicator_use_cases
        .list_global()
        .await
        .map_err(map_custom_indicator_error)?;
    Ok(Json(items.into_iter().map(Into::into).collect()))
}

/// 戦略 scope indicator 一覧
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/indicators",
    tag = "custom_indicators",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = Vec<CustomIndicatorResponse>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_indicators(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
) -> Result<Json<Vec<CustomIndicatorResponse>>, AppError> {
    let items = state
        .custom_indicator_use_cases
        .list_strategy(strategy_id)
        .await
        .map_err(map_custom_indicator_error)?;
    Ok(Json(items.into_iter().map(Into::into).collect()))
}

/// indicator 詳細
#[utoipa::path(
    get,
    path = "/api/indicators/{indicator_id}",
    tag = "custom_indicators",
    params(("indicator_id" = Uuid, Path, description = "indicator ID")),
    responses(
        (status = 200, body = CustomIndicatorResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_indicator(
    State(state): State<AppState>,
    JsonPath(indicator_id): JsonPath<Uuid>,
) -> Result<Json<CustomIndicatorResponse>, AppError> {
    let indicator = state
        .custom_indicator_use_cases
        .get(indicator_id)
        .await
        .map_err(map_custom_indicator_error)?;
    Ok(Json(indicator.into()))
}

/// グローバル indicator 作成
#[utoipa::path(
    post,
    path = "/api/indicators",
    tag = "custom_indicators",
    request_body = CreateCustomIndicatorRequest,
    responses(
        (status = 201, body = CustomIndicatorResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_global_indicator(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateCustomIndicatorRequest>,
) -> Result<(StatusCode, Json<CustomIndicatorResponse>), AppError> {
    let indicator = state
        .custom_indicator_use_cases
        .create(CreateCustomIndicatorCommand {
            name: payload.name,
            strategy_id: None,
            code: payload.code,
            input_schema: payload.input_schema,
            output_schema: payload.output_schema,
            description: payload.description,
        })
        .await
        .map_err(map_custom_indicator_error)?;
    Ok((StatusCode::CREATED, Json(indicator.into())))
}

/// 戦略 scope indicator 作成
#[utoipa::path(
    post,
    path = "/api/strategies/{id}/indicators",
    tag = "custom_indicators",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = CreateCustomIndicatorRequest,
    responses(
        (status = 201, body = CustomIndicatorResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_strategy_indicator(
    State(state): State<AppState>,
    JsonPath(strategy_id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<CreateCustomIndicatorRequest>,
) -> Result<(StatusCode, Json<CustomIndicatorResponse>), AppError> {
    let indicator = state
        .custom_indicator_use_cases
        .create(CreateCustomIndicatorCommand {
            name: payload.name,
            strategy_id: Some(strategy_id),
            code: payload.code,
            input_schema: payload.input_schema,
            output_schema: payload.output_schema,
            description: payload.description,
        })
        .await
        .map_err(map_custom_indicator_error)?;
    Ok((StatusCode::CREATED, Json(indicator.into())))
}

/// indicator 更新
#[utoipa::path(
    put,
    path = "/api/indicators/{indicator_id}",
    tag = "custom_indicators",
    params(("indicator_id" = Uuid, Path, description = "indicator ID")),
    request_body = UpdateCustomIndicatorRequest,
    responses(
        (status = 200, body = CustomIndicatorResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_indicator(
    State(state): State<AppState>,
    JsonPath(indicator_id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateCustomIndicatorRequest>,
) -> Result<Json<CustomIndicatorResponse>, AppError> {
    let indicator = state
        .custom_indicator_use_cases
        .update(
            indicator_id,
            UpdateCustomIndicatorCommand {
                name: payload.name,
                code: payload.code,
                input_schema: payload.input_schema,
                output_schema: payload.output_schema,
                description: payload.description,
            },
        )
        .await
        .map_err(map_custom_indicator_error)?;
    Ok(Json(indicator.into()))
}

/// indicator 削除
#[utoipa::path(
    delete,
    path = "/api/indicators/{indicator_id}",
    tag = "custom_indicators",
    params(("indicator_id" = Uuid, Path, description = "indicator ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_indicator(
    State(state): State<AppState>,
    JsonPath(indicator_id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    state
        .custom_indicator_use_cases
        .delete(indicator_id)
        .await
        .map_err(map_custom_indicator_error)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 戦略 scope の指定 indicator 詳細 (境界外は 404)
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/indicators/{indicator_id}",
    tag = "custom_indicators",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("indicator_id" = Uuid, Path, description = "indicator ID"),
    ),
    responses(
        (status = 200, body = CustomIndicatorResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_strategy_indicator(
    State(state): State<AppState>,
    JsonPath((strategy_id, indicator_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<Json<CustomIndicatorResponse>, AppError> {
    let indicator = state
        .custom_indicator_use_cases
        .get_strategy_indicator(strategy_id, indicator_id)
        .await
        .map_err(map_custom_indicator_error)?;
    Ok(Json(indicator.into()))
}

fn map_custom_indicator_error(error: CustomIndicatorUseCaseError) -> AppError {
    match error {
        CustomIndicatorUseCaseError::Validation(message) => AppError::Validation(message),
        CustomIndicatorUseCaseError::NotFound(indicator_id) => {
            AppError::NotFound(format!("indicator {indicator_id} not found"))
        }
        CustomIndicatorUseCaseError::Repository(CustomIndicatorRepositoryError::Database(
            error,
        ))
        | CustomIndicatorUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
        | CustomIndicatorUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | CustomIndicatorUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error))
        | CustomIndicatorUseCaseError::StrategyExistence(StrategyExistenceError::Database(error)) => {
            error.into()
        }
        other => AppError::Internal(other.to_string()),
    }
}

/// indicator を保存せずに 1 回だけ実行してみる (Monaco エディタのプレビュー用)
#[utoipa::path(
    post,
    path = "/api/indicators/preview",
    tag = "custom_indicators",
    request_body = PreviewIndicatorRequest,
    responses(
        (status = 200, body = PreviewIndicatorResponse),
        (status = 400, description = "code / schema / args が不正", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 503, description = "indicator runtime が未設定または利用不可", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn preview_indicator(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<PreviewIndicatorRequest>,
) -> Result<Json<PreviewIndicatorResponse>, AppError> {
    ensure_json_object("input_schema", &payload.input_schema)?;
    ensure_json_object("output_schema", &payload.output_schema)?;

    let executor = state.kata_executor.as_ref().ok_or_else(|| {
        AppError::ServiceUnavailable("indicator runtime is not configured".into())
    })?;

    let outcome = run_preview(
        executor,
        PreviewInput {
            code: &payload.code,
            input_schema: &payload.input_schema,
            output_schema: &payload.output_schema,
            args: &payload.args,
            timeout_secs: payload.timeout_secs,
            max_output_bytes: payload.max_output_bytes,
        },
    )
    .await?;

    Ok(Json(PreviewIndicatorResponse {
        output: outcome.output,
        stdout: outcome.stdout,
        stderr: outcome.stderr,
        exit_code: outcome.exit_code,
    }))
}
