use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use core_application::subagent_definition::{SubagentDefinition, SubagentDefinitionFields};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateSubagentDefinitionRequest {
    pub key: String,
    pub description: String,
    pub model: String,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    pub prompt: String,
    pub tools: Vec<String>,
    pub timeout_minutes: i32,
    pub max_model_calls: i64,
    pub tool_limits: BTreeMap<String, i64>,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub output: BTreeMap<String, Value>,
}

impl CreateSubagentDefinitionRequest {
    pub fn into_fields(self) -> (String, SubagentDefinitionFields) {
        (
            self.key,
            SubagentDefinitionFields {
                description: self.description,
                model: self.model,
                reasoning_effort: self.reasoning_effort,
                prompt: self.prompt,
                tools: self.tools,
                timeout_minutes: self.timeout_minutes,
                max_model_calls: self.max_model_calls,
                tool_limits: self.tool_limits,
                output: self.output.into_iter().collect(),
            },
        )
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateSubagentDefinitionRequest {
    pub description: String,
    pub model: String,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    pub prompt: String,
    pub tools: Vec<String>,
    pub timeout_minutes: i32,
    pub max_model_calls: i64,
    pub tool_limits: BTreeMap<String, i64>,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub output: BTreeMap<String, Value>,
}

impl From<UpdateSubagentDefinitionRequest> for SubagentDefinitionFields {
    fn from(request: UpdateSubagentDefinitionRequest) -> Self {
        Self {
            description: request.description,
            model: request.model,
            reasoning_effort: request.reasoning_effort,
            prompt: request.prompt,
            tools: request.tools,
            timeout_minutes: request.timeout_minutes,
            max_model_calls: request.max_model_calls,
            tool_limits: request.tool_limits,
            output: request.output.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SubagentDefinitionResponse {
    pub id: uuid::Uuid,
    pub key: String,
    pub description: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub prompt: String,
    pub tools: Vec<String>,
    pub timeout_minutes: i32,
    pub max_model_calls: i64,
    pub tool_limits: BTreeMap<String, i64>,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub output: BTreeMap<String, Value>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

impl From<SubagentDefinition> for SubagentDefinitionResponse {
    fn from(definition: SubagentDefinition) -> Self {
        Self {
            id: definition.id,
            key: definition.key,
            description: definition.description,
            model: definition.model,
            reasoning_effort: definition.reasoning_effort,
            prompt: definition.prompt,
            tools: definition.tools,
            timeout_minutes: definition.timeout_minutes,
            max_model_calls: definition.max_model_calls,
            tool_limits: definition.tool_limits,
            output: definition.output.into_iter().collect(),
            created_at: definition.created_at,
            updated_at: definition.updated_at,
        }
    }
}
