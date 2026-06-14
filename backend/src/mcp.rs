//! MCP (Model Context Protocol) サーバーの実装
//!
//! - `/mcp/mgmt`: personal-bot などのコントロールプレーンが叩く管理 MCP
//! - `/mcp/strategy`: 戦略 Agent が叩く戦略実行 MCP

use axum::Router;
use rmcp::ServerHandler;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;

/// 管理 MCP: 戦略タスク投入や状態参照のエントリポイント
#[derive(Clone, Default)]
pub struct MgmtServer;

impl ServerHandler for MgmtServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("t-rader-mgmt", env!("CARGO_PKG_VERSION")),
        )
    }
}

/// 戦略実行 MCP: 個別戦略 Agent がノート / アノテーションを書き込むエントリポイント
#[derive(Clone, Default)]
pub struct StrategyServer;

impl ServerHandler for StrategyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("t-rader-strategy", env!("CARGO_PKG_VERSION")),
        )
    }
}

/// MCP server 群を Axum router として返す
///
/// 上位の `create_router` がこれを merge して `/mcp/mgmt` と `/mcp/strategy` を露出する。
pub fn router() -> Router {
    let mgmt = StreamableHttpService::new(
        || Ok(MgmtServer),
        LocalSessionManager::default().into(),
        Default::default(),
    );
    let strategy = StreamableHttpService::new(
        || Ok(StrategyServer),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    Router::new()
        .nest_service("/mcp/mgmt", mgmt)
        .nest_service("/mcp/strategy", strategy)
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

    #[rstest]
    #[case::mgmt("/mcp/mgmt", "t-rader-mgmt")]
    #[case::strategy("/mcp/strategy", "t-rader-strategy")]
    #[tokio::test]
    async fn responds_to_initialize(#[case] path: &str, #[case] expected_name: &str) {
        let server = TestServer::new(router()).expect("failed to build test server");

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
}
