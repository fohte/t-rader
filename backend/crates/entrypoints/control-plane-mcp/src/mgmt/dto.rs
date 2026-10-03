//! 管理 MCP tool の入出力スキーマ。

use chrono::{DateTime, FixedOffset};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use core_application::rss_feed::RssFeed;

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
    /// タスク実行に使う `agent_config` テーブルの purpose キー。省略時は
    /// `core_application::strategy_task::DEFAULT_PURPOSE` を使う。対応する `agent_config` 行が存在しない
    /// purpose を指定した場合はこの呼び出し自体がエラーになる。
    #[serde(default)]
    pub purpose: Option<String>,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ResumeStrategyTaskParams {
    pub task_id: Uuid,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ResumeStrategyTaskResult {
    pub task_id: Uuid,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateRssFeedParams {
    pub id: Uuid,
    /// 本文の取得元。省略時は現在の設定を維持する。
    #[serde(default)]
    pub content_source: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RssFeedSummary {
    pub id: Uuid,
    pub source: String,
    pub display_name: String,
    pub url: String,
    pub enabled: bool,
    pub content_source: String,
}

impl From<RssFeed> for RssFeedSummary {
    fn from(m: RssFeed) -> Self {
        Self {
            id: m.id,
            source: m.source,
            display_name: m.display_name,
            url: m.url,
            enabled: m.enabled,
            content_source: m.content_source,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListRssFeedsResult {
    pub feeds: Vec<RssFeedSummary>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct NoteKindSummary {
    pub key: String,
    pub display_name: String,
    pub requires_approval: bool,
    pub description: Option<String>,
    pub sort_order: i32,
}

impl From<core_application::note_kind::NoteKind> for NoteKindSummary {
    fn from(model: core_application::note_kind::NoteKind) -> Self {
        Self {
            key: model.key,
            display_name: model.display_name,
            requires_approval: model.requires_approval,
            description: model.description,
            sort_order: model.sort_order,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListNoteKindsResult {
    pub note_kinds: Vec<NoteKindSummary>,
}

#[derive(Debug, PartialEq, Serialize, JsonSchema)]
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetStrategyConfigParams {
    pub strategy_id: Uuid,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetStrategyConfigResult {
    pub strategy_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub triggers: Vec<TriggerSummary>,
}
