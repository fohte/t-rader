use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

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
