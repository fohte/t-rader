//! 管理 MCP tool の入出力スキーマ。

use chrono::{DateTime, FixedOffset};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::rss_feed;

#[derive(Debug, Serialize, JsonSchema)]
pub struct StrategySummary {
    pub strategy_id: Uuid,
    pub name: String,
    pub updated_at: DateTime<FixedOffset>,
    /// status='unread' のノート + アノテーション件数の合計
    pub unread_card_count: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListStrategiesResult {
    pub strategies: Vec<StrategySummary>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubmitStrategyTaskParams {
    pub strategy_id: Uuid,
    pub prompt: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SubmitStrategyTaskResult {
    pub task_id: Uuid,
    pub a2a_task_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetStrategyTaskStatusParams {
    pub a2a_task_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetStrategyTaskStatusResult {
    pub task_id: Uuid,
    pub strategy_id: Uuid,
    pub a2a_task_id: Option<String>,
    pub phase: String,
    pub error_summary: Option<String>,
    pub result_text: Option<String>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRecentParams {
    pub strategy_id: Uuid,
    /// 取得件数 (デフォルト 20、最大 100)
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct NoteMeta {
    pub note_id: Uuid,
    pub title: String,
    pub status: String,
    pub created_by_kind: String,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListRecentNotesResult {
    pub notes: Vec<NoteMeta>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AnnotationMeta {
    pub annotation_id: Uuid,
    pub target_symbol: String,
    pub target_kind: String,
    pub status: String,
    pub created_by_kind: String,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListRecentAnnotationsResult {
    pub annotations: Vec<AnnotationMeta>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRssFeedsParams {
    /// true なら enabled=true の行のみ返す
    #[serde(default)]
    pub enabled_only: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RssFeedSummary {
    pub id: Uuid,
    pub source: String,
    pub display_name: String,
    pub url: String,
    pub enabled: bool,
}

impl From<rss_feed::Model> for RssFeedSummary {
    fn from(m: rss_feed::Model) -> Self {
        Self {
            id: m.id,
            source: m.source,
            display_name: m.display_name,
            url: m.url,
            enabled: m.enabled,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListRssFeedsResult {
    pub feeds: Vec<RssFeedSummary>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateRssFeedParams {
    pub source: String,
    pub display_name: String,
    pub url: String,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateRssFeedParams {
    pub id: Uuid,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteRssFeedParams {
    pub id: Uuid,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteRssFeedResult {
    pub id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetStrategyAgentConfigParams {
    pub strategy_id: Uuid,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetStrategyAgentConfigResult {
    pub yaml: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PutStrategyAgentConfigParams {
    pub strategy_id: Uuid,
    pub yaml: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PutStrategyAgentConfigResult {
    pub ok: bool,
    pub errors: Vec<String>,
}
