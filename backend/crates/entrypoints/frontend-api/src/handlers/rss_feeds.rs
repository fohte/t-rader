//! `rss_feed` テーブルの CRUD HTTP handler。
//!
//! RSS feed のユースケース結果を HTTP 応答に変換する。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use core_application::rss_feed::{
    CreateRssFeedCommand, RssFeedRepositoryError, RssFeedUseCaseError, UpdateRssFeedPatch,
};

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::models::{
    CreateRssFeedRequest, ListRssFeedsQuery, RssFeedResponse, UpdateRssFeedRequest,
};

fn map_err(err: RssFeedUseCaseError) -> AppError {
    match err {
        RssFeedUseCaseError::Validation(message) => AppError::Validation(message),
        error @ RssFeedUseCaseError::Repository(RssFeedRepositoryError::DuplicateSource(_)) => {
            AppError::Conflict(error.to_string())
        }
        error @ RssFeedUseCaseError::NotFound(_) => AppError::NotFound(error.to_string()),
        RssFeedUseCaseError::Repository(RssFeedRepositoryError::Persistence(error)) => error.into(),
        RssFeedUseCaseError::UnitOfWork(
            core_application::unit_of_work::UnitOfWorkError::Begin(error)
            | core_application::unit_of_work::UnitOfWorkError::Commit(error),
        ) => error.into(),
        other => AppError::Internal(other.to_string()),
    }
}

/// RSS フィード一覧
#[utoipa::path(
    get,
    path = "/api/rss-feeds",
    tag = "rss_feeds",
    params(
        ("enabled_only" = Option<bool>, Query, description = "true なら enabled=true のみ返す"),
    ),
    responses(
        (status = 200, body = Vec<RssFeedResponse>),
        (status = 400, description = "クエリパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_rss_feeds(
    State(state): State<FrontendApiState>,
    JsonQuery(query): JsonQuery<ListRssFeedsQuery>,
) -> Result<Json<Vec<RssFeedResponse>>, AppError> {
    let rows = state
        .rss_feed_use_cases
        .list(query.enabled_only.unwrap_or(false))
        .await
        .map_err(map_err)?;
    Ok(Json(rows.into_iter().map(RssFeedResponse::from).collect()))
}

/// RSS フィードを取得
#[utoipa::path(
    get,
    path = "/api/rss-feeds/{id}",
    tag = "rss_feeds",
    params(("id" = Uuid, Path, description = "rss_feed ID")),
    responses(
        (status = 200, body = RssFeedResponse),
        (status = 400, description = "パスパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_rss_feed(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<RssFeedResponse>, AppError> {
    let feed = state.rss_feed_use_cases.get(id).await.map_err(map_err)?;
    Ok(Json(feed.into()))
}

/// RSS フィードを作成
#[utoipa::path(
    post,
    path = "/api/rss-feeds",
    tag = "rss_feeds",
    request_body = CreateRssFeedRequest,
    responses(
        (status = 201, body = RssFeedResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 409, description = "source が既存と衝突", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_rss_feed(
    State(state): State<FrontendApiState>,
    JsonBody(payload): JsonBody<CreateRssFeedRequest>,
) -> Result<(StatusCode, Json<RssFeedResponse>), AppError> {
    let created = state
        .rss_feed_use_cases
        .create(CreateRssFeedCommand {
            source: payload.source,
            display_name: payload.display_name,
            url: payload.url,
            enabled: payload.enabled,
        })
        .await
        .map_err(map_err)?;
    Ok((StatusCode::CREATED, Json(created.into())))
}

/// RSS フィードを部分更新する (`source` は変更不可)
#[utoipa::path(
    patch,
    path = "/api/rss-feeds/{id}",
    tag = "rss_feeds",
    params(("id" = Uuid, Path, description = "rss_feed ID")),
    request_body = UpdateRssFeedRequest,
    responses(
        (status = 200, body = RssFeedResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_rss_feed(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateRssFeedRequest>,
) -> Result<Json<RssFeedResponse>, AppError> {
    let updated = state
        .rss_feed_use_cases
        .update(
            id,
            UpdateRssFeedPatch {
                display_name: payload.display_name,
                url: payload.url,
                enabled: payload.enabled,
            },
        )
        .await
        .map_err(map_err)?;
    Ok(Json(updated.into()))
}

/// RSS フィードを削除する。news_item 行は残す (履歴互換性)。
#[utoipa::path(
    delete,
    path = "/api/rss-feeds/{id}",
    tag = "rss_feeds",
    params(("id" = Uuid, Path, description = "rss_feed ID")),
    responses(
        (status = 204),
        (status = 400, description = "パスパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_rss_feed(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    state.rss_feed_use_cases.delete(id).await.map_err(map_err)?;
    Ok(StatusCode::NO_CONTENT)
}
