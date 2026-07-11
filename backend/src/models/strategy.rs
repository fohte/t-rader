use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateStrategyRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateStrategyRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: Option<String>,
    pub description: Option<String>,
    pub sort_order: Option<i32>,
}

/// フローティングチャットから戦略 Agent に投入する 1 メッセージ。
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StrategyChatRequest {
    #[schema(min_length = 1)]
    pub prompt: String,
}

/// `POST /api/strategies/:id/chat` の戻り値。後続の polling 用 task 識別子を返す。
#[derive(Debug, Serialize, ToSchema)]
pub struct StrategyChatResponse {
    pub task_id: Uuid,
    pub kubeopencode_task_name: String,
}

/// `GET /api/strategies/:id/tasks/:task_id` の戻り値。
#[derive(Debug, Serialize, ToSchema)]
pub struct StrategyTaskStatusResponse {
    pub task_id: Uuid,
    pub strategy_id: Uuid,
    pub kubeopencode_task_name: String,
    pub source: String,
    pub phase: String,
    pub error_summary: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentsMdBody {
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillBody {
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillsBody {
    pub skills: BTreeMap<String, String>,
}

/// t-rader-agent がタスク実行時に取得する agent 設定一式。
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentConfigResponse {
    pub agents_md: String,
    pub skills: BTreeMap<String, String>,
    pub model: String,
    pub small_model: String,
}
