use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde_json::{Map, Value};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct SubagentDefinition {
    pub id: Uuid,
    pub key: String,
    pub description: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub prompt: String,
    pub tools: Vec<String>,
    pub timeout_minutes: i32,
    pub max_model_calls: i64,
    pub tool_limits: BTreeMap<String, i64>,
    pub output: Map<String, Value>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubagentDefinitionFields {
    pub description: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub prompt: String,
    pub tools: Vec<String>,
    pub timeout_minutes: i32,
    pub max_model_calls: i64,
    pub tool_limits: BTreeMap<String, i64>,
    pub output: Map<String, Value>,
}
