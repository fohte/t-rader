use std::collections::BTreeMap;

use chrono::Utc;
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};

use super::error::{AgentConfigRepositoryError, AgentConfigUseCaseError};
use super::graph::parse_agent_graph;
use super::repository::SharedAgentConfigRepository;
use super::types::{AgentConfig, NewAgentConfig};

#[derive(Clone)]
pub struct AgentConfigUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedAgentConfigRepository,
}

impl AgentConfigUseCases {
    pub fn new(unit_of_work: SharedUnitOfWork, repository: SharedAgentConfigRepository) -> Self {
        Self {
            unit_of_work,
            repository,
        }
    }

    pub async fn list(&self) -> Result<Vec<AgentConfig>, AgentConfigUseCaseError> {
        self.repository.list().await.map_err(Into::into)
    }

    pub async fn get(&self, purpose: &str) -> Result<AgentConfig, AgentConfigUseCaseError> {
        self.repository
            .find_by_purpose(purpose)
            .await?
            .ok_or_else(|| AgentConfigUseCaseError::NotFound(purpose.to_string()))
    }

    pub async fn create(&self, purpose: String) -> Result<AgentConfig, AgentConfigUseCaseError> {
        validate_purpose(&purpose)?;

        let transaction = self.unit_of_work.begin().await?;
        let agent_config = self
            .repository
            .insert(
                &transaction,
                NewAgentConfig {
                    id: Uuid::new_v4(),
                    purpose: purpose.clone(),
                },
            )
            .await
            .map_err(|error| match error {
                AgentConfigRepositoryError::Database(PersistenceError::Conflict(_)) => {
                    AgentConfigUseCaseError::DuplicatePurpose(purpose.clone())
                }
                error => error.into(),
            })?;
        self.unit_of_work.commit(transaction).await?;
        tracing::info!(purpose = %agent_config.purpose, "created agent_config");
        Ok(agent_config)
    }

    pub async fn delete(&self, purpose: &str) -> Result<(), AgentConfigUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        if !self.repository.delete(&transaction, purpose).await? {
            return Err(AgentConfigUseCaseError::NotFound(purpose.to_string()));
        }
        self.unit_of_work.commit(transaction).await?;
        tracing::info!(purpose, "deleted agent_config");
        Ok(())
    }

    pub async fn save_agents_md(
        &self,
        purpose: &str,
        content: String,
    ) -> Result<String, AgentConfigUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let mut agent_config = self.find_in_transaction(&transaction, purpose).await?;
        let from_len = agent_config.agents_md.len();
        agent_config.agents_md = content;
        agent_config.updated_at = Utc::now().fixed_offset();
        let saved = self
            .repository
            .update(&transaction, agent_config)
            .await
            .map_err(|error| map_update_error(error, purpose))?;
        self.unit_of_work.commit(transaction).await?;
        tracing::info!(
            purpose,
            from_len,
            to_len = saved.agents_md.len(),
            "updated agent_config agents_md"
        );
        Ok(saved.agents_md)
    }

    pub async fn put_skills(
        &self,
        purpose: &str,
        skills: BTreeMap<String, String>,
    ) -> Result<AgentConfig, AgentConfigUseCaseError> {
        for name in skills.keys() {
            validate_skill_name(name)?;
        }
        let transaction = self.unit_of_work.begin().await?;
        let current = self.find_in_transaction(&transaction, purpose).await?;
        let map = skills
            .into_iter()
            .map(|(name, content)| (name, Value::String(content)))
            .collect();
        self.save_skills(transaction, current, Value::Object(map))
            .await
    }

    pub async fn put_skill(
        &self,
        purpose: &str,
        name: &str,
        content: String,
    ) -> Result<AgentConfig, AgentConfigUseCaseError> {
        validate_skill_name(name)?;
        let transaction = self.unit_of_work.begin().await?;
        let current = self.find_in_transaction(&transaction, purpose).await?;
        let mut patch = Map::new();
        patch.insert(name.to_string(), Value::String(content));
        let merged = apply_skills_patch(&current.skills, patch);
        self.save_skills(transaction, current, Value::Object(merged))
            .await
    }

    pub async fn delete_skill(
        &self,
        purpose: &str,
        name: &str,
    ) -> Result<AgentConfig, AgentConfigUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let current = self.find_in_transaction(&transaction, purpose).await?;
        let mut skills = skills_object(&current.skills);
        if skills.remove(name).is_none() {
            return Err(AgentConfigUseCaseError::SkillNotFound(name.to_string()));
        }
        self.save_skills(transaction, current, Value::Object(skills))
            .await
    }

    pub async fn save_agent_graph(
        &self,
        purpose: &str,
        content: &str,
    ) -> Result<String, AgentConfigUseCaseError> {
        parse_agent_graph(content)?;

        let transaction = self.unit_of_work.begin().await?;
        let mut agent_config = self.find_in_transaction(&transaction, purpose).await?;
        let from_len = agent_config.agent_graph.len();
        agent_config.agent_graph = content.to_string();
        agent_config.updated_at = Utc::now().fixed_offset();
        let saved = self
            .repository
            .update(&transaction, agent_config)
            .await
            .map_err(|error| map_update_error(error, purpose))?;
        self.unit_of_work.commit(transaction).await?;
        tracing::info!(
            purpose,
            from_len,
            to_len = saved.agent_graph.len(),
            "updated agent_config agent_graph"
        );
        Ok(saved.agent_graph)
    }

    async fn find_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        purpose: &str,
    ) -> Result<AgentConfig, AgentConfigUseCaseError> {
        self.repository
            .find_by_purpose_in_transaction(transaction, purpose)
            .await?
            .ok_or_else(|| AgentConfigUseCaseError::NotFound(purpose.to_string()))
    }

    async fn save_skills(
        &self,
        transaction: UnitOfWorkTransaction,
        mut current: AgentConfig,
        skills: Value,
    ) -> Result<AgentConfig, AgentConfigUseCaseError> {
        let purpose = current.purpose.clone();
        let from: Vec<String> = current.skills_as_btree().into_keys().collect();
        let to: Vec<String> = skills_to_btree(&skills).into_keys().collect();
        current.skills = skills;
        current.updated_at = Utc::now().fixed_offset();
        let saved = self
            .repository
            .update(&transaction, current)
            .await
            .map_err(|error| map_update_error(error, &purpose))?;
        self.unit_of_work.commit(transaction).await?;
        tracing::info!(purpose, ?from, ?to, "updated agent_config skills");
        Ok(saved)
    }
}

fn map_update_error(error: AgentConfigRepositoryError, purpose: &str) -> AgentConfigUseCaseError {
    match error {
        AgentConfigRepositoryError::Database(PersistenceError::RecordNotUpdated(_)) => {
            AgentConfigUseCaseError::NotFound(purpose.to_string())
        }
        error => error.into(),
    }
}

fn is_valid_slug(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {
            chars.all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || character == '_'
                    || character == '-'
            })
        }
        _ => false,
    }
}

fn validate_purpose(purpose: &str) -> Result<(), AgentConfigUseCaseError> {
    if !is_valid_slug(purpose) {
        return Err(AgentConfigUseCaseError::InvalidPurpose(purpose.to_string()));
    }
    Ok(())
}

fn validate_skill_name(name: &str) -> Result<(), AgentConfigUseCaseError> {
    if !is_valid_slug(name) {
        return Err(AgentConfigUseCaseError::InvalidSkillName(name.to_string()));
    }
    Ok(())
}

fn skills_object(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn skills_to_btree(value: &Value) -> BTreeMap<String, String> {
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

fn apply_skills_patch(current: &Value, patch: Map<String, Value>) -> Map<String, Value> {
    let mut skills = skills_object(current);
    for (name, content) in patch {
        if content.is_null() {
            skills.remove(&name);
        } else {
            skills.insert(name, content);
        }
    }
    skills
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
