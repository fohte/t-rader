use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::mcp::StrategyServer;
use crate::models::{AgentModelsResponse, AgentTool, AgentToolsResponse};

/// 戦略 Agent 設定フォームに供給するモデル一覧を取得する。
/// LiteLLM Proxy が未設定、または応答不能な場合は空配列を返す (設定画面全体を壊さないため)。
#[utoipa::path(
    get,
    path = "/api/agent-models",
    tag = "agent_options",
    responses((status = 200, body = AgentModelsResponse)),
)]
pub async fn get_agent_models(State(state): State<AppState>) -> Json<AgentModelsResponse> {
    let models = match &state.litellm_client {
        Some(client) => client.list_models().await.unwrap_or_else(|e| {
            tracing::warn!(
                error = %e,
                "failed to fetch agent models from litellm; returning empty list"
            );
            Vec::new()
        }),
        None => Vec::new(),
    };
    Json(AgentModelsResponse { models })
}

/// 戦略 MCP の tool 一覧を取得する。`#[tool(...)]` の登録情報から動的に組み立てるので、
/// tool を追加してもここを手で更新する必要はない。
#[utoipa::path(
    get,
    path = "/api/agent-tools",
    tag = "agent_options",
    responses((status = 200, body = AgentToolsResponse)),
)]
pub async fn get_agent_tools() -> Json<AgentToolsResponse> {
    let tools = StrategyServer::list_tool_summaries()
        .into_iter()
        .map(|(name, description)| AgentTool { name, description })
        .collect();
    Json(AgentToolsResponse { tools })
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use crate::testing::{create_test_server, create_test_server_with_litellm};

    #[sqlx::test(migrations = false)]
    async fn agent_models_returns_empty_list_when_litellm_unconfigured(pool: PgPool) {
        let server = create_test_server(pool).await;
        let response = server.get("/api/agent-models").await;
        response.assert_status_ok();
        assert_eq!(
            response.json::<serde_json::Value>(),
            serde_json::json!({ "models": [] }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn agent_models_proxies_litellm_response(pool: PgPool) {
        let litellm = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/model_group/info"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": [
                        {
                            "model_group": "claude-opus-4",
                            "providers": ["anthropic"],
                            "supports_reasoning": true,
                            "supports_web_search": false,
                        },
                    ],
                })),
            )
            .mount(&litellm)
            .await;

        let server = create_test_server_with_litellm(pool, &litellm.uri()).await;
        let response = server.get("/api/agent-models").await;
        response.assert_status_ok();
        assert_eq!(
            response.json::<serde_json::Value>(),
            serde_json::json!({
                "models": [
                    {
                        "id": "claude-opus-4",
                        "providers": ["anthropic"],
                        "max_input_tokens": null,
                        "max_output_tokens": null,
                        "supports_reasoning": true,
                        "supports_web_search": false,
                    },
                ],
            }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn agent_models_returns_empty_list_when_litellm_unreachable(pool: PgPool) {
        let litellm = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/model_group/info"))
            .respond_with(wiremock::ResponseTemplate::new(503))
            .mount(&litellm)
            .await;

        let server = create_test_server_with_litellm(pool, &litellm.uri()).await;
        let response = server.get("/api/agent-models").await;
        response.assert_status_ok();
        assert_eq!(
            response.json::<serde_json::Value>(),
            serde_json::json!({ "models": [] }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn agent_tools_lists_known_strategy_mcp_tools(pool: PgPool) {
        let server = create_test_server(pool).await;
        let response = server.get("/api/agent-tools").await;
        response.assert_status_ok();
        let body = response.json::<serde_json::Value>();
        let names: Vec<&str> = body["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .map(|t| t["name"].as_str().expect("name"))
            .collect();
        // ToolRouter::list_all() は name の昇順でソートして返す
        assert_eq!(
            names,
            vec![
                "add_interest",
                "create_annotation",
                "eval_indicator",
                "eval_python",
                "list_notes",
                "query_data",
                "read_annotations",
                "read_comments",
                "read_note",
                "reply_comment",
                "resolve_comment",
                "write_note",
            ],
        );
    }
}
