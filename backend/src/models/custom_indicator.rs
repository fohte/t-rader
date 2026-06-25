use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateCustomIndicatorRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: String,
    pub code: String,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub input_schema: serde_json::Value,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub output_schema: serde_json::Value,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateCustomIndicatorRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: Option<String>,
    pub code: Option<String>,
    #[schema(value_type = Option<std::collections::HashMap<String, serde_json::Value>>)]
    pub input_schema: Option<serde_json::Value>,
    #[schema(value_type = Option<std::collections::HashMap<String, serde_json::Value>>)]
    pub output_schema: Option<serde_json::Value>,
    pub description: Option<String>,
}
