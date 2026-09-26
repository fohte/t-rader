use sea_orm::entity::prelude::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::entities::{note, note_version};
use crate::services::graph::GraphDef;

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

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = Note)]
pub struct NoteResponse {
    pub id: Uuid,
    pub version_id: Uuid,
    pub version_no: i32,
    pub is_current: bool,
    pub strategy_id: Option<Uuid>,
    pub title: String,
    pub body_md: String,
    pub frontmatter_json: Json,
    pub kind: Option<String>,
    pub status: String,
    pub trigger: Option<String>,
    pub trigger_label: Option<String>,
    pub created_by_kind: String,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
    #[schema(value_type = Vec<GraphDef>)]
    pub graphs_json: serde_json::Value,
    pub execution_id: Option<String>,
}

impl NoteResponse {
    pub fn from_version(
        note: note::Model,
        version: note_version::Model,
        created_by_kind: String,
    ) -> Self {
        Self {
            id: note.id,
            version_id: version.id,
            version_no: version.version_no,
            is_current: version.is_current,
            strategy_id: note.strategy_id,
            title: version.title,
            body_md: version.body_md,
            frontmatter_json: version.frontmatter_json,
            kind: note.kind,
            status: version.status,
            trigger: note.trigger,
            trigger_label: note.trigger_label,
            created_by_kind,
            created_at: note.created_at,
            updated_at: note.updated_at,
            graphs_json: version.graphs_json,
            execution_id: note.execution_id,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateNoteRequest {
    /// 任意。省略した場合、どの戦略にも属さないノートになる (市況・セクター横断の分析など)
    pub strategy_id: Option<Uuid>,
    #[schema(min_length = 1, pattern = r"\S")]
    pub title: String,
    /// `[[note:<uuid>]]` はリンク元バージョンを作成した時点の現行バージョンに固定する。
    /// `@current` を付けると以降の現行バージョンに追従する。
    pub body_md: String,
    #[serde(default)]
    #[schema(value_type = Option<std::collections::HashMap<String, serde_json::Value>>)]
    pub frontmatter_json: Option<serde_json::Value>,
    pub kind: Option<String>,
    /// 人間の作成では省略時に "approved"。指定する場合も "approved" のみ許可する。
    /// エージェントの作成では省略時に "unread"。承認必須種別では "unread" のみ許可する。
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
    /// `[[note:<uuid>]]` はリンク元バージョンを作成した時点の現行バージョンに固定する。
    /// `@current` を付けると以降の現行バージョンに追従する。
    pub body_md: Option<String>,
    #[schema(value_type = Option<std::collections::HashMap<String, serde_json::Value>>)]
    pub frontmatter_json: Option<serde_json::Value>,
    #[serde(
        default,
        deserialize_with = "crate::serde_helpers::deserialize_nullable_option"
    )]
    #[schema(value_type = Option<String>)]
    pub kind: Option<Option<String>>,
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

    /// Display は DB に書く文字列、serde は API 受け渡しの文字列で、
    /// 両者がずれると CHECK 制約違反や FE/BE 不一致が起きる。pin する
    #[rstest]
    #[case::hook(NoteTrigger::Hook)]
    #[case::cron(NoteTrigger::Cron)]
    #[case::on_demand(NoteTrigger::OnDemand)]
    #[case::manual(NoteTrigger::Manual)]
    fn test_note_trigger_display_matches_serde(#[case] trigger: NoteTrigger) {
        assert_eq!(
            serde_json::to_string(&trigger).unwrap(),
            format!("\"{trigger}\""),
        );
    }
}
