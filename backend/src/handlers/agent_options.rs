use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::models::{AgentModel, AgentModelsResponse, AgentTool, AgentToolsResponse};

/// 戦略 Agent 設定フォームに供給するモデル一覧を取得する。
/// LLM ゲートウェイが未設定、または応答不能な場合は空配列を返す (設定画面全体を壊さないため)。
#[utoipa::path(
    get,
    path = "/api/agent-models",
    tag = "agent_options",
    responses((status = 200, body = AgentModelsResponse)),
)]
pub async fn get_agent_models(State(state): State<AppState>) -> Json<AgentModelsResponse> {
    let models = match &state.llm_gateway_client {
        Some(client) => client.list_models().await.unwrap_or_else(|e| {
            tracing::warn!(
                error = %e,
                "failed to fetch agent models from llm gateway; returning empty list"
            );
            Vec::new()
        }),
        None => Vec::new(),
    };
    Json(AgentModelsResponse {
        models: models
            .into_iter()
            .map(|model| AgentModel {
                id: model.id,
                providers: model.providers,
                max_input_tokens: model.max_input_tokens,
                max_output_tokens: model.max_output_tokens,
                supports_reasoning: model.supports_reasoning,
            })
            .collect(),
    })
}

/// 戦略 MCP の tool 一覧を取得する。`#[tool(...)]` の登録情報から動的に組み立てるので、
/// tool を追加してもここを手で更新する必要はない。
#[utoipa::path(
    get,
    path = "/api/agent-tools",
    tag = "agent_options",
    responses((status = 200, body = AgentToolsResponse)),
)]
pub async fn get_agent_tools(State(state): State<AppState>) -> Json<AgentToolsResponse> {
    let tools = state
        .agent_tool_summaries
        .iter()
        .cloned()
        .map(|(name, description)| AgentTool { name, description })
        .collect();
    Json(AgentToolsResponse { tools })
}
