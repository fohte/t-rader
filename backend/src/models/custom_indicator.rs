use serde::{Deserialize, Deserializer};
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
    // 未指定 (= 変更しない) と null 指定 (= clear) を区別するため double Option を使う
    #[serde(default, deserialize_with = "deserialize_some")]
    #[schema(value_type = Option<String>)]
    pub description: Option<Option<String>>,
}

fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}
