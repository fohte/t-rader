pub mod agent_client;
pub mod cli;
pub mod data_provider;
pub mod entities;
pub mod error;
pub mod extractors;
pub mod handlers;
#[cfg(test)]
mod integration_tests;
pub mod kata_exec;
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

use crate::agent_client::{AgentTaskClient, DisabledAgentTaskClient, SharedAgentTaskClient};
use crate::data_provider::DataProviderKind;
use crate::data_provider::macro_data::MacroCache;
use crate::error::{AppError, ErrorResponse};
use crate::handlers::{
    agent_tasks, annotations, bars, comments, custom_indicators, history, hooks, hypotheses,
    imports, interests, macro_data, news, notes, refs, rss_feeds, strategies, trades, triggers,
    watchlists,
};
use crate::kata_exec::SharedKataExecutor;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    /// 株価データプロバイダー (J-Quants API 等)
    ///
    /// `JQUANTS_API_KEY` 未設定時は None で起動する。
    /// データ取得系のエンドポイントは利用時にエラーを返す。
    pub data_provider: Option<Arc<DataProviderKind>>,
    /// t-rader-agent 内部 API クライアント。戦略タスクの投入 / 状態照会に使う。
    /// `TRADER_AGENT_API_URL=disabled` (dev opt-out) の場合は `DisabledAgentTaskClient` が入る。
    pub agent_task_client: SharedAgentTaskClient,
    /// watcher (`mcp::watcher`) の polling を即時発火させるための通知。
    /// t-rader-agent からの webhook 受信時に `notify_one()` される。
    pub agent_task_notify: Arc<tokio::sync::Notify>,
    /// t-rader-agent からの webhook (`POST /api/agent-tasks/notifications`) を認証する
    /// bearer トークン。
    pub agent_webhook_token: Arc<str>,
    /// Kata Containers exec Pod executor。`KATA_EXEC_API_URL` 未設定時は `None` で
    /// 起動し、`eval_python` tool は MCP エラーを返す。
    pub kata_executor: Option<SharedKataExecutor>,
    /// マクロ指標の最新値 cache (Stooq 等の poll task が書き込み、handler が読む)
    pub macro_cache: Option<Arc<MacroCache>>,
}

impl AppState {
    /// テスト・初期化以外で t-rader-agent 内部 API クライアントが未設定のケース向けのデフォルト
    pub fn disabled_agent_task_client() -> SharedAgentTaskClient {
        let client: Arc<dyn AgentTaskClient + Send + Sync> = Arc::new(DisabledAgentTaskClient);
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
        (name = "triggers", description = "戦略 trigger (cron / hook)"),
        (name = "imports", description = "外部ソースからの取込 (SBI CSV 等)"),
        (name = "custom_indicators", description = "カスタムインジケーター (Python 定義)"),
        (name = "macro", description = "マクロ指標 (日経225 / TOPIX / USD/JPY 等の現在値)"),
        (name = "news", description = "ニュース (公開 RSS の集約結果と戦略への紐付け)"),
        (name = "rss_feeds", description = "ニュース集約対象の RSS フィード定義"),
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
            agent_task_client: AppState::disabled_agent_task_client(),
            agent_task_notify: Arc::new(tokio::sync::Notify::new()),
            agent_webhook_token: Arc::from("test-token"),
            kata_executor: None,
            macro_cache: None,
        };
        assert!(state.data_provider().is_ok());
    }

    #[rstest]
    fn test_data_provider_returns_error_when_none() {
        let state = AppState {
            db: mock_db(),
            data_provider: None,
            agent_task_client: AppState::disabled_agent_task_client(),
            agent_task_notify: Arc::new(tokio::sync::Notify::new()),
            agent_webhook_token: Arc::from("test-token"),
            kata_executor: None,
            macro_cache: None,
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
        .routes(routes!(
            strategies::list_strategy_interests,
            interests::create_strategy_interest
        ))
        .routes(routes!(
            interests::update_strategy_interest,
            interests::delete_strategy_interest
        ))
        // hypotheses
        .routes(routes!(
            hypotheses::list_strategy_hypotheses,
            hypotheses::create_strategy_hypothesis
        ))
        .routes(routes!(
            hypotheses::get_strategy_hypothesis,
            hypotheses::update_strategy_hypothesis,
            hypotheses::delete_strategy_hypothesis
        ))
        .routes(routes!(strategies::submit_strategy_chat))
        .routes(routes!(strategies::get_strategy_task))
        .routes(routes!(
            strategies::get_agents_md,
            strategies::put_agents_md
        ))
        .routes(routes!(strategies::get_skills, strategies::put_skills))
        .routes(routes!(strategies::put_skill, strategies::delete_skill))
        .routes(routes!(strategies::get_agent_config))
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
        // triggers
        .routes(routes!(
            triggers::list_strategy_triggers,
            triggers::create_strategy_trigger
        ))
        .routes(routes!(
            triggers::get_trigger,
            triggers::update_trigger,
            triggers::delete_trigger
        ))
        // hooks (外部 webhook 受信)
        .routes(routes!(hooks::receive_hook))
        // t-rader-agent からのタスク決着通知
        .routes(routes!(agent_tasks::receive_agent_task_notification))
        // imports
        .routes(routes!(imports::sbi_preview))
        .routes(routes!(imports::sbi_commit))
        // custom indicators
        .routes(routes!(
            custom_indicators::list_global_indicators,
            custom_indicators::create_global_indicator
        ))
        .routes(routes!(
            custom_indicators::get_indicator,
            custom_indicators::update_indicator,
            custom_indicators::delete_indicator
        ))
        .routes(routes!(
            custom_indicators::list_strategy_indicators,
            custom_indicators::create_strategy_indicator
        ))
        .routes(routes!(custom_indicators::get_strategy_indicator))
        .routes(routes!(custom_indicators::preview_indicator))
        // macro
        .routes(routes!(macro_data::get_macro_ticks))
        // news
        .routes(routes!(news::list_strategy_news))
        // rss feeds
        .routes(routes!(
            rss_feeds::list_rss_feeds,
            rss_feeds::create_rss_feed
        ))
        .routes(routes!(
            rss_feeds::update_rss_feed,
            rss_feeds::delete_rss_feed
        ))
}

/// OpenAPI スペックを生成する (DB 接続不要)
pub fn create_openapi_spec() -> utoipa::openapi::OpenApi {
    let mut router = build_openapi_router();
    router.to_openapi()
}

pub fn create_router(state: AppState) -> Router {
    let db = state.db.clone();
    let agent_task_client = state.agent_task_client.clone();
    let data_provider = state.data_provider.clone();
    let kata_executor = state.kata_executor.clone();
    let (router, api) = build_openapi_router().with_state(state).split_for_parts();

    router
        .layer(axum::middleware::from_fn(middleware::reject_null_bytes))
        .merge(SwaggerUi::new("/api-docs").url("/api-docs/openapi.json", api))
        .merge(mcp::router(
            db,
            agent_task_client,
            data_provider,
            kata_executor,
            mcp::allowed_hosts_from_env(),
        ))
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
