use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// ノートが生成された契機。DB の note_trigger_check CHECK 制約と一致させる
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NoteTrigger {
    Hook,
    Cron,
    OnDemand,
    Manual,
}

impl std::fmt::Display for NoteTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Hook => "hook",
            Self::Cron => "cron",
            Self::OnDemand => "on-demand",
            Self::Manual => "manual",
        })
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateNoteRequest {
    pub strategy_id: Uuid,
    #[schema(min_length = 1, pattern = r"\S")]
    pub title: String,
    pub body_md: String,
    #[serde(default)]
    #[schema(value_type = Option<std::collections::HashMap<String, serde_json::Value>>)]
    pub frontmatter_json: Option<serde_json::Value>,
    pub type_tag: Option<String>,
    /// 任意。デフォルトは "unread"
    pub status: Option<String>,
    pub trigger: Option<NoteTrigger>,
    pub trigger_label: Option<String>,
    /// 作成者種別 ("human" | "llm")。デフォルトは "human"
    #[serde(default)]
    pub created_by_kind: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateNoteRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub title: Option<String>,
    pub body_md: Option<String>,
    #[schema(value_type = Option<std::collections::HashMap<String, serde_json::Value>>)]
    pub frontmatter_json: Option<serde_json::Value>,
    pub type_tag: Option<String>,
    pub trigger: Option<NoteTrigger>,
    pub trigger_label: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeStatusRequest {
    /// 任意のラベル (例: 却下理由)
    pub label: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::hook("\"hook\"", NoteTrigger::Hook)]
    #[case::cron("\"cron\"", NoteTrigger::Cron)]
    #[case::on_demand("\"on-demand\"", NoteTrigger::OnDemand)]
    #[case::manual("\"manual\"", NoteTrigger::Manual)]
    fn test_note_trigger_deserialize_valid(#[case] input: &str, #[case] expected: NoteTrigger) {
        assert_eq!(
            serde_json::from_str::<NoteTrigger>(input).unwrap(),
            expected,
        );
    }

    #[rstest]
    #[case::empty("\"\"")]
    #[case::unknown("\"invalid\"")]
    #[case::snake_case("\"on_demand\"")]
    fn test_note_trigger_deserialize_invalid(#[case] input: &str) {
        assert_eq!(serde_json::from_str::<NoteTrigger>(input).ok(), None);
    }

    #[rstest]
    #[case::hook(NoteTrigger::Hook, "hook")]
    #[case::cron(NoteTrigger::Cron, "cron")]
    #[case::on_demand(NoteTrigger::OnDemand, "on-demand")]
    #[case::manual(NoteTrigger::Manual, "manual")]
    fn test_note_trigger_display(#[case] trigger: NoteTrigger, #[case] expected: &str) {
        assert_eq!(trigger.to_string(), expected);
    }
}
