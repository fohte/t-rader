use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateStrategyRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateStrategyRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: Option<String>,
    pub description: Option<String>,
    pub sort_order: Option<i32>,
}
