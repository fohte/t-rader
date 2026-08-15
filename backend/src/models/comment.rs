use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateCommentRequest {
    /// "note" | "annotation"
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
    /// コメント時点で選択された本文の該当箇所全文。target_kind が "note" の場合のみ
    /// note 更新後の位置追跡に使う (annotation では保存されるだけで追跡されない)。
    #[serde(default)]
    pub anchor_text: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateCommentRequest {
    pub resolved: bool,
}
