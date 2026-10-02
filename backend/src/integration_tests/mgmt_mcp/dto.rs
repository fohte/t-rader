//! 管理 MCP tool の入出力スキーマ。

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use core_application::rss_feed::RssFeed;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct StrategySummary {
    pub strategy_id: Uuid,
    pub name: String,
    pub updated_at: DateTime<FixedOffset>,
    /// status='unread' のノート + アノテーション件数の合計
    pub unread_card_count: u64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ListStrategiesResult {
    pub strategies: Vec<StrategySummary>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct SubmitStrategyTaskParams {
    pub strategy_id: Uuid,
    pub prompt: String,
    /// タスク実行に使う `agent_config` テーブルの purpose キー。省略時は
    /// `core_application::strategy_task::DEFAULT_PURPOSE` を使う。対応する `agent_config` 行が存在しない
    /// purpose を指定した場合はこの呼び出し自体がエラーになる。
    #[serde(default)]
    pub purpose: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct SubmitStrategyTaskResult {
    pub task_id: Uuid,
    pub a2a_task_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct GetStrategyTaskStatusParams {
    pub a2a_task_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ResumeStrategyTaskParams {
    pub task_id: Uuid,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ResumeStrategyTaskResult {
    pub task_id: Uuid,
    pub a2a_task_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct GetStrategyTaskStatusResult {
    pub task_id: Uuid,
    pub strategy_id: Uuid,
    pub a2a_task_id: Option<String>,
    pub phase: String,
    pub error_summary: Option<String>,
    pub result_text: Option<String>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ListRecentParams {
    pub strategy_id: Uuid,
    /// 取得件数 (デフォルト 20、最大 100)
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct NoteMeta {
    pub note_id: Uuid,
    pub title: String,
    pub status: String,
    pub created_by_kind: String,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ListRecentNotesResult {
    pub notes: Vec<NoteMeta>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct AnnotationMeta {
    pub annotation_id: Uuid,
    pub target_symbol: String,
    pub target_kind: String,
    pub status: String,
    pub created_by_kind: String,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ListRecentAnnotationsResult {
    pub annotations: Vec<AnnotationMeta>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ListRssFeedsParams {
    /// true なら enabled=true の行のみ返す
    #[serde(default)]
    pub enabled_only: Option<bool>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct RssFeedSummary {
    pub id: Uuid,
    pub source: String,
    pub display_name: String,
    pub url: String,
    pub enabled: bool,
}

impl From<RssFeed> for RssFeedSummary {
    fn from(m: RssFeed) -> Self {
        Self {
            id: m.id,
            source: m.source,
            display_name: m.display_name,
            url: m.url,
            enabled: m.enabled,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ListRssFeedsResult {
    pub feeds: Vec<RssFeedSummary>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TriggerSummary {
    pub trigger_id: Uuid,
    pub purpose: Option<String>,
    pub kind: String,
    pub schedule: Option<String>,
    pub hook_slug: Option<String>,
    /// trigger use case が書き込み時に object または null のみに制限する。
    pub event_match: Option<serde_json::Map<String, serde_json::Value>>,
    pub prompt_template: String,
    pub enabled: bool,
    pub last_fired_at: Option<DateTime<FixedOffset>>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

impl From<core_application::trigger::Trigger> for TriggerSummary {
    fn from(trigger: core_application::trigger::Trigger) -> Self {
        Self {
            trigger_id: trigger.trigger_id,
            purpose: trigger.purpose,
            kind: trigger.kind.as_str().to_string(),
            schedule: trigger.schedule,
            hook_slug: trigger.hook_slug,
            event_match: trigger
                .event_match
                .and_then(|value| value.as_object().cloned()),
            prompt_template: trigger.prompt_template,
            enabled: trigger.enabled,
            last_fired_at: trigger.last_fired_at,
            created_at: trigger.created_at,
            updated_at: trigger.updated_at,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct GetStrategyConfigParams {
    pub strategy_id: Uuid,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct GetStrategyConfigResult {
    pub strategy_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub triggers: Vec<TriggerSummary>,
}
