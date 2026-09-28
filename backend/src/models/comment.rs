use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use gateway_postgres::entities::comment;

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Comment)]
pub struct CommentResponse {
    pub id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub body: String,
    pub author_kind: String,
    pub author_label: String,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
    pub resolved: bool,
    pub anchor_text: Option<String>,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
    pub anchor_side: Option<String>,
}

impl From<comment::Model> for CommentResponse {
    fn from(model: comment::Model) -> Self {
        Self {
            id: model.id,
            target_kind: model.target_kind,
            target_id: model.target_id,
            parent_id: model.parent_id,
            body: model.body,
            author_kind: model.author_kind,
            author_label: model.author_label,
            created_at: model.created_at,
            resolved: model.resolved,
            anchor_text: model.anchor_text,
            start_line: model.start_line,
            end_line: model.end_line,
            anchor_side: model.anchor_side,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CommentAnchorSide {
    Old,
    New,
}

impl CommentAnchorSide {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Old => "old",
            Self::New => "new",
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateCommentRequest {
    /// "note_version" | "annotation"
    pub target_kind: String,
    pub target_id: Uuid,
    pub parent_id: Option<Uuid>,
    #[schema(min_length = 1)]
    pub body: String,
    /// "human" | "llm"。デフォルトは "human"
    #[serde(default)]
    pub author_kind: Option<String>,
    #[serde(default)]
    pub author_label: Option<String>,
    /// 行コメントに添える引用文。空行では空文字になる場合がある。
    #[serde(default)]
    pub anchor_text: Option<String>,
    /// note_version のどちら側の本文に対する行コメントか。
    #[serde(default)]
    pub anchor_side: Option<CommentAnchorSide>,
    /// note_version 本文上の 1-indexed の開始行。
    #[serde(default)]
    #[schema(minimum = 1)]
    pub start_line: Option<i32>,
    /// note_version 本文上の 1-indexed の終了行。
    #[serde(default)]
    #[schema(minimum = 1)]
    pub end_line: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateCommentRequest {
    pub resolved: bool,
}
