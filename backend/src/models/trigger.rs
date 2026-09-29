use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

use gateway_postgres::entities::trigger;

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Trigger)]
pub struct TriggerResponse {
    pub trigger_id: Uuid,
    pub strategy_id: Option<Uuid>,
    pub purpose: Option<String>,
    pub kind: String,
    pub schedule: Option<String>,
    pub hook_slug: Option<String>,
    #[schema(value_type = Option<HashMap<String, serde_json::Value>>)]
    pub event_match: Option<HashMap<String, serde_json::Value>>,
    pub prompt_template: String,
    pub enabled: bool,
    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]
    pub last_fired_at: Option<DateTime<FixedOffset>>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub updated_at: DateTime<FixedOffset>,
}

impl From<trigger::Model> for TriggerResponse {
    fn from(model: trigger::Model) -> Self {
        let event_match = model
            .event_match
            .and_then(|value| value.as_object().cloned())
            .map(|object| object.into_iter().collect());

        Self {
            trigger_id: model.trigger_id,
            strategy_id: model.strategy_id,
            purpose: model.purpose,
            kind: model.kind,
            schedule: model.schedule,
            hook_slug: model.hook_slug,
            event_match,
            prompt_template: model.prompt_template,
            enabled: model.enabled,
            last_fired_at: model.last_fired_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
pub enum TriggerKind {
    Cron,
    Hook,
}

impl TriggerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            TriggerKind::Cron => "cron",
            TriggerKind::Hook => "hook",
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateTriggerRequest {
    #[serde(default)]
    pub purpose: Option<String>,
    pub kind: TriggerKind,
    /// kind=cron 時に必須 (UTC の 5 フィールド cron 式)
    pub schedule: Option<String>,
    /// kind=hook 時に必須 (`/api/hooks/:hook_slug` のパス識別子)
    pub hook_slug: Option<String>,
    pub event_match: Option<serde_json::Value>,
    #[schema(min_length = 1, pattern = r"\S")]
    pub prompt_template: String,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateTriggerRequest {
    /// 省略時は変更せず、`null` 指定時は default purpose に戻す。
    #[serde(
        default,
        deserialize_with = "crate::serde_helpers::deserialize_nullable_option"
    )]
    #[schema(value_type = Option<String>)]
    pub purpose: Option<Option<String>>,
    pub schedule: Option<String>,
    pub hook_slug: Option<String>,
    /// 省略時は変更せず、`null` 指定時は条件を解除する。
    #[serde(
        default,
        deserialize_with = "crate::serde_helpers::deserialize_nullable_option"
    )]
    #[schema(value_type = Option<serde_json::Value>)]
    pub event_match: Option<Option<serde_json::Value>>,
    #[schema(min_length = 1, pattern = r"\S")]
    pub prompt_template: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
pub struct ListTriggersQuery {
    pub kind: Option<TriggerKind>,
}
