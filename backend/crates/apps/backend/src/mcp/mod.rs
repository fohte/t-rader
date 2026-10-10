//! MCP (Model Context Protocol) サーバーの実装
//!
//! - `/mcp/mgmt`: 上流のコントロールプレーンが叩く管理 MCP
//! - `/mcp/strategy`: 戦略 Agent が叩く戦略実行 MCP

mod access_log;

use std::sync::Arc;
use std::time::Duration;

use crate::services::use_cases::UseCases;
use axum::Router;
use core_application::agent_task_client::SharedAgentTaskClient;
use core_application::daily_bar_source::SharedDailyBarSource;
use core_application::kata_exec::SharedKataExecutor;
use core_application::llm_client::SharedLlmClient;
use core_application::strategy_task::DEADLINE_DURATION;
pub use entrypoint_agent_mcp::{StrategyServer, StrategyServerDependencies};
use entrypoint_control_plane_mcp::{MgmtDependencies, MgmtServer};
use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::tower::StreamableHttpServerConfig;

/// MCP ルータを構築する。mgmt はプロセス内 session、strategy は stateless で処理する。
pub fn router(
    use_cases: UseCases,
    agent_client: SharedAgentTaskClient,
    daily_bar_source: Option<SharedDailyBarSource>,
    kata_executor: Option<SharedKataExecutor>,
    litellm_client: Option<SharedLlmClient>,
    extra_allowed_hosts: Vec<String>,
) -> Router {
    let mgmt_dependencies = mgmt_dependencies(&use_cases, agent_client);
    let mgmt = StreamableHttpService::new(
        move || Ok(MgmtServer::new(mgmt_dependencies.clone())),
        mgmt_session_manager().into(),
        build_config(&extra_allowed_hosts),
    );
    let strategy = StreamableHttpService::new(
        move || {
            Ok(StrategyServer::new(strategy_server_dependencies(
                &use_cases,
                daily_bar_source.clone(),
                kata_executor.clone(),
                litellm_client.clone(),
            )))
        },
        NeverSessionManager::default().into(),
        strategy_config(&extra_allowed_hosts),
    );

    Router::new()
        .nest_service("/mcp/mgmt", mgmt)
        .nest_service("/mcp/strategy", strategy)
        .layer(axum::middleware::from_fn_with_state(
            access_log::AccessLogState::new(),
            access_log::access_log,
        ))
}

pub(crate) fn mgmt_dependencies(
    use_cases: &UseCases,
    agent_client: SharedAgentTaskClient,
) -> MgmtDependencies {
    MgmtDependencies {
        strategies: use_cases.strategies(),
        strategy_scope: Arc::new(use_cases.strategy_scope()),
        strategy_tasks: use_cases.strategy_tasks(),
        triggers: use_cases.triggers(),
        note_kinds: use_cases.note_kinds(),
        note_reads: use_cases.note_reads(),
        note_status_change_aggregate: use_cases.note_status_change_aggregate(),
        annotation_reads: use_cases.annotation_reads(),
        rss_feeds: use_cases.rss_feeds(),
        agent_client,
    }
}

pub(crate) fn strategy_server_dependencies(
    use_cases: &UseCases,
    daily_bar_source: Option<SharedDailyBarSource>,
    kata_executor: Option<SharedKataExecutor>,
    llm_client: Option<SharedLlmClient>,
) -> StrategyServerDependencies {
    StrategyServerDependencies {
        account_risk_policies: use_cases.account_risk_policies(),
        annotation_reads: use_cases.annotation_reads(),
        annotations: use_cases.annotations(),
        bars: use_cases.bars(),
        calendar_event_reads: use_cases.calendar_event_reads(),
        comment_reads: use_cases.comment_reads(),
        comments: use_cases.comments(),
        custom_indicators: use_cases.custom_indicators(),
        daily_bar_source,
        financial_summaries: use_cases.financial_summaries(),
        indicator_observations: use_cases.indicator_observations(),
        kata_executor,
        llm_client,
        margins: use_cases.margins(),
        market_movers: use_cases.market_movers(),
        mcp_tool_call_counts: use_cases.mcp_tool_call_counts(),
        news: use_cases.news(),
        note_kinds: use_cases.note_kinds(),
        note_reads: use_cases.note_reads(),
        notes: use_cases.notes(),
        paper_trades: use_cases.paper_trade(),
        predictions: use_cases.predictions(),
        refs: use_cases.refs(),
        shareholding_structures: use_cases.shareholding_structures(),
        short_ratios: use_cases.short_ratios(),
        short_sale_reports: use_cases.short_sale_reports(),
        stock_registration: use_cases.stock_registration(),
        stock_groups: use_cases.stock_groups(),
        strategy_earnings_targets: use_cases.strategy_earnings_targets(),
        strategies: use_cases.strategies(),
        strategy_scope: Arc::new(use_cases.strategy_scope()),
        strategy_tasks: use_cases.strategy_tasks(),
        strategy_task_step_evidence: use_cases.strategy_task_step_evidence(),
        trades: use_cases.trades(),
        valuations: use_cases.valuations(),
    }
}

/// `MCP_ALLOWED_HOSTS` (カンマ区切り) をパースする。未設定または空なら空 Vec。
pub fn allowed_hosts_from_env() -> Vec<String> {
    let hosts = std::env::var("MCP_ALLOWED_HOSTS")
        .ok()
        .map(|raw| parse_allowed_hosts(&raw))
        .unwrap_or_default();
    if !hosts.is_empty() {
        tracing::info!(
            extra_allowed_hosts = ?hosts,
            "MCP allowed hosts extended from MCP_ALLOWED_HOSTS"
        );
    }
    hosts
}

fn parse_allowed_hosts(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// デフォルトの idle timeout (5 分) で mgmt MCP のタスク実行中に session が破棄されないよう、
/// deadline を超える keep_alive を設定する。
fn mgmt_session_manager() -> LocalSessionManager {
    let mut manager = LocalSessionManager::default();
    manager.session_config.keep_alive = Some(mgmt_session_keep_alive());
    manager
}

fn mgmt_session_keep_alive() -> Duration {
    // to_std() は負の Duration でのみ失敗する (起こらない想定)。フォールバックは
    // deadline を下回らない安全側 (Duration::MAX) にする。
    DEADLINE_DURATION
        .to_std()
        .map(|deadline| deadline + Duration::from_secs(60))
        .unwrap_or(Duration::MAX)
}

fn build_config(extra_allowed_hosts: &[String]) -> StreamableHttpServerConfig {
    let mut config = StreamableHttpServerConfig::default();
    for host in extra_allowed_hosts {
        if !config.allowed_hosts.contains(host) {
            config.allowed_hosts.push(host.clone());
        }
    }
    config
}

fn strategy_config(extra_allowed_hosts: &[String]) -> StreamableHttpServerConfig {
    build_config(extra_allowed_hosts).with_legacy_session_mode(false)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum_test::TestServer;
    use rstest::rstest;
    use serde_json::json;

    use crate::testing::mcp::{legacy_initialize_body, parse_sse_response};

    use super::*;

    fn initialize_body_v2026_07_28() -> serde_json::Value {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2026-07-28",
                "capabilities": {},
                "clientInfo": { "name": "test-client-2026", "version": "0.0.0" },
            },
        })
    }

    async fn maybe_db() -> Option<sea_orm::DatabaseConnection> {
        let url = std::env::var("TEST_DATABASE_URL").ok()?;
        sea_orm::Database::connect(&url).await.ok()
    }

    fn test_agent_client() -> SharedAgentTaskClient {
        Arc::new(core_application::agent_task_client::DisabledAgentTaskClient)
    }

    /// MCP spec 2026-07-28 (SEP-2567) は `initialize` から session の概念を除き、
    /// このバージョンを negotiate したリクエストは stateless に処理される
    /// (`mcp-session-id` が発行されない)。それ以前のバージョンとの挙動差を固定する。
    #[rstest]
    #[case::mgmt_legacy(
        "/mcp/mgmt",
        "t-rader-mgmt",
        legacy_initialize_body(),
        "2025-06-18",
        true
    )]
    #[case::strategy_legacy_stateless(
        "/mcp/strategy",
        "t-rader-strategy",
        legacy_initialize_body(),
        "2025-06-18",
        false
    )]
    #[case::mgmt_2026_07_28(
        "/mcp/mgmt",
        "t-rader-mgmt",
        initialize_body_v2026_07_28(),
        "2026-07-28",
        false
    )]
    #[case::strategy_2026_07_28(
        "/mcp/strategy",
        "t-rader-strategy",
        initialize_body_v2026_07_28(),
        "2026-07-28",
        false
    )]
    #[tokio::test]
    async fn responds_to_initialize(
        #[case] path: &str,
        #[case] expected_name: &str,
        #[case] body: serde_json::Value,
        #[case] expected_protocol_version: &str,
        #[case] expect_session_id: bool,
    ) {
        let Some(db) = maybe_db().await else {
            eprintln!("TEST_DATABASE_URL not set; skipping");
            return;
        };
        let server = TestServer::new(router(
            crate::services::use_cases::build_use_cases(db.clone()),
            test_agent_client(),
            None,
            None,
            None,
            Vec::new(),
        ))
        .expect("failed to build test server");

        let response = server
            .post(path)
            .add_header("accept", "application/json, text/event-stream")
            .json(&body)
            .await;

        response.assert_status_ok();

        assert_eq!(
            parse_sse_response(&response.text()),
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "protocolVersion": expected_protocol_version,
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": expected_name,
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                },
            }),
        );
        assert_eq!(
            response.headers().get("mcp-session-id").is_some(),
            expect_session_id,
            "mcp-session-id header presence should match the negotiated protocol generation"
        );
    }

    /// 2026-07-28 世代クライアントは、`initialize` を経ず `MCP-Protocol-Version` ヘッダのみで
    /// 直接 `tools/list` を呼べる (SEP-2575 discover lifecycle)。この場合 SEP-2243 の
    /// `Mcp-Method` ヘッダが必須になる。tool 同期に相当するこの経路が正しく動くことの回帰テスト。
    #[tokio::test]
    async fn lists_tools_statelessly_for_2026_07_28_with_standard_headers() {
        let Some(db) = maybe_db().await else {
            eprintln!("TEST_DATABASE_URL not set; skipping");
            return;
        };
        let server = TestServer::new(router(
            crate::services::use_cases::build_use_cases(db.clone()),
            test_agent_client(),
            None,
            None,
            None,
            Vec::new(),
        ))
        .expect("failed to build test server");

        let response = server
            .post("/mcp/mgmt")
            .add_header("accept", "application/json, text/event-stream")
            .add_header("mcp-protocol-version", "2026-07-28")
            .add_header("mcp-method", "tools/list")
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/list",
                "params": {
                    "_meta": {
                        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                        "io.modelcontextprotocol/clientCapabilities": {},
                    },
                },
            }))
            .await;

        response.assert_status_ok();

        let body = parse_sse_response(&response.text());
        let mut tool_names: Vec<&str> = body["result"]["tools"]
            .as_array()
            .expect("tools/list result.tools should be an array")
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name should be a string"))
            .collect();
        tool_names.sort_unstable();
        assert_eq!(
            tool_names,
            vec![
                "get_note_status_change_counts",
                "get_strategy_config",
                "get_strategy_task_status",
                "list_note_kinds",
                "list_recent_annotations",
                "list_recent_notes",
                "list_rss_feeds",
                "list_strategies",
                "resume_strategy_task",
                "submit_strategy_task",
                "update_rss_feed",
            ]
        );
    }

    /// router を跨ぐ (= バックエンド再起動相当) と `mcp-session-id` が再開されないことの回帰テスト。
    #[tokio::test]
    async fn does_not_resume_session_after_restart() {
        let Some(db) = maybe_db().await else {
            eprintln!("TEST_DATABASE_URL not set; skipping");
            return;
        };

        let session_id = {
            let server_a = TestServer::new(router(
                crate::services::use_cases::build_use_cases(db.clone()),
                test_agent_client(),
                None,
                None,
                None,
                Vec::new(),
            ))
            .expect("failed to build server A");
            let resp = server_a
                .post("/mcp/mgmt")
                .add_header("accept", "application/json, text/event-stream")
                .json(&legacy_initialize_body())
                .await;
            resp.assert_status_ok();
            resp.headers()
                .get("mcp-session-id")
                .expect("no mcp-session-id header")
                .to_str()
                .expect("non-ascii session id")
                .to_owned()
        };

        // server_a は drop されたので in-memory session も消えている。
        let server_b = TestServer::new(router(
            crate::services::use_cases::build_use_cases(db.clone()),
            test_agent_client(),
            None,
            None,
            None,
            Vec::new(),
        ))
        .expect("failed to build server B");
        let resume = server_b
            .get("/mcp/mgmt")
            .add_header("accept", "text/event-stream")
            .add_header("mcp-session-id", &session_id)
            .await;

        assert_eq!(
            resume.status_code(),
            axum::http::StatusCode::NOT_FOUND,
            "session should not be resumable after a backend restart"
        );
    }

    /// keep_alive が deadline を下回ると、deadline 内でも session が破棄されうる。
    #[test]
    fn mgmt_session_keep_alive_exceeds_task_deadline() {
        let deadline = DEADLINE_DURATION
            .to_std()
            .expect("DEADLINE_DURATION should be a positive duration");
        assert!(
            mgmt_session_keep_alive() > deadline,
            "session keep_alive ({:?}) must exceed the task deadline ({deadline:?})",
            mgmt_session_keep_alive(),
        );
    }

    #[rstest]
    #[case::empty("", Vec::<String>::new())]
    #[case::single("example.com", vec!["example.com".to_string()])]
    #[case::multi_with_port_and_whitespace(
        "example.com, foo.svc.cluster.local:3000 ,bar",
        vec![
            "example.com".to_string(),
            "foo.svc.cluster.local:3000".to_string(),
            "bar".to_string(),
        ],
    )]
    #[case::trailing_comma("a,,b,", vec!["a".to_string(), "b".to_string()])]
    fn parses_env_allowed_hosts(#[case] raw: &str, #[case] expected: Vec<String>) {
        assert_eq!(parse_allowed_hosts(raw), expected);
    }

    #[test]
    fn build_config_keeps_rmcp_defaults_and_appends_extras_without_dup() {
        let extra_host = "t-rader-backend.t-rader.svc.cluster.local".to_string();
        let default_hosts = StreamableHttpServerConfig::default().allowed_hosts;
        assert!(!default_hosts.is_empty());

        let config = build_config(&[extra_host.clone(), default_hosts[0].clone()]);

        let mut expected = default_hosts;
        expected.push(extra_host);
        assert_eq!(config.allowed_hosts, expected);
        assert!(config.session_store.is_none());
    }

    #[test]
    fn strategy_config_disables_legacy_sessions_and_keeps_host_policy() {
        let default_hosts = StreamableHttpServerConfig::default().allowed_hosts;
        let config = strategy_config(&[]);

        assert_eq!(
            (
                config.legacy_session_mode,
                config.allowed_hosts,
                config.session_store.is_some(),
            ),
            (false, default_hosts, false),
        );
    }

    /// rmcp の DNS rebinding 保護が in-cluster Service DNS を弾く挙動の回帰テスト。
    #[tokio::test]
    async fn rejects_in_cluster_host_header_without_extra_allowed_hosts() {
        let Some(db) = maybe_db().await else {
            eprintln!("TEST_DATABASE_URL not set; skipping");
            return;
        };
        let server = TestServer::new(router(
            crate::services::use_cases::build_use_cases(db.clone()),
            test_agent_client(),
            None,
            None,
            None,
            Vec::new(),
        ))
        .expect("failed to build test server");

        let response = server
            .post("/mcp/mgmt")
            .add_header("accept", "application/json, text/event-stream")
            .add_header("host", "t-rader-backend.t-rader.svc.cluster.local:3000")
            .json(&legacy_initialize_body())
            .await;

        assert_eq!(response.status_code(), axum::http::StatusCode::FORBIDDEN);
    }

    /// 環境変数経由で in-cluster Service DNS を許可した場合は initialize が通る。
    #[tokio::test]
    async fn accepts_in_cluster_host_header_when_configured() {
        let Some(db) = maybe_db().await else {
            eprintln!("TEST_DATABASE_URL not set; skipping");
            return;
        };
        let server = TestServer::new(router(
            crate::services::use_cases::build_use_cases(db.clone()),
            test_agent_client(),
            None,
            None,
            None,
            vec!["t-rader-backend.t-rader.svc.cluster.local".to_string()],
        ))
        .expect("failed to build test server");

        let response = server
            .post("/mcp/mgmt")
            .add_header("accept", "application/json, text/event-stream")
            .add_header("host", "t-rader-backend.t-rader.svc.cluster.local:3000")
            .json(&legacy_initialize_body())
            .await;

        response.assert_status_ok();
    }
}
