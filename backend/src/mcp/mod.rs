//! MCP (Model Context Protocol) サーバーの実装
//!
//! - `/mcp/mgmt`: personal-bot などのコントロールプレーンが叩く管理 MCP
//! - `/mcp/strategy`: 戦略 Agent が叩く戦略実行 MCP

pub mod mgmt;
pub mod store;
pub mod strategy;
pub mod watcher;

use std::sync::Arc;

use axum::Router;
use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::SessionStore;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::tower::StreamableHttpServerConfig;
use sea_orm::DatabaseConnection;

use crate::agent_client::SharedAgentTaskClient;
use crate::data_provider::DataProviderKind;
use crate::kata_exec::SharedKataExecutor;
pub use mgmt::MgmtServer;
pub use store::PostgresSessionStore;
pub use strategy::StrategyServer;

/// MCP ルータを構築する。
///
/// session の `initialize` パラメータを PostgreSQL に永続化し、バックエンド再起動を
/// 跨いだ `mcp-session-id` で来たリクエストを transparently に再開する。
pub fn router(
    db: DatabaseConnection,
    agent_client: SharedAgentTaskClient,
    data_provider: Option<Arc<DataProviderKind>>,
    kata_executor: Option<SharedKataExecutor>,
    extra_allowed_hosts: Vec<String>,
) -> Router {
    let session_store: Arc<dyn SessionStore> = Arc::new(PostgresSessionStore::new(db.clone()));

    let mgmt_db = db.clone();
    let mgmt = StreamableHttpService::new(
        move || Ok(MgmtServer::new(mgmt_db.clone(), agent_client.clone())),
        LocalSessionManager::default().into(),
        build_config(session_store.clone(), &extra_allowed_hosts),
    );
    let strategy = StreamableHttpService::new(
        move || {
            Ok(StrategyServer::new(db.clone(), data_provider.clone())
                .with_kata_executor(kata_executor.clone()))
        },
        LocalSessionManager::default().into(),
        build_config(session_store, &extra_allowed_hosts),
    );

    Router::new()
        .nest_service("/mcp/mgmt", mgmt)
        .nest_service("/mcp/strategy", strategy)
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

fn build_config(
    store: Arc<dyn SessionStore>,
    extra_allowed_hosts: &[String],
) -> StreamableHttpServerConfig {
    let mut config = StreamableHttpServerConfig::default();
    config.session_store = Some(store);
    for host in extra_allowed_hosts {
        if !config.allowed_hosts.contains(host) {
            config.allowed_hosts.push(host.clone());
        }
    }
    config
}

#[cfg(test)]
mod tests {
    use axum_test::TestServer;
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    fn initialize_body() -> serde_json::Value {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test-client", "version": "0.0.0" },
            },
        })
    }

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

    /// MCP Streamable HTTP は Accept に `text/event-stream` を含むと
    /// `data: <json>` 形式の SSE で返す。先頭 keep-alive 行は空 payload なので、
    /// JSON としてパース可能な最初の `data:` 行を返す。
    fn parse_initialize_response(body: &str) -> serde_json::Value {
        body.lines()
            .filter_map(|line| line.strip_prefix("data:").map(str::trim))
            .find_map(|payload| serde_json::from_str(payload).ok())
            .expect("no JSON data line in MCP initialize response")
    }

    async fn maybe_db() -> Option<sea_orm::DatabaseConnection> {
        let url = std::env::var("TEST_DATABASE_URL").ok()?;
        sea_orm::Database::connect(&url).await.ok()
    }

    fn test_agent_client() -> SharedAgentTaskClient {
        Arc::new(crate::agent_client::DisabledAgentTaskClient)
    }

    /// `mcp-session-id` ヘッダを SSE レスポンスから抽出する。
    fn extract_session_id(response: &axum_test::TestResponse) -> String {
        response
            .headers()
            .get("mcp-session-id")
            .expect("no mcp-session-id header")
            .to_str()
            .expect("non-ascii session id")
            .to_owned()
    }

    /// MCP spec 2026-07-28 (SEP-2567) は `initialize` から session の概念を除き、
    /// このバージョンを negotiate したリクエストは stateless に処理される
    /// (`mcp-session-id` が発行されない)。それ以前のバージョンとの挙動差を固定する。
    #[rstest]
    #[case::mgmt_legacy("/mcp/mgmt", "t-rader-mgmt", initialize_body(), "2025-06-18", true)]
    #[case::strategy_legacy(
        "/mcp/strategy",
        "t-rader-strategy",
        initialize_body(),
        "2025-06-18",
        true
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
        let server = TestServer::new(router(db, test_agent_client(), None, None, Vec::new()))
            .expect("failed to build test server");

        let response = server
            .post(path)
            .add_header("accept", "application/json, text/event-stream")
            .json(&body)
            .await;

        response.assert_status_ok();

        assert_eq!(
            parse_initialize_response(&response.text()),
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
        let server = TestServer::new(router(db, test_agent_client(), None, None, Vec::new()))
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

        let body = parse_initialize_response(&response.text());
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
                "create_rss_feed",
                "delete_rss_feed",
                "get_strategy_task_status",
                "list_recent_annotations",
                "list_recent_notes",
                "list_rss_feeds",
                "list_strategies",
                "submit_strategy_task",
                "update_rss_feed",
            ]
        );
    }

    /// 別プロセスを模した 2 つの router で session を継続できることを検証する。
    ///
    /// 1. router A で initialize して session を発行・PostgreSQL に永続化
    /// 2. router A を drop (= プロセス再起動相当、in-memory state を捨てる)
    /// 3. router B を同じ DB で構築し、A で発行された `mcp-session-id` で
    ///    GET /mcp/mgmt にアクセスして 404 ではなく SSE が確立できることを確認
    #[tokio::test]
    async fn resumes_session_after_restart() {
        let Some(db) = maybe_db().await else {
            eprintln!("TEST_DATABASE_URL not set; skipping");
            return;
        };

        let session_id = {
            let server_a = TestServer::new(router(
                db.clone(),
                test_agent_client(),
                None,
                None,
                Vec::new(),
            ))
            .expect("failed to build server A");
            let resp = server_a
                .post("/mcp/mgmt")
                .add_header("accept", "application/json, text/event-stream")
                .json(&initialize_body())
                .await;
            resp.assert_status_ok();
            extract_session_id(&resp)
        };

        // server_a は drop されたので in-memory session も消えている。
        // 別 router (= 再起動後のプロセス) で同じ session_id が受け付けられるはず。
        let server_b = TestServer::new(router(
            db.clone(),
            test_agent_client(),
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

        // 404 (= session 未知) ではなく、SSE ストリームが確立されることを確認。
        assert_ne!(
            resume.status_code(),
            axum::http::StatusCode::NOT_FOUND,
            "session was not restored from PostgreSQL after restart"
        );

        // 後片付け
        let store = PostgresSessionStore::new(db);
        store
            .delete(&session_id)
            .await
            .expect("failed to clean up session row");
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

        let store: Arc<dyn SessionStore> = Arc::new(PostgresSessionStore::new(
            sea_orm::DatabaseConnection::default(),
        ));
        let config = build_config(store, &[extra_host.clone(), default_hosts[0].clone()]);

        let mut expected = default_hosts;
        expected.push(extra_host);
        assert_eq!(config.allowed_hosts, expected);
    }

    /// rmcp の DNS rebinding 保護が in-cluster Service DNS を弾く挙動の回帰テスト。
    #[tokio::test]
    async fn rejects_in_cluster_host_header_without_extra_allowed_hosts() {
        let Some(db) = maybe_db().await else {
            eprintln!("TEST_DATABASE_URL not set; skipping");
            return;
        };
        let server = TestServer::new(router(db, test_agent_client(), None, None, Vec::new()))
            .expect("failed to build test server");

        let response = server
            .post("/mcp/mgmt")
            .add_header("accept", "application/json, text/event-stream")
            .add_header("host", "t-rader-backend.t-rader.svc.cluster.local:3000")
            .json(&initialize_body())
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
            db,
            test_agent_client(),
            None,
            None,
            vec!["t-rader-backend.t-rader.svc.cluster.local".to_string()],
        ))
        .expect("failed to build test server");

        let response = server
            .post("/mcp/mgmt")
            .add_header("accept", "application/json, text/event-stream")
            .add_header("host", "t-rader-backend.t-rader.svc.cluster.local:3000")
            .json(&initialize_body())
            .await;

        response.assert_status_ok();
    }
}
