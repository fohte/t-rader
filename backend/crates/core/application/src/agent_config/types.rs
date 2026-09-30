use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct AgentConfig {
    pub id: Uuid,
    pub purpose: String,
    pub agents_md: String,
    pub skills: Value,
    pub agent_graph: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewAgentConfig {
    pub id: Uuid,
    pub purpose: String,
}

impl AgentConfig {
    pub fn skills_as_btree(&self) -> BTreeMap<String, String> {
        skills_as_btree(&self.skills)
    }
}

pub(crate) fn skills_as_btree(value: &Value) -> BTreeMap<String, String> {
    let mut skills = BTreeMap::new();
    if let Some(object) = value.as_object() {
        for (name, content) in object {
            if let Some(content) = content.as_str() {
                skills.insert(name.clone(), content.to_string());
            }
        }
    }
    skills
}
