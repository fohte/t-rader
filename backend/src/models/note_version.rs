use chrono::{DateTime, FixedOffset};
use sea_orm::entity::prelude::Json;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::services::graph::GraphDef;
use gateway_postgres::entities::note_version;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = NoteVersion)]
pub struct NoteVersionResponse {
    pub id: Uuid,
    pub note_id: Uuid,
    pub version_no: i32,
    pub title: String,
    pub body_md: String,
    pub frontmatter_json: Json,
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

impl From<note_version::Model> for NoteVersionResponse {
    fn from(version: note_version::Model) -> Self {
        Self {
            id: version.id,
            note_id: version.note_id,
            version_no: version.version_no,
            title: version.title,
            body_md: version.body_md,
            frontmatter_json: version.frontmatter_json,
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

impl From<core_application::note::NoteVersion> for NoteVersionResponse {
    fn from(version: core_application::note::NoteVersion) -> Self {
        Self {
            id: version.id,
            note_id: version.note_id,
            version_no: version.version_no,
            title: version.title,
            body_md: version.body_md,
            frontmatter_json: version.frontmatter_json,
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
