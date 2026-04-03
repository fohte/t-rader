pub mod cli;
pub mod data_provider;
pub mod entities;
pub mod error;
pub mod extractors;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod repositories;
pub mod schemas;
pub mod services;
#[cfg(test)]
pub mod testing;

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode};
use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::Serialize;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use utoipa::OpenApi;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use utoipa_swagger_ui::SwaggerUi;

use crate::data_provider::DataProviderKind;
use crate::error::{AppError, ErrorResponse};
use crate::handlers::{bars, watchlists};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    /// 株価データプロバイダー (J-Quants API 等)
    ///
    /// `JQUANTS_API_KEY` 未設定時は None で起動する。
    /// データ取得系のエンドポイントは利用時にエラーを返す。
    pub data_provider: Option<Arc<DataProviderKind>>,
}

impl AppState {
    /// DataProvider を取得する
    ///
    /// `JQUANTS_API_KEY` 未設定で起動した場合は 503 エラーを返す。
    pub fn data_provider(&self) -> Result<&DataProviderKind, AppError> {
        self.data_provider
            .as_deref()
            .ok_or_else(|| AppError::ServiceUnavailable("data provider is not configured".into()))
    }
}

#[derive(OpenApi)]
#[openapi(
    tags(
        (name = "health", description = "ヘルスチェック"),
        (name = "bars", description = "バーデータ (OHLCV)"),
        (name = "watchlists", description = "ウォッチリスト管理"),
        (name = "watchlist_items", description = "ウォッチリスト内の銘柄管理"),
    ),
    info(
        title = "T-Rader API",
        version = "0.1.0",
        description = "日本株投資プラットフォーム T-Rader の API",
    ),
)]
struct ApiDoc;

#[cfg(test)]
mod app_state_tests {
    use rstest::rstest;
    use sea_orm::{DatabaseBackend, MockDatabase};

    use super::*;

    fn mock_db() -> sea_orm::DatabaseConnection {
        MockDatabase::new(DatabaseBackend::Postgres).into_connection()
    }

    #[rstest]
    fn test_data_provider_returns_provider_when_set() {
        let client = crate::data_provider::jquants::JQuantsClient::new("test-key".into()).unwrap();
        let state = AppState {
            db: mock_db(),
            data_provider: Some(Arc::new(DataProviderKind::JQuants(client))),
        };
        assert!(state.data_provider().is_ok());
    }

    #[rstest]
    fn test_data_provider_returns_error_when_none() {
        let state = AppState {
            db: mock_db(),
            data_provider: None,
        };
        let result = state.data_provider();
        assert!(result.is_err());
    }
}

/// ヘルスチェックレスポンス
#[derive(Serialize, ToSchema)]
struct HealthResponse {
    /// サービスの状態
    status: String,
}

/// OpenAPI ルート定義を構築する
fn build_openapi_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health_check))
        .routes(routes!(watchlists::create_watchlist))
        .routes(routes!(watchlists::list_watchlists))
        .routes(routes!(watchlists::delete_watchlist))
        .routes(routes!(watchlists::add_watchlist_item))
        .routes(routes!(watchlists::list_watchlist_items))
        .routes(routes!(watchlists::delete_watchlist_item))
        .routes(routes!(bars::list_bars))
}

/// OpenAPI スペックを生成する (DB 接続不要)
pub fn create_openapi_spec() -> utoipa::openapi::OpenApi {
    let mut router = build_openapi_router();
    router.to_openapi()
}

/// CORS_ORIGIN 環境変数から CorsLayer を構築する。
/// `*` で全オリジン許可、カンマ区切りで複数オリジンを指定可能。
fn build_cors_layer() -> Result<CorsLayer, AppError> {
    let origin_val = std::env::var("CORS_ORIGIN")
        .map_err(|_| AppError::Config("CORS_ORIGIN environment variable is not set".to_string()))?;

    let allow_origin = if origin_val == "*" {
        AllowOrigin::any()
    } else {
        let origins = origin_val
            .split(',')
            .map(|s| s.trim().parse::<HeaderValue>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::Config(format!("CORS_ORIGIN の値が不正です: {origin_val}")))?;
        AllowOrigin::list(origins)
    };

    Ok(CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods(Any)
        .allow_headers(Any))
}

pub fn create_router(state: AppState) -> Result<Router, AppError> {
    let (router, api) = build_openapi_router().with_state(state).split_for_parts();

    Ok(router
        .merge(SwaggerUi::new("/api-docs").url("/api-docs/openapi.json", api))
        .layer(axum::middleware::from_fn(middleware::reject_null_bytes))
        .layer(build_cors_layer()?))
}

/// ヘルスチェック
#[utoipa::path(
    get,
    path = "/api/health",
    tag = "health",
    responses(
        (status = 200, description = "サービス正常", body = HealthResponse),
        (status = 500, description = "内部サーバーエラー", body = ErrorResponse),
    )
)]
async fn health_check(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<HealthResponse>), AppError> {
    // DB 接続の正常性を確認
    state.db.execute_unprepared("SELECT 1").await?;

    Ok((
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok".to_string(),
        }),
    ))
}
