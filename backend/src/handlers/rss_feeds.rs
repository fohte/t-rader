//! `rss_feed` テーブルの CRUD HTTP handler。
//!
//! バリデーション・DB 操作は `services::rss_feed` に委譲する thin wrapper。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::models::{
    CreateRssFeedRequest, ListRssFeedsQuery, RssFeedResponse, UpdateRssFeedRequest,
};
use crate::services::rss_feed as svc;

/// service レイヤのエラーを AppError にマップする
fn map_err(err: svc::RssFeedError) -> AppError {
    match err {
        svc::RssFeedError::InvalidSource(_)
        | svc::RssFeedError::InvalidUrl(_)
        | svc::RssFeedError::EmptyDisplayName => AppError::Validation(err.to_string()),
        svc::RssFeedError::DuplicateSource(_) => AppError::Conflict(err.to_string()),
        svc::RssFeedError::NotFound(_) => AppError::NotFound(err.to_string()),
        svc::RssFeedError::Database(e) => AppError::Database(e),
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
    State(state): State<AppState>,
    JsonQuery(query): JsonQuery<ListRssFeedsQuery>,
) -> Result<Json<Vec<RssFeedResponse>>, AppError> {
    let rows = svc::list(&state.db, query.enabled_only.unwrap_or(false))
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
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<RssFeedResponse>, AppError> {
    let feed = svc::get(&state.db, id).await.map_err(map_err)?;
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
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateRssFeedRequest>,
) -> Result<(StatusCode, Json<RssFeedResponse>), AppError> {
    let created = svc::create(
        &state.db,
        svc::CreateInput {
            source: payload.source,
            display_name: payload.display_name,
            url: payload.url,
            enabled: payload.enabled,
        },
    )
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
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateRssFeedRequest>,
) -> Result<Json<RssFeedResponse>, AppError> {
    let updated = svc::update(
        &state.db,
        id,
        svc::UpdatePatch {
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
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    svc::delete(&state.db, id).await.map_err(map_err)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use crate::testing::create_test_server;
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    fn normalize(mut value: Value) -> Value {
        for key in ["id", "created_at", "updated_at"] {
            if let Some(v) = value.get_mut(key) {
                *v = Value::String(format!("<{key}>"));
            }
        }
        value
    }

    #[backend_test_macros::database_test]
    async fn create_returns_201_with_full_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "bloomberg-jp",
                "display_name": "Bloomberg JP",
                "url": "https://feeds.bloomberg.co.jp/markets.xml",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "source": "bloomberg-jp",
                "display_name": "Bloomberg JP",
                "url": "https://feeds.bloomberg.co.jp/markets.xml",
                "enabled": true,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_returns_full_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let created: Value = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "example-feed",
                "display_name": "Example Feed",
                "url": "https://example.com/feed.xml",
            }))
            .await
            .json();
        let id = created["id"].as_str().unwrap();

        let res = server.get(&format!("/api/rss-feeds/{id}")).await;
        res.assert_status_ok();
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "source": "example-feed",
                "display_name": "Example Feed",
                "url": "https://example.com/feed.xml",
                "enabled": true,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_nonexistent_feed_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .get("/api/rss-feeds/00000000-0000-4000-8000-000000000001")
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn create_rejects_unknown_field(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "example-feed",
                "display_name": "Example Feed",
                "url": "https://example.com/feed.xml",
                "unexpected": true,
            }))
            .await;
        res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[backend_test_macros::database_test]
    async fn duplicate_source_is_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let body = json!({
            "source": "dup",
            "display_name": "Dup",
            "url": "https://example.com/a",
        });
        server
            .post("/api/rss-feeds")
            .json(&body)
            .await
            .assert_status(StatusCode::CREATED);
        let res = server.post("/api/rss-feeds").json(&body).await;
        res.assert_status(StatusCode::CONFLICT);
    }

    #[backend_test_macros::database_test]
    async fn invalid_source_slug_is_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "Has Space",
                "display_name": "x",
                "url": "https://example.com/a",
            }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn invalid_url_is_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "ok",
                "display_name": "x",
                "url": "not-a-url",
            }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn list_enabled_only_filters(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let a: Value = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "a", "display_name": "A", "url": "https://example.com/a",
            }))
            .await
            .json();
        server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "b", "display_name": "B", "url": "https://example.com/b",
            }))
            .await
            .assert_status(StatusCode::CREATED);
        let a_id = a["id"].as_str().unwrap();
        server
            .patch(&format!("/api/rss-feeds/{a_id}"))
            .json(&json!({ "enabled": false }))
            .await
            .assert_status_ok();
        let res = server.get("/api/rss-feeds?enabled_only=true").await;
        res.assert_status_ok();
        let body: Vec<Value> = res.json();
        let normalized: Vec<Value> = body.into_iter().map(normalize).collect();
        assert_eq!(
            normalized,
            vec![json!({
                "id": "<id>",
                "source": "b",
                "display_name": "B",
                "url": "https://example.com/b",
                "enabled": true,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            })],
        );
    }

    #[backend_test_macros::database_test]
    async fn patch_updates_fields(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let created: Value = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "x", "display_name": "Old", "url": "https://example.com/a",
            }))
            .await
            .json();
        let id = created["id"].as_str().unwrap();
        let res = server
            .patch(&format!("/api/rss-feeds/{id}"))
            .json(&json!({ "display_name": "New", "enabled": false }))
            .await;
        res.assert_status_ok();
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "source": "x",
                "display_name": "New",
                "url": "https://example.com/a",
                "enabled": false,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn patch_rejects_unknown_field(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let created: Value = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "example-feed",
                "display_name": "Example Feed",
                "url": "https://example.com/feed.xml",
            }))
            .await
            .json();
        let id = created["id"].as_str().unwrap();

        let res = server
            .patch(&format!("/api/rss-feeds/{id}"))
            .json(&json!({
                "display_name": "Renamed Feed",
                "unexpected": true,
            }))
            .await;
        res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[backend_test_macros::database_test]
    async fn delete_returns_204_then_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let created: Value = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "x", "display_name": "x", "url": "https://example.com/a",
            }))
            .await
            .json();
        let id = created["id"].as_str().unwrap();
        server
            .delete(&format!("/api/rss-feeds/{id}"))
            .await
            .assert_status(StatusCode::NO_CONTENT);
        server
            .delete(&format!("/api/rss-feeds/{id}"))
            .await
            .assert_status(StatusCode::NOT_FOUND);
    }
}
