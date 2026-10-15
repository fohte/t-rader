use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use serde_json::Value as Json;
use utoipa::ToSchema;
use uuid::Uuid;

use core_application::note::PendingNoteVersion;
use core_application::note::frontmatter_tags;

use crate::services::graph::GraphDef;

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[schema(as = NoteVersion)]
pub struct NoteVersionResponse {
    pub id: Uuid,
    pub note_id: Uuid,
    pub version_no: i32,
    pub title: String,
    pub body_md: String,
    pub frontmatter_json: Json,
    /// 本文中の価格・市場データ参照リンクを key、解決した値を value とする。
    pub resolved_price_references_json: Json,
    #[schema(value_type = Vec<GraphDef>)]
    pub graphs_json: serde_json::Value,
    pub status: String,
    pub is_current: bool,
    pub change_reason: Option<String>,
    pub created_by_kind: String,
    pub execution_id: Option<String>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]
    pub reviewed_at: Option<DateTime<FixedOffset>>,
}

impl From<core_application::note::NoteVersion> for NoteVersionResponse {
    fn from(version: core_application::note::NoteVersion) -> Self {
        Self {
            id: version.id,
            note_id: version.note_id,
            version_no: version.version_no,
            title: version.title,
            body_md: version.body_md,
            frontmatter_json: version.frontmatter_json,
            resolved_price_references_json: version.resolved_price_references_json,
            graphs_json: version.graphs_json,
            status: version.status,
            is_current: version.is_current,
            change_reason: version.change_reason,
            created_by_kind: version.created_by_kind,
            execution_id: version.execution_id,
            created_at: version.created_at,
            reviewed_at: version.reviewed_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[schema(as = PendingNoteVersion)]
pub struct PendingNoteVersionResponse {
    #[serde(flatten)]
    pub version: NoteVersionResponse,
    pub kind: Option<String>,
    pub tags: Vec<String>,
}

impl From<PendingNoteVersion> for PendingNoteVersionResponse {
    fn from(pending: PendingNoteVersion) -> Self {
        let tags = frontmatter_tags(&pending.version.frontmatter_json);
        Self {
            version: pending.version.into(),
            kind: pending.kind,
            tags,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset};
    use core_application::note::{NoteVersion, PendingNoteVersion};
    use serde_json::json;
    use uuid::Uuid;

    use super::PendingNoteVersionResponse;

    #[test]
    fn pending_version_response_includes_kind_and_frontmatter_tags() {
        let created_at = timestamp();
        let actual = PendingNoteVersionResponse::from(PendingNoteVersion {
            version: NoteVersion {
                id: Uuid::from_u128(1),
                note_id: Uuid::from_u128(2),
                version_no: 3,
                title: "Sample title".to_owned(),
                body_md: "Sample body".to_owned(),
                frontmatter_json: json!({ "tags": ["sample-tag", "another-tag"] }),
                graphs_json: json!([]),
                resolved_price_references_json: json!({}),
                status: "unread".to_owned(),
                is_current: false,
                change_reason: Some("Sample reason".to_owned()),
                created_by_kind: "llm".to_owned(),
                execution_id: None,
                created_at,
                reviewed_at: None,
            },
            kind: Some("sample-kind".to_owned()),
        });

        assert_eq!(
            serde_json::to_value(actual).expect("response serializes"),
            json!({
                "id": Uuid::from_u128(1),
                "note_id": Uuid::from_u128(2),
                "version_no": 3,
                "title": "Sample title",
                "body_md": "Sample body",
                "frontmatter_json": { "tags": ["sample-tag", "another-tag"] },
                "resolved_price_references_json": {},
                "graphs_json": [],
                "status": "unread",
                "is_current": false,
                "change_reason": "Sample reason",
                "created_by_kind": "llm",
                "execution_id": null,
                "created_at": created_at,
                "reviewed_at": null,
                "kind": "sample-kind",
                "tags": ["sample-tag", "another-tag"],
            }),
        );
    }

    fn timestamp() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-10-13T00:00:00+00:00").expect("valid test timestamp")
    }
}
