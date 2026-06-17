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

use crate::data_provider::DataProviderKind;
use crate::kata_exec::SharedKataExecutor;
use crate::kubeopencode::SharedKubeopencodeClient;
pub use mgmt::MgmtServer;
pub use store::PostgresSessionStore;
pub use strategy::StrategyServer;

/// MCP ルータを構築する。
///
/// session の `initialize` パラメータを PostgreSQL に永続化し、バックエンド再起動を
/// 跨いだ `mcp-session-id` で来たリクエストを transparently に再開する。
pub fn router(
    db: DatabaseConnection,
    kube: SharedKubeopencodeClient,
    data_provider: Option<Arc<DataProviderKind>>,
    kata_executor: Option<SharedKataExecutor>,
) -> Router {
    let session_store: Arc<dyn SessionStore> = Arc::new(PostgresSessionStore::new(db.clone()));

    let mgmt_db = db.clone();
    let mgmt = StreamableHttpService::new(
        move || Ok(MgmtServer::new(mgmt_db.clone(), kube.clone())),
        LocalSessionManager::default().into(),
        config_with_store(session_store.clone()),
    );
    let strategy = StreamableHttpService::new(
        move || {
            Ok(StrategyServer::new(db.clone(), data_provider.clone())
                .with_kata_executor(kata_executor.clone()))
        },
        LocalSessionManager::default().into(),
        config_with_store(session_store),
    );

    Router::new()
        .nest_service("/mcp/mgmt", mgmt)
        .nest_service("/mcp/strategy", strategy)
}

fn config_with_store(store: Arc<dyn SessionStore>) -> StreamableHttpServerConfig {
    let mut config = StreamableHttpServerConfig::default();
    config.session_store = Some(store);
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

    fn test_kube() -> SharedKubeopencodeClient {
        Arc::new(crate::kubeopencode::client::DisabledKubeopencodeClient)
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

    #[rstest]
    #[case::mgmt("/mcp/mgmt", "t-rader-mgmt")]
    #[case::strategy("/mcp/strategy", "t-rader-strategy")]
    #[tokio::test]
    async fn responds_to_initialize(#[case] path: &str, #[case] expected_name: &str) {
        let Some(db) = maybe_db().await else {
            eprintln!("TEST_DATABASE_URL not set; skipping");
            return;
        };
        let server = TestServer::new(router(db, test_kube(), None, None))
            .expect("failed to build test server");

        let response = server
            .post(path)
            .add_header("accept", "application/json, text/event-stream")
            .json(&initialize_body())
            .await;

        response.assert_status_ok();

        assert_eq!(
            parse_initialize_response(&response.text()),
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": expected_name,
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                },
            }),
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
            let server_a = TestServer::new(router(db.clone(), test_kube(), None, None))
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
        let server_b = TestServer::new(router(db.clone(), test_kube(), None, None))
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
}
