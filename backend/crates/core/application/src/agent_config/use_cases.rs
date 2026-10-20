use std::collections::BTreeMap;

use chrono::Utc;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::change_history::{Actor, ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind};
use crate::persistence::PersistenceError;
use crate::slug::is_valid_slug;
use crate::subagent_definition::SharedSubagentDefinitionRepository;
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};

use super::error::{AgentConfigRepositoryError, AgentConfigUseCaseError};
use super::graph::parse_agent_graph;
use super::repository::SharedAgentConfigRepository;
use super::types::{AgentConfig, NewAgentConfig, skills_as_btree};

#[derive(Clone)]
pub struct AgentConfigUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedAgentConfigRepository,
    subagent_definition_repository: SharedSubagentDefinitionRepository,
    change_history: SharedChangeHistoryPort,
}

impl AgentConfigUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedAgentConfigRepository,
        subagent_definition_repository: SharedSubagentDefinitionRepository,
        change_history: SharedChangeHistoryPort,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            subagent_definition_repository,
            change_history,
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
        self.record_snapshot(
            &transaction,
            agent_config.id,
            Op::Create,
            None,
            Some(&agent_config),
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        tracing::info!(purpose = %agent_config.purpose, "created agent_config");
        Ok(agent_config)
    }

    pub async fn delete(&self, purpose: &str) -> Result<(), AgentConfigUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .find_for_update_in_transaction(&transaction, purpose)
            .await?;
        if !self.repository.delete(&transaction, purpose).await? {
            return Err(AgentConfigUseCaseError::NotFound(purpose.to_string()));
        }
        self.record_snapshot(&transaction, current.id, Op::Delete, Some(&current), None)
            .await?;
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
        let agent_config = self
            .find_for_update_in_transaction(&transaction, purpose)
            .await?;
        let from_len = agent_config.agents_md.len();
        let saved = self
            .repository
            .update_agents_md(
                &transaction,
                agent_config.id,
                content,
                Utc::now().fixed_offset(),
            )
            .await
            .map_err(|error| map_update_error(error, purpose))?;
        self.record_snapshot(
            &transaction,
            saved.id,
            Op::Update,
            Some(&agent_config),
            Some(&saved),
        )
        .await?;
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
        let current = self
            .find_for_update_in_transaction(&transaction, purpose)
            .await?;
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
        let current = self
            .find_for_update_in_transaction(&transaction, purpose)
            .await?;
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
        let current = self
            .find_for_update_in_transaction(&transaction, purpose)
            .await?;
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
        let graph = parse_agent_graph(content)?;
        let transaction = self.unit_of_work.begin().await?;
        self.subagent_definition_repository
            .acquire_agent_graph_reference_lock(&transaction)
            .await?;

        if let Some(graph) = graph {
            let keys: std::collections::BTreeSet<_> = self
                .subagent_definition_repository
                .list_in_transaction(&transaction)
                .await?
                .into_iter()
                .map(|definition| definition.key)
                .collect();
            if let Some(key) = graph
                .phases
                .iter()
                .flat_map(|phase| &phase.subagents)
                .find(|key| !keys.contains(key.as_str()))
            {
                return Err(AgentConfigUseCaseError::UnknownSubagentDefinition(
                    key.clone(),
                ));
            }
        }

        let agent_config = self
            .find_for_update_in_transaction(&transaction, purpose)
            .await?;
        let from_len = agent_config.agent_graph.len();
        let saved = self
            .repository
            .update_agent_graph(
                &transaction,
                agent_config.id,
                content.to_string(),
                Utc::now().fixed_offset(),
            )
            .await
            .map_err(|error| map_update_error(error, purpose))?;
        self.record_snapshot(
            &transaction,
            saved.id,
            Op::Update,
            Some(&agent_config),
            Some(&saved),
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        tracing::info!(
            purpose,
            from_len,
            to_len = saved.agent_graph.len(),
            "updated agent_config agent_graph"
        );
        Ok(saved.agent_graph)
    }

    async fn find_for_update_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        purpose: &str,
    ) -> Result<AgentConfig, AgentConfigUseCaseError> {
        self.repository
            .find_by_purpose_for_update_in_transaction(transaction, purpose)
            .await?
            .ok_or_else(|| AgentConfigUseCaseError::NotFound(purpose.to_string()))
    }

    async fn save_skills(
        &self,
        transaction: UnitOfWorkTransaction,
        current: AgentConfig,
        skills: Value,
    ) -> Result<AgentConfig, AgentConfigUseCaseError> {
        let purpose = current.purpose.clone();
        let from: Vec<String> = current.skills_as_btree().into_keys().collect();
        let to: Vec<String> = skills_as_btree(&skills).into_keys().collect();
        let updated_at = Utc::now().fixed_offset();
        let saved = self
            .repository
            .update_skills(&transaction, current.id, skills, updated_at)
            .await
            .map_err(|error| map_update_error(error, &purpose))?;
        self.record_snapshot(
            &transaction,
            saved.id,
            Op::Update,
            Some(&current),
            Some(&saved),
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        tracing::info!(purpose, ?from, ?to, "updated agent_config skills");
        Ok(saved)
    }

    async fn record_snapshot(
        &self,
        transaction: &UnitOfWorkTransaction,
        target_id: Uuid,
        op: Op,
        before: Option<&AgentConfig>,
        after: Option<&AgentConfig>,
    ) -> Result<(), AgentConfigUseCaseError> {
        self.change_history
            .record(
                transaction,
                ChangeHistoryRecord {
                    actor: Actor::Human,
                    target_kind: TargetKind::AgentConfig,
                    target_id,
                    op,
                    diff: json!({
                        "format": "definition_snapshot",
                        "version": 1,
                        "before": before.map(agent_config_snapshot),
                        "after": after.map(agent_config_snapshot),
                    }),
                    summary: None,
                },
            )
            .await?;
        Ok(())
    }
}

fn agent_config_snapshot(agent_config: &AgentConfig) -> Value {
    json!({
        "id": agent_config.id,
        "purpose": agent_config.purpose,
        "agents_md": agent_config.agents_md,
        "skills": agent_config.skills,
        "agent_graph": agent_config.agent_graph,
    })
}

fn map_update_error(error: AgentConfigRepositoryError, purpose: &str) -> AgentConfigUseCaseError {
    match error {
        AgentConfigRepositoryError::Database(PersistenceError::RecordNotUpdated(_)) => {
            AgentConfigUseCaseError::NotFound(purpose.to_string())
        }
        error => error.into(),
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

#[cfg(all(test, feature = "test-support"))]
#[path = "tests.rs"]
mod tests;
