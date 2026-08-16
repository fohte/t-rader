use serde::Serialize;
use utoipa::ToSchema;

/// LiteLLM `/model_group/info` の必要フィールドだけを写した 1 モデル分。
#[derive(Debug, PartialEq, Serialize, ToSchema)]
pub struct AgentModel {
    pub id: String,
    pub providers: Vec<String>,
    pub max_input_tokens: Option<f64>,
    pub max_output_tokens: Option<f64>,
    pub supports_reasoning: bool,
    pub supports_web_search: bool,
}

/// `GET /api/agent-models` の戻り値。LiteLLM が未設定/応答不能なら `models` は空配列。
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentModelsResponse {
    pub models: Vec<AgentModel>,
}

/// 戦略 MCP の tool 1 件分。
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentTool {
    pub name: String,
    pub description: Option<String>,
}

/// `GET /api/agent-tools` の戻り値。
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentToolsResponse {
    pub tools: Vec<AgentTool>,
}
