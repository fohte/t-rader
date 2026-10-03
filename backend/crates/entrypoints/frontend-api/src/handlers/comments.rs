use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::change_history::Actor;
use core_application::change_history::ChangeHistoryError;
use core_application::comment::{
    CommentListQuery, CommentReadQueryError, CommentReadUseCaseError, CommentRepositoryError,
    CommentTargetKind, CommentUseCaseError, CreateCommentCommand, DeleteCommentCommand,
    ResolveCommentCommand,
};
use core_application::unit_of_work::UnitOfWorkError;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::models::{CommentResponse, CreateCommentRequest, UpdateCommentRequest};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListCommentsQuery {
    pub target_kind: String,
    pub target_id: Uuid,
}

/// コメント一覧。`target_kind` + `target_id` でフィルタ。スレッドは parent_id で表現する。
#[utoipa::path(
    get,
    path = "/api/comments",
    tag = "comments",
    params(ListCommentsQuery),
    responses(
        (status = 200, body = Vec<CommentResponse>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_comments(
    State(state): State<FrontendApiState>,
    JsonQuery(p): JsonQuery<ListCommentsQuery>,
) -> Result<Json<Vec<CommentResponse>>, AppError> {
    let target_kind = CommentTargetKind::parse(&p.target_kind)
        .ok_or_else(|| AppError::Validation(format!("invalid target_kind: {}", p.target_kind)))?;
    let comments = state
        .comment_read_use_cases
        .list_comments(
            CommentListQuery {
                target_kind,
                target_id: p.target_id,
                resolved: None,
            },
            None,
        )
        .await
        .map_err(map_comment_read_error)?;
    Ok(Json(
        comments.into_iter().map(CommentResponse::from).collect(),
    ))
}

/// コメント投稿
#[utoipa::path(
    post,
    path = "/api/comments",
    tag = "comments",
    request_body = CreateCommentRequest,
    responses(
        (status = 201, body = CommentResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_comment(
    State(state): State<FrontendApiState>,
    JsonBody(p): JsonBody<CreateCommentRequest>,
) -> Result<(StatusCode, Json<CommentResponse>), AppError> {
    let author_kind = p.author_kind.as_deref().unwrap_or("human").to_string();
    let author_label = p.author_label.as_deref().unwrap_or("user").to_string();
    let created = state
        .comment_use_cases
        .create(CreateCommentCommand {
            scope: None,
            actor: Actor::Human,
            target_kind: p.target_kind,
            target_id: p.target_id,
            parent_id: p.parent_id,
            body: p.body,
            author_kind,
            author_label,
            anchor_text: p.anchor_text,
            anchor_side: p.anchor_side.map(|side| side.as_str().to_string()),
            start_line: p.start_line,
            end_line: p.end_line,
        })
        .await
        .map_err(map_comment_error)?;

    Ok((StatusCode::CREATED, Json(created.into())))
}

/// コメントの resolved を更新する
#[utoipa::path(
    patch,
    path = "/api/comments/{id}",
    tag = "comments",
    params(("id" = Uuid, Path, description = "コメント ID")),
    request_body = UpdateCommentRequest,
    responses(
        (status = 200, body = CommentResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_comment(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateCommentRequest>,
) -> Result<Json<CommentResponse>, AppError> {
    let updated = state
        .comment_use_cases
        .resolve(ResolveCommentCommand {
            scope: None,
            actor: Actor::Human,
            id,
            resolved: payload.resolved,
        })
        .await
        .map_err(map_comment_error)?;

    Ok(Json(updated.into()))
}

/// コメント削除
#[utoipa::path(
    delete,
    path = "/api/comments/{id}",
    tag = "comments",
    params(("id" = Uuid, Path, description = "コメント ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_comment(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    state
        .comment_use_cases
        .delete(DeleteCommentCommand {
            scope: None,
            actor: Actor::Human,
            id,
        })
        .await
        .map_err(map_comment_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn map_comment_error(error: CommentUseCaseError) -> AppError {
    match error {
        CommentUseCaseError::Validation(message) => AppError::Validation(message),
        CommentUseCaseError::NotFound(message) => AppError::NotFound(message),
        CommentUseCaseError::Forbidden(message) => AppError::Validation(message),
        CommentUseCaseError::Repository(CommentRepositoryError::Database(error))
        | CommentUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
        | CommentUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | CommentUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
        other => AppError::Internal(other.to_string()),
    }
}

pub(super) fn map_comment_read_error(error: CommentReadUseCaseError) -> AppError {
    match error {
        CommentReadUseCaseError::Query(CommentReadQueryError::Database(error)) => error.into(),
        CommentReadUseCaseError::AnnotationRead(error) => {
            crate::handlers::annotations::map_annotation_read_error(error)
        }
        CommentReadUseCaseError::NoteRead(error) => {
            crate::handlers::notes::map_note_read_error(error)
        }
    }
}
