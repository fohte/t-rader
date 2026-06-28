use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateHypothesisRequest {
    pub title: String,
    pub body: String,
    /// 省略時 `unverified`
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub related_note_ids: Option<Vec<Uuid>>,
    #[serde(default)]
    pub related_interest_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateHypothesisRequest {
    pub title: Option<String>,
    pub body: Option<String>,
    pub status: Option<String>,
    pub related_note_ids: Option<Vec<Uuid>>,
    pub related_interest_ids: Option<Vec<Uuid>>,
}
