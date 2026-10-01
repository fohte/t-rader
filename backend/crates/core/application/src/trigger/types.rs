use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    Cron,
    Hook,
}

impl TriggerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cron => "cron",
            Self::Hook => "hook",
        }
    }
}

impl TryFrom<&str> for TriggerKind {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "cron" => Ok(Self::Cron),
            "hook" => Ok(Self::Hook),
            _ => Err("trigger kind is invalid"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub trigger_id: Uuid,
    pub strategy_id: Option<Uuid>,
    pub purpose: Option<String>,
    pub kind: TriggerKind,
    pub schedule: Option<String>,
    pub hook_slug: Option<String>,
    pub event_match: Option<Value>,
    pub prompt_template: String,
    pub enabled: bool,
    pub last_fired_at: Option<DateTime<FixedOffset>>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewTrigger {
    pub trigger_id: Uuid,
    pub strategy_id: Uuid,
    pub purpose: Option<String>,
    pub kind: TriggerKind,
    pub schedule: Option<String>,
    pub hook_slug: Option<String>,
    pub event_match: Option<Value>,
    pub prompt_template: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTriggerCommand {
    pub purpose: Option<String>,
    pub kind: TriggerKind,
    pub schedule: Option<String>,
    pub hook_slug: Option<String>,
    pub event_match: Option<Value>,
    pub prompt_template: String,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpdateTriggerCommand {
    pub purpose: Option<Option<String>>,
    pub schedule: Option<String>,
    pub hook_slug: Option<String>,
    pub event_match: Option<Option<Value>>,
    pub prompt_template: Option<String>,
    pub enabled: Option<bool>,
}
