use serde::Deserialize;
use utoipa::ToSchema;

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
    pub schedule: Option<String>,
    pub hook_slug: Option<String>,
    pub event_match: Option<serde_json::Value>,
    #[schema(min_length = 1, pattern = r"\S")]
    pub prompt_template: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
pub struct ListTriggersQuery {
    pub kind: Option<TriggerKind>,
}
