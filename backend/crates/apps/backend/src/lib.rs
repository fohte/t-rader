pub mod cli;
#[cfg(test)]
mod integration_tests;
pub mod mcp;
pub mod middleware;
pub mod migrations;
pub mod services;
#[cfg(test)]
pub mod testing;

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use core_application::agent_task_client::SharedAgentTaskClient;
use core_application::daily_bar_source::SharedDailyBarSource;
use core_application::kata_exec::SharedKataExecutor;
use core_application::llm_client::SharedLlmClient;
use entrypoint_frontend_api::{AppError, ErrorResponse, FrontendApiState};
use gateway_postgres::DatabaseHandle;
use sea_orm::ConnectionTrait;
use serde::Serialize;
use utoipa::OpenApi;
use utoipa::ToSchema;
use utoipa_swagger_ui::SwaggerUi;
/// composition root の UseCases から frontend-api の依存 state を組み立てる。
pub fn build_http_state(
    use_cases: &crate::services::use_cases::UseCases,
    agent_task_client: SharedAgentTaskClient,
    kata_executor: Option<SharedKataExecutor>,
    llm_gateway_client: Option<SharedLlmClient>,
) -> FrontendApiState {
    let agent_tool_summaries = mcp::StrategyServer::list_tool_summaries();
    let trigger_use_cases = use_cases.triggers();
    FrontendApiState {
        account_risk_policy_use_cases: use_cases.account_risk_policies(),
        agent_config_use_cases: use_cases.agent_configs(),
        annotation_read_use_cases: use_cases.annotation_reads(),
        annotation_use_cases: use_cases.annotations(),
        bars_use_cases: use_cases.bars(),
        calendar_event_read_use_cases: use_cases.calendar_event_reads(),
        change_history_use_cases: use_cases.change_history_reads(),
        comment_read_use_cases: use_cases.comment_reads(),
        comment_use_cases: use_cases.comments(),
        custom_indicator_use_cases: use_cases.custom_indicators(),
        group_axis_use_cases: use_cases.group_axes(),
        ingest_status_use_case: Arc::new(use_cases.ingest_status()),
        note_kind_use_cases: use_cases.note_kinds(),
        note_read_use_cases: use_cases.note_reads(),
        note_use_cases: use_cases.notes(),
        prediction_use_cases: use_cases.predictions(),
        ref_use_cases: use_cases.refs(),
        rss_feed_use_cases: use_cases.rss_feeds(),
        strategy_scope_use_cases: Arc::new(use_cases.strategy_scope()),
        strategy_task_use_cases: use_cases.strategy_tasks(),
        strategy_earnings_target_use_cases: use_cases.strategy_earnings_targets(),
        strategy_use_cases: use_cases.strategies(),
        trigger_use_cases,
        trade_note_use_cases: use_cases.trade_notes(),
        trade_use_cases: use_cases.trades(),
        agent_task_client,
        kata_executor,
        llm_gateway_client,
        agent_tool_summaries,
    }
}

pub fn build_agent_webhook_state(
    use_cases: &crate::services::use_cases::UseCases,
    webhook_token: impl Into<Arc<str>>,
) -> entrypoint_agent_webhook::AgentWebhookState {
    entrypoint_agent_webhook::AgentWebhookState {
        strategy_task_reconcile_job_use_cases: use_cases.strategy_task_reconcile_job(),
        webhook_token: webhook_token.into(),
    }
}

pub fn build_external_webhook_state(
    use_cases: &crate::services::use_cases::UseCases,
    agent_task_client: SharedAgentTaskClient,
) -> entrypoint_external_webhook::ExternalWebhookState {
    entrypoint_external_webhook::ExternalWebhookState {
        trigger_use_cases: use_cases.triggers(),
        agent_task_client,
    }
}

#[derive(OpenApi)]
#[openapi(paths(health_check))]
struct HealthDoc;

/// ヘルスチェックレスポンス
#[derive(Serialize, ToSchema)]
struct HealthResponse {
    /// サービスの状態
    status: String,
}

/// OpenAPI スペックを生成する (DB 接続不要)
pub fn create_openapi_spec() -> utoipa::openapi::OpenApi {
    let mut openapi = entrypoint_frontend_api::router().into_openapi();
    openapi.merge(HealthDoc::openapi());
    openapi.merge(entrypoint_agent_webhook::router().into_openapi());
    openapi.merge(entrypoint_external_webhook::router().into_openapi());
    openapi
}

pub fn create_router(
    state: FrontendApiState,
    agent_webhook_state: entrypoint_agent_webhook::AgentWebhookState,
    external_webhook_state: entrypoint_external_webhook::ExternalWebhookState,
    mcp_use_cases: crate::services::use_cases::UseCases,
    mcp_daily_bar_source: Option<SharedDailyBarSource>,
    health_db: DatabaseHandle,
) -> Router {
    let agent_task_client = state.agent_task_client.clone();
    let kata_executor = state.kata_executor.clone();
    let llm_gateway_client = state.llm_gateway_client.clone();
    let agent_webhook_router =
        entrypoint_agent_webhook::router().with_state::<()>(agent_webhook_state);
    let external_webhook_router =
        entrypoint_external_webhook::router().with_state::<()>(external_webhook_state);
    let (router, mut api) = entrypoint_frontend_api::router()
        .with_state::<()>(state)
        .merge(agent_webhook_router)
        .merge(external_webhook_router)
        .split_for_parts();
    api.merge(HealthDoc::openapi());
    let health_router = Router::new()
        .route("/api/health", get(health_check))
        .with_state(health_db);

    router
        .merge(health_router)
        .layer(axum::middleware::from_fn(middleware::reject_null_bytes))
        .merge(SwaggerUi::new("/api-docs").url("/api-docs/openapi.json", api))
        .merge(mcp::router(
            mcp_use_cases,
            agent_task_client,
            mcp_daily_bar_source,
            kata_executor,
            llm_gateway_client,
            mcp::allowed_hosts_from_env(),
        ))
}

#[cfg(test)]
mod health_check_tests {
    use axum::routing::get;
    use axum_test::TestServer;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn database_query_failure_returns_internal_http_response() {
        let router = Router::new()
            .route("/api/health", get(health_check))
            .with_state(DatabaseHandle::from(
                MockDatabase::new(DatabaseBackend::Postgres).into_connection(),
            ));
        let server = TestServer::new(router).expect("create health check test server");
        let response = server.get("/api/health").await;

        assert_eq!(
            (response.status_code(), response.json::<serde_json::Value>()),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "internal server error" }),
            ),
        );
    }
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
    State(db): State<DatabaseHandle>,
) -> Result<(StatusCode, Json<HealthResponse>), AppError> {
    db.execute_unprepared("SELECT 1")
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;

    Ok((
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok".to_string(),
        }),
    ))
}
