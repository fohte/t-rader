use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use uuid::Uuid;

pub const SCOPE_GLOBAL: &str = "global";
pub const SCOPE_STRATEGY: &str = "strategy";

#[derive(Debug, Clone, PartialEq)]
pub struct CustomIndicator {
    pub indicator_id: Uuid,
    pub name: String,
    pub scope: String,
    pub strategy_id: Option<Uuid>,
    pub code: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub description: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewCustomIndicator {
    pub indicator_id: Uuid,
    pub name: String,
    pub scope: String,
    pub strategy_id: Option<Uuid>,
    pub code: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateCustomIndicatorCommand {
    pub name: String,
    pub strategy_id: Option<Uuid>,
    pub code: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateCustomIndicatorCommand {
    pub name: Option<String>,
    pub code: Option<String>,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub description: Option<Option<String>>,
}
