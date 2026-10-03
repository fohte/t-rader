use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::annotation::{
    AnnotationListQuery, AnnotationReadQueryError, AnnotationReadUseCaseError,
    AnnotationRepositoryError, AnnotationUseCaseError, ChangeAnnotationStatusCommand,
    CreateAnnotationCommand, DeleteAnnotationCommand, UpdateAnnotationCommand,
};
use core_application::change_history::{Actor, ChangeHistoryError};
use core_application::strategy_existence::StrategyExistenceError;
use core_application::strategy_task::TaskSource;
use core_application::unit_of_work::UnitOfWorkError;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::handlers::strategies::map_submit_error;
use crate::models::{
    AnnotationResponse, ChangeStatusRequest, CreateAnnotationRequest, UpdateAnnotationRequest,
};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListAnnotationsQuery {
    pub strategy_id: Option<Uuid>,
    pub target_symbol: Option<String>,
}

/// アノテーション一覧
#[utoipa::path(
    get,
    path = "/api/annotations",
    tag = "annotations",
    params(ListAnnotationsQuery),
    responses(
        (status = 200, body = Vec<AnnotationResponse>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_annotations(
    State(state): State<FrontendApiState>,
    JsonQuery(params): JsonQuery<ListAnnotationsQuery>,
) -> Result<Json<Vec<AnnotationResponse>>, AppError> {
    let annotations = state
        .annotation_read_use_cases
        .list_annotations(AnnotationListQuery {
            strategy_id: params.strategy_id,
            target_symbol: params.target_symbol.filter(|symbol| !symbol.is_empty()),
            limit: None,
        })
        .await
        .map_err(map_annotation_read_error)?;
    Ok(Json(
        annotations
            .into_iter()
            .map(AnnotationResponse::from)
            .collect(),
    ))
}

/// アノテーション取得
#[utoipa::path(
    get,
    path = "/api/annotations/{id}",
    tag = "annotations",
    params(("id" = Uuid, Path, description = "アノテーション ID")),
    responses(
        (status = 200, body = AnnotationResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_annotation(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<AnnotationResponse>, AppError> {
    let annotation = state
        .annotation_read_use_cases
        .get_annotation(id)
        .await
        .map_err(map_annotation_read_error)?;
    Ok(Json(annotation.into()))
}

/// アノテーション作成
#[utoipa::path(
    post,
    path = "/api/annotations",
    tag = "annotations",
    request_body = CreateAnnotationRequest,
    responses(
        (status = 201, body = AnnotationResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_annotation(
    State(state): State<FrontendApiState>,
    JsonBody(p): JsonBody<CreateAnnotationRequest>,
) -> Result<(StatusCode, Json<AnnotationResponse>), AppError> {
    let status = p.status.as_deref().unwrap_or("unread").to_string();
    let created_by = p.created_by_kind.as_deref().unwrap_or("human").to_string();
    let created = state
        .annotation_use_cases
        .create(CreateAnnotationCommand {
            scope: None,
            actor: Actor::Human,
            strategy_id: p.strategy_id,
            target_symbol: p.target_symbol,
            target_kind: p.target_kind,
            timestamp: p.timestamp,
            price: p.price,
            text: p.text,
            status,
            linked_note_id: p.linked_note_id,
            created_by_kind: created_by,
            execution_step_id: None,
            execution_task_id: None,
        })
        .await
        .map_err(map_annotation_error)?;

    Ok((StatusCode::CREATED, Json(created.into())))
}

/// アノテーション更新
#[utoipa::path(
    patch,
    path = "/api/annotations/{id}",
    tag = "annotations",
    params(("id" = Uuid, Path, description = "アノテーション ID")),
    request_body = UpdateAnnotationRequest,
    responses(
        (status = 200, body = AnnotationResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_annotation(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(p): JsonBody<UpdateAnnotationRequest>,
) -> Result<Json<AnnotationResponse>, AppError> {
    let updated = state
        .annotation_use_cases
        .update(UpdateAnnotationCommand {
            actor: Actor::Human,
            id,
            target_symbol: p.target_symbol,
            target_kind: p.target_kind,
            timestamp: p.timestamp,
            price: p.price,
            text: p.text,
            linked_note_id: p.linked_note_id,
        })
        .await
        .map_err(map_annotation_error)?;
    Ok(Json(updated.into()))
}

/// アノテーションを approved に遷移
#[utoipa::path(
    post,
    path = "/api/annotations/{id}/approve",
    tag = "annotations",
    params(("id" = Uuid, Path, description = "アノテーション ID")),
    request_body = ChangeStatusRequest,
    responses(
        (status = 200, body = AnnotationResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn approve_annotation(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<AnnotationResponse>, AppError> {
    let updated = state
        .annotation_use_cases
        .change_status(ChangeAnnotationStatusCommand {
            actor: Actor::Human,
            id,
            status: "approved".into(),
            label: payload.label,
        })
        .await
        .map_err(map_annotation_error)?;
    Ok(Json(updated.into()))
}

/// アノテーションを rejected に遷移
#[utoipa::path(
    post,
    path = "/api/annotations/{id}/reject",
    tag = "annotations",
    params(("id" = Uuid, Path, description = "アノテーション ID")),
    request_body = ChangeStatusRequest,
    responses(
        (status = 200, body = AnnotationResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
        (status = 503, description = "agent task client が未設定、または agent_config が見つからない", body = ErrorResponse),
    )
)]
pub async fn reject_annotation(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<AnnotationResponse>, AppError> {
    let current = state
        .annotation_read_use_cases
        .get_annotation(id)
        .await
        .map_err(map_annotation_read_error)?;
    // 却下確定前の check-then-act。ほぼ同時に reject が 2 回届くと両方通過し得るが、
    // frontend は mutation pending 中ボタンを disable するため実運用では起きない。
    if current.status == "rejected" {
        return Ok(Json(current.into()));
    }

    if let Some(strategy_id) = current.strategy_id {
        let prompt = format!(
            "アノテーション (id: {}, 対象: {}) がレビューで却下されました。付いているコメントを確認し、指摘を反映してください。",
            current.id, current.target_symbol
        );
        state
            .strategy_task_use_cases
            .submit_task(
                state.agent_task_client.as_ref(),
                strategy_id,
                &prompt,
                TaskSource::Review,
                None,
            )
            .await
            .map_err(map_submit_error)?;
    }

    let updated = state
        .annotation_use_cases
        .change_status(ChangeAnnotationStatusCommand {
            actor: Actor::Human,
            id,
            status: "rejected".into(),
            label: payload.label,
        })
        .await
        .map_err(map_annotation_error)?;
    Ok(Json(updated.into()))
}

/// アノテーション削除
#[utoipa::path(
    delete,
    path = "/api/annotations/{id}",
    tag = "annotations",
    params(("id" = Uuid, Path, description = "アノテーション ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_annotation(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    state
        .annotation_use_cases
        .delete(DeleteAnnotationCommand {
            actor: Actor::Human,
            id,
        })
        .await
        .map_err(map_annotation_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn map_annotation_error(error: AnnotationUseCaseError) -> AppError {
    match error {
        AnnotationUseCaseError::Validation(message) => AppError::Validation(message),
        AnnotationUseCaseError::NotFound(id) => {
            AppError::NotFound(format!("annotation {id} not found"))
        }
        AnnotationUseCaseError::LinkedNoteNotFound(_) => {
            AppError::Validation("referenced resource does not exist".into())
        }
        AnnotationUseCaseError::Repository(AnnotationRepositoryError::Database(error))
        | AnnotationUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
        | AnnotationUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | AnnotationUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error))
        | AnnotationUseCaseError::StrategyExistence(StrategyExistenceError::Database(error)) => {
            error.into()
        }
        other => AppError::Internal(other.to_string()),
    }
}

pub(super) fn map_annotation_read_error(error: AnnotationReadUseCaseError) -> AppError {
    match error {
        AnnotationReadUseCaseError::NotFound(id) => {
            AppError::NotFound(format!("annotation {id} not found"))
        }
        AnnotationReadUseCaseError::Query(AnnotationReadQueryError::Database(error)) => {
            error.into()
        }
    }
}
