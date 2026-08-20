//! 管理 MCP tool の入出力スキーマ。

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{rss_feed, trigger};
use crate::models::TriggerKind;

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

#[derive(Debug, Serialize, JsonSchema)]
pub struct TriggerSummary {
    pub trigger_id: Uuid,
    pub kind: String,
    pub schedule: Option<String>,
    pub hook_slug: Option<String>,
    pub event_match: Option<serde_json::Value>,
    pub prompt_template: String,
    pub enabled: bool,
    pub last_fired_at: Option<DateTime<FixedOffset>>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

impl From<trigger::Model> for TriggerSummary {
    fn from(m: trigger::Model) -> Self {
        Self {
            trigger_id: m.trigger_id,
            kind: m.kind,
            schedule: m.schedule,
            hook_slug: m.hook_slug,
            event_match: m.event_match,
            prompt_template: m.prompt_template,
            enabled: m.enabled,
            last_fired_at: m.last_fired_at,
            created_at: m.created_at,
            updated_at: m.updated_at,
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
    pub agents_md: String,
    pub skills: BTreeMap<String, String>,
    pub agent_graph: String,
    pub triggers: Vec<TriggerSummary>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateStrategyParams {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub agents_md: Option<String>,
    /// skill 名 -> 本文 (markdown)。指定されたものが初期値としてそのまま保存される。
    #[serde(default)]
    pub skills: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub agent_graph: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateStrategyResult {
    pub ok: bool,
    pub errors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateStrategyConfigParams {
    pub strategy_id: Uuid,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub agents_md: Option<String>,
    /// JSON Merge Patch セマンティクス: 値が null のキーは削除、それ以外は追加/更新。
    /// 未指定のキーは変更しない。
    #[serde(default)]
    pub skills: Option<BTreeMap<String, Option<String>>>,
    #[serde(default)]
    pub agent_graph: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateStrategyConfigResult {
    pub ok: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteStrategyParams {
    pub strategy_id: Uuid,
    /// 戦略名の完全一致が必須。cascade 削除の対象を取り違えないための確認用。
    pub confirm_name: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteStrategyResult {
    pub ok: bool,
    pub errors: Vec<String>,
}

/// `crate::models::TriggerKind` は `schemars::JsonSchema` を derive していないため、
/// MCP tool の入力スキーマ用に別途定義する。
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TriggerKindParam {
    Cron,
    Hook,
}

impl From<TriggerKindParam> for TriggerKind {
    fn from(value: TriggerKindParam) -> Self {
        match value {
            TriggerKindParam::Cron => TriggerKind::Cron,
            TriggerKindParam::Hook => TriggerKind::Hook,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateStrategyTriggerParams {
    pub strategy_id: Uuid,
    pub kind: TriggerKindParam,
    /// kind=cron 時に必須 (UTC の 5 フィールド cron 式)
    #[serde(default)]
    pub schedule: Option<String>,
    /// kind=hook 時に必須 (`/api/hooks/:hook_slug` のパス識別子)
    #[serde(default)]
    pub hook_slug: Option<String>,
    #[serde(default)]
    pub event_match: Option<serde_json::Value>,
    pub prompt_template: String,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateStrategyTriggerResult {
    pub ok: bool,
    pub errors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateStrategyTriggerParams {
    pub trigger_id: Uuid,
    #[serde(default)]
    pub schedule: Option<String>,
    #[serde(default)]
    pub hook_slug: Option<String>,
    #[serde(default)]
    pub event_match: Option<serde_json::Value>,
    #[serde(default)]
    pub prompt_template: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateStrategyTriggerResult {
    pub ok: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteStrategyTriggerParams {
    pub trigger_id: Uuid,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteStrategyTriggerResult {
    pub ok: bool,
    pub errors: Vec<String>,
}
