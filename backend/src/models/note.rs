use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

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
    pub trigger: Option<String>,
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
    pub trigger: Option<String>,
    pub trigger_label: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeStatusRequest {
    /// 任意のラベル (例: 却下理由)
    pub label: Option<String>,
}
