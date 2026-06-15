pub mod cli;
pub mod data_provider;
pub mod entities;
pub mod error;
pub mod extractors;
pub mod handlers;
pub mod kubeopencode;
pub mod mcp;
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
use axum::http::StatusCode;
use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::Serialize;
use utoipa::OpenApi;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use utoipa_swagger_ui::SwaggerUi;

use crate::data_provider::DataProviderKind;
use crate::error::{AppError, ErrorResponse};
use crate::handlers::{
    annotations, bars, comments, history, imports, notes, refs, strategies, trades, watchlists,
};
use crate::kubeopencode::{
    DisabledKubeopencodeClient, KubeopencodeClient, SharedKubeopencodeClient,
};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    /// 株価データプロバイダー (J-Quants API 等)
    ///
    /// `JQUANTS_API_KEY` 未設定時は None で起動する。
    /// データ取得系のエンドポイントは利用時にエラーを返す。
    pub data_provider: Option<Arc<DataProviderKind>>,
    /// kubeopencode (Task CR) クライアント。`KUBEOPENCODE_API_URL` 未設定時は
    /// `DisabledKubeopencodeClient` が入り、submit_strategy_task は MCP エラーを返す。
    pub kubeopencode: SharedKubeopencodeClient,
}

impl AppState {
    /// テスト・初期化以外で kubeopencode が未設定のケース向けのデフォルト
    pub fn disabled_kubeopencode() -> SharedKubeopencodeClient {
        let client: Arc<dyn KubeopencodeClient + Send + Sync> =
            Arc::new(DisabledKubeopencodeClient);
        client
    }
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
        (name = "strategies", description = "戦略 (ワークスペース)"),
        (name = "refs", description = "一級参照型 (stock / indicator / sector / theme)"),
        (name = "notes", description = "ノート"),
        (name = "annotations", description = "アノテーション"),
        (name = "comments", description = "コメントスレッド"),
        (name = "history", description = "変更履歴"),
        (name = "trades", description = "取引履歴と損益サマリ"),
        (name = "imports", description = "外部ソースからの取込 (SBI CSV 等)"),
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
            kubeopencode: AppState::disabled_kubeopencode(),
        };
        assert!(state.data_provider().is_ok());
    }

    #[rstest]
    fn test_data_provider_returns_error_when_none() {
        let state = AppState {
            db: mock_db(),
            data_provider: None,
            kubeopencode: AppState::disabled_kubeopencode(),
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
        // strategies
        .routes(routes!(
            strategies::list_strategies,
            strategies::create_strategy
        ))
        .routes(routes!(
            strategies::get_strategy,
            strategies::update_strategy,
            strategies::delete_strategy
        ))
        .routes(routes!(strategies::list_strategy_interests))
        // refs
        .routes(routes!(refs::list_stocks))
        .routes(routes!(refs::get_stock))
        .routes(routes!(refs::list_indicators))
        .routes(routes!(refs::get_indicator))
        .routes(routes!(refs::list_sectors))
        .routes(routes!(refs::get_sector))
        .routes(routes!(refs::list_themes))
        .routes(routes!(refs::get_theme))
        .routes(routes!(refs::resolve_refs))
        // notes
        .routes(routes!(notes::list_notes, notes::create_note))
        .routes(routes!(
            notes::get_note,
            notes::update_note,
            notes::delete_note
        ))
        .routes(routes!(notes::approve_note))
        .routes(routes!(notes::reject_note))
        // annotations
        .routes(routes!(
            annotations::list_annotations,
            annotations::create_annotation
        ))
        .routes(routes!(
            annotations::get_annotation,
            annotations::update_annotation,
            annotations::delete_annotation
        ))
        .routes(routes!(annotations::approve_annotation))
        .routes(routes!(annotations::reject_annotation))
        // comments
        .routes(routes!(comments::list_comments, comments::create_comment))
        .routes(routes!(comments::delete_comment))
        // history
        .routes(routes!(history::list_history))
        .routes(routes!(history::get_history))
        // trades — summary before {id} to avoid path conflict
        .routes(routes!(trades::trades_summary))
        .routes(routes!(trades::list_trades, trades::create_trade))
        .routes(routes!(
            trades::get_trade,
            trades::update_trade,
            trades::delete_trade
        ))
        // imports
        .routes(routes!(imports::sbi_preview))
        .routes(routes!(imports::sbi_commit))
}

/// OpenAPI スペックを生成する (DB 接続不要)
pub fn create_openapi_spec() -> utoipa::openapi::OpenApi {
    let mut router = build_openapi_router();
    router.to_openapi()
}

pub fn create_router(state: AppState) -> Router {
    let db = state.db.clone();
    let kube = state.kubeopencode.clone();
    let (router, api) = build_openapi_router().with_state(state).split_for_parts();

    router
        .layer(axum::middleware::from_fn(middleware::reject_null_bytes))
        .merge(SwaggerUi::new("/api-docs").url("/api-docs/openapi.json", api))
        .merge(mcp::router(db, kube))
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
