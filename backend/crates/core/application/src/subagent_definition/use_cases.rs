use chrono::Utc;
use uuid::Uuid;

use crate::agent_config::{SharedAgentConfigRepository, graph::parse_agent_graph};
use crate::persistence::PersistenceError;
use crate::unit_of_work::SharedUnitOfWork;

use super::error::{SubagentDefinitionRepositoryError, SubagentDefinitionUseCaseError};
use super::repository::SharedSubagentDefinitionRepository;
use super::types::{SubagentDefinition, SubagentDefinitionFields};

#[derive(Clone)]
pub struct SubagentDefinitionUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedSubagentDefinitionRepository,
    agent_config_repository: SharedAgentConfigRepository,
}

impl SubagentDefinitionUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedSubagentDefinitionRepository,
        agent_config_repository: SharedAgentConfigRepository,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            agent_config_repository,
        }
    }

    pub async fn list(&self) -> Result<Vec<SubagentDefinition>, SubagentDefinitionUseCaseError> {
        self.repository.list().await.map_err(Into::into)
    }

    pub async fn get(
        &self,
        key: &str,
    ) -> Result<SubagentDefinition, SubagentDefinitionUseCaseError> {
        self.repository
            .find_by_key(key)
            .await?
            .ok_or_else(|| SubagentDefinitionUseCaseError::NotFound(key.to_string()))
    }

    pub async fn create(
        &self,
        key: String,
        fields: SubagentDefinitionFields,
    ) -> Result<SubagentDefinition, SubagentDefinitionUseCaseError> {
        validate(&key, &fields)?;

        let transaction = self.unit_of_work.begin().await?;
        let now = Utc::now().fixed_offset();
        let definition = self
            .repository
            .insert(
                &transaction,
                SubagentDefinition {
                    id: Uuid::new_v4(),
                    key: key.clone(),
                    description: fields.description,
                    model: fields.model,
                    reasoning_effort: fields.reasoning_effort,
                    prompt: fields.prompt,
                    tools: fields.tools,
                    timeout_minutes: fields.timeout_minutes,
                    max_model_calls: fields.max_model_calls,
                    tool_limits: fields.tool_limits,
                    output: fields.output,
                    created_at: now,
                    updated_at: now,
                },
            )
            .await
            .map_err(|error| match error {
                SubagentDefinitionRepositoryError::Database(PersistenceError::Conflict(_)) => {
                    SubagentDefinitionUseCaseError::DuplicateKey(key.clone())
                }
                error => error.into(),
            })?;
        self.unit_of_work.commit(transaction).await?;
        Ok(definition)
    }

    pub async fn update(
        &self,
        key: &str,
        fields: SubagentDefinitionFields,
    ) -> Result<SubagentDefinition, SubagentDefinitionUseCaseError> {
        validate(key, &fields)?;

        let transaction = self.unit_of_work.begin().await?;
        let mut definition = self
            .repository
            .find_by_key_in_transaction(&transaction, key)
            .await?
            .ok_or_else(|| SubagentDefinitionUseCaseError::NotFound(key.to_string()))?;
        definition.description = fields.description;
        definition.model = fields.model;
        definition.reasoning_effort = fields.reasoning_effort;
        definition.prompt = fields.prompt;
        definition.tools = fields.tools;
        definition.timeout_minutes = fields.timeout_minutes;
        definition.max_model_calls = fields.max_model_calls;
        definition.tool_limits = fields.tool_limits;
        definition.output = fields.output;
        definition.updated_at = Utc::now().fixed_offset();
        let saved = self
            .repository
            .update(&transaction, definition)
            .await
            .map_err(|error| map_update_error(error, key))?;
        self.unit_of_work.commit(transaction).await?;
        Ok(saved)
    }

    pub async fn delete(&self, key: &str) -> Result<(), SubagentDefinitionUseCaseError> {
        self.ensure_not_referenced(key).await?;
        let transaction = self.unit_of_work.begin().await?;
        if !self.repository.delete(&transaction, key).await? {
            return Err(SubagentDefinitionUseCaseError::NotFound(key.to_string()));
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    async fn ensure_not_referenced(&self, key: &str) -> Result<(), SubagentDefinitionUseCaseError> {
        for agent_config in self.agent_config_repository.list().await? {
            let Some(graph) = parse_agent_graph(&agent_config.agent_graph)? else {
                continue;
            };
            for phase in graph.phases {
                if phase.subagents.iter().any(|subagent| subagent == key) {
                    return Err(SubagentDefinitionUseCaseError::Referenced {
                        key: key.to_string(),
                        purpose: agent_config.purpose,
                        phase: phase.key,
                    });
                }
            }
        }
        Ok(())
    }
}

fn validate(
    key: &str,
    fields: &SubagentDefinitionFields,
) -> Result<(), SubagentDefinitionUseCaseError> {
    let mut chars = key.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {
            if !chars.all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || character == '_'
                    || character == '-'
            }) {
                return Err(SubagentDefinitionUseCaseError::InvalidKey(key.to_string()));
            }
        }
        _ => return Err(SubagentDefinitionUseCaseError::InvalidKey(key.to_string())),
    }
    if fields.tools.iter().any(|tool| tool == "run_subagent") {
        return Err(SubagentDefinitionUseCaseError::RunSubagentToolNotAllowed);
    }
    if !(1..=crate::agent_config::graph::MAX_TIMEOUT_MINUTES as i32)
        .contains(&fields.timeout_minutes)
    {
        return Err(SubagentDefinitionUseCaseError::TimeoutMinutesOutOfRange {
            value: fields.timeout_minutes,
        });
    }
    if fields.max_model_calls <= 0 {
        return Err(SubagentDefinitionUseCaseError::MaxModelCallsOutOfRange(
            fields.max_model_calls,
        ));
    }
    if let Some((tool, value)) = fields.tool_limits.iter().find(|(_, value)| **value < 0) {
        return Err(SubagentDefinitionUseCaseError::ToolLimitOutOfRange {
            tool: tool.clone(),
            value: *value,
        });
    }
    Ok(())
}

fn map_update_error(
    error: SubagentDefinitionRepositoryError,
    key: &str,
) -> SubagentDefinitionUseCaseError {
    match error {
        SubagentDefinitionRepositoryError::Database(PersistenceError::RecordNotUpdated(_)) => {
            SubagentDefinitionUseCaseError::NotFound(key.to_string())
        }
        error => error.into(),
    }
}
