mod payload_read;
mod payload_write;

pub use payload_write::DatasetCaptureOptions;

use std::{collections::BTreeMap, sync::Arc};

use chrono::Utc;
use core::fmt::Write as _;
use serde_json::{Map, Value as JsonValue, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    agent_config::AgentConfigUseCases,
    custom_indicator::{CustomIndicator, CustomIndicatorUseCases},
    custom_indicator_evaluation::{
        CustomIndicatorEvaluationUseCases, RecordCustomIndicatorEvaluationCommand,
    },
    strategy_scope::StrategyScope,
    strategy_task::{StrategyTaskPhase, StrategyTaskRepository},
    subagent_definition::SubagentDefinitionUseCases,
    unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction},
};

use self::payload_read::assemble_payload;
use super::{
    DefinitionSnapshot, MAX_PAYLOAD_PAGE_ITEMS, PayloadKind, PayloadStatus,
    StrategyExecutionRepository, StrategyExecutionUseCaseError, TaskStartSnapshotter,
};

#[derive(Clone)]
pub struct StrategyExecutionUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: Arc<dyn StrategyExecutionRepository + Send + Sync>,
    task_repository: Arc<dyn StrategyTaskRepository + Send + Sync>,
    custom_indicators: CustomIndicatorUseCases,
    custom_indicator_evaluations: CustomIndicatorEvaluationUseCases,
    subagent_definitions: SubagentDefinitionUseCases,
    agent_configs: AgentConfigUseCases,
}

impl StrategyExecutionUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: Arc<dyn StrategyExecutionRepository + Send + Sync>,
        task_repository: Arc<dyn StrategyTaskRepository + Send + Sync>,
        custom_indicators: CustomIndicatorUseCases,
        custom_indicator_evaluations: CustomIndicatorEvaluationUseCases,
        subagent_definitions: SubagentDefinitionUseCases,
        agent_configs: AgentConfigUseCases,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            task_repository,
            custom_indicators,
            custom_indicator_evaluations,
            subagent_definitions,
            agent_configs,
        }
    }

    pub async fn authorize_execution(
        &self,
        scope: StrategyScope,
        a2a_task_id: &str,
        execution_step_id: Option<Uuid>,
    ) -> Result<super::AuthorizedExecution, StrategyExecutionUseCaseError> {
        let task = self
            .task_repository
            .find_by_a2a_task_id(a2a_task_id)
            .await?
            .ok_or(StrategyExecutionUseCaseError::TaskNotFound)?;
        if task.strategy_id != scope.id() {
            return Err(StrategyExecutionUseCaseError::StrategyMismatch);
        }
        if task.phase != StrategyTaskPhase::Running {
            return Err(StrategyExecutionUseCaseError::TaskNotRunning);
        }
        Ok(super::AuthorizedExecution {
            task_id: task.task_id,
            strategy_id: task.strategy_id,
            a2a_task_id: a2a_task_id.to_string(),
            execution_step_id,
        })
    }

    pub async fn read_task_start_snapshot(
        &self,
        access: &super::AuthorizedExecution,
    ) -> Result<Option<DefinitionSnapshot>, StrategyExecutionUseCaseError> {
        let snapshot = self
            .repository
            .find_definition_snapshot(access.task_id, "task_start")
            .await?;
        if let Some(snapshot) = &snapshot {
            verify_definition_digest(snapshot)?;
        }
        Ok(snapshot)
    }

    pub async fn resolve_custom_indicator(
        &self,
        scope: StrategyScope,
        access: &super::AuthorizedExecution,
        name: &str,
    ) -> Result<super::ResolvedCustomIndicator, StrategyExecutionUseCaseError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StrategyExecutionUseCaseError::InvalidDefinitionSnapshot);
        }
        let definition_key = format!("custom_indicator:{name}");
        if let Some(snapshot) = self
            .repository
            .find_definition_snapshot(access.task_id, &definition_key)
            .await?
        {
            verify_definition_digest(&snapshot)?;
            return Ok(super::ResolvedCustomIndicator {
                definition: serde_json::from_value(snapshot.definition)
                    .map_err(|_| StrategyExecutionUseCaseError::InvalidDefinitionSnapshot)?,
                digest: snapshot.digest,
            });
        }

        let live = self
            .custom_indicators
            .resolve(scope, name)
            .await?
            .ok_or(StrategyExecutionUseCaseError::CustomIndicatorNotFound)?;
        let definition = serde_json::to_value(&live)
            .map_err(|_| StrategyExecutionUseCaseError::Serialization)?;
        let snapshot = DefinitionSnapshot {
            task_id: access.task_id,
            definition_key,
            digest: digest_json(&definition)?,
            definition,
            created_at: Utc::now().fixed_offset(),
        };
        let transaction = self.unit_of_work.begin().await?;
        let persisted = self
            .repository
            .insert_definition_snapshot(&transaction, snapshot)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        verify_definition_digest(&persisted)?;
        Ok(super::ResolvedCustomIndicator {
            definition: serde_json::from_value(persisted.definition)
                .map_err(|_| StrategyExecutionUseCaseError::InvalidDefinitionSnapshot)?,
            digest: persisted.digest,
        })
    }
}

impl StrategyExecutionUseCases {
    pub async fn record_numeric_projection(
        &self,
        access: &super::AuthorizedExecution,
        payload_id: Uuid,
        indicator: &CustomIndicator,
        args: JsonValue,
        pointers: BTreeMap<String, String>,
    ) -> Result<Uuid, StrategyExecutionUseCaseError> {
        if pointers.is_empty() || pointers.len() > MAX_PAYLOAD_PAGE_ITEMS {
            return Err(StrategyExecutionUseCaseError::InvalidNumericProjection);
        }
        let manifest = self
            .repository
            .find_payload(access.task_id, payload_id)
            .await?
            .ok_or(StrategyExecutionUseCaseError::PayloadNotFound)?;
        if manifest.status != PayloadStatus::Sealed || manifest.kind != PayloadKind::Output {
            return Err(StrategyExecutionUseCaseError::PayloadNotSealed);
        }
        let chunks = self
            .repository
            .list_payload_chunks(access.task_id, payload_id)
            .await?;
        let root = assemble_payload(&manifest, &chunks)?;
        let result = root
            .pointer("/value")
            .ok_or(StrategyExecutionUseCaseError::InvalidNumericProjection)?;
        crate::json_schema::validate_instance(&indicator.output_schema, result)
            .map_err(|_| StrategyExecutionUseCaseError::InvalidNumericProjection)?;
        let mut output = Map::new();
        let mut pointer_map = Map::new();
        for (key, pointer) in pointers {
            if key.trim().is_empty() || (pointer != "/value" && !pointer.starts_with("/value/")) {
                return Err(StrategyExecutionUseCaseError::InvalidNumericProjection);
            }
            let result_pointer = pointer
                .strip_prefix("/value")
                .ok_or(StrategyExecutionUseCaseError::InvalidNumericProjection)?;
            let value = result
                .pointer(result_pointer)
                .filter(|value| value.is_number())
                .ok_or(StrategyExecutionUseCaseError::InvalidNumericProjection)?;
            output.insert(key.clone(), value.clone());
            pointer_map.insert(key, JsonValue::String(pointer));
        }
        let projection_args = json!({
            "payload_id": payload_id,
            "pointers": pointer_map.clone(),
            "args": args,
        });
        self.custom_indicator_evaluations
            .record(RecordCustomIndicatorEvaluationCommand {
                indicator_id: indicator.indicator_id,
                indicator_name: indicator.name.clone(),
                code: indicator.code.clone(),
                args: projection_args,
                output: JsonValue::Object(output),
                payload_id: Some(payload_id),
                payload_pointers: Some(JsonValue::Object(pointer_map)),
                execution_step_id: access.execution_step_id,
            })
            .await
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl TaskStartSnapshotter for StrategyExecutionUseCases {
    async fn capture_task_start(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        purpose: &str,
    ) -> Result<(), StrategyExecutionUseCaseError> {
        let agent_config = self.agent_configs.get(purpose).await?;
        let mut definitions = self.subagent_definitions.list().await?;
        definitions.sort_by(|left, right| left.key.cmp(&right.key));
        let subagents = definitions
            .into_iter()
            .map(|definition| {
                json!({
                    "id": definition.id,
                    "key": definition.key,
                    "description": definition.description,
                    "model": definition.model,
                    "reasoning_effort": definition.reasoning_effort,
                    "prompt": definition.prompt,
                    "tools": definition.tools,
                    "timeout_minutes": definition.timeout_minutes,
                    "max_model_calls": definition.max_model_calls,
                    "tool_limits": definition.tool_limits,
                    "output": definition.output,
                    "created_at": definition.created_at,
                    "updated_at": definition.updated_at,
                })
            })
            .collect::<Vec<_>>();
        let definition = json!({
            "agent_config": {
                "id": agent_config.id,
                "purpose": agent_config.purpose,
                "agents_md": agent_config.agents_md,
                "skills": agent_config.skills,
                "agent_graph": agent_config.agent_graph,
                "created_at": agent_config.created_at,
                "updated_at": agent_config.updated_at,
            },
            "subagent_definitions": subagents,
        });
        self.repository
            .insert_definition_snapshot(
                transaction,
                DefinitionSnapshot {
                    task_id,
                    definition_key: "task_start".to_string(),
                    digest: digest_json(&definition)?,
                    definition,
                    created_at: Utc::now().fixed_offset(),
                },
            )
            .await?;
        Ok(())
    }
}

fn digest_json(value: &JsonValue) -> Result<String, StrategyExecutionUseCaseError> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| StrategyExecutionUseCaseError::Serialization)?;
    Ok(digest_bytes(&bytes))
}

fn verify_definition_digest(
    snapshot: &DefinitionSnapshot,
) -> Result<(), StrategyExecutionUseCaseError> {
    if digest_json(&snapshot.definition)? == snapshot.digest {
        Ok(())
    } else {
        Err(StrategyExecutionUseCaseError::InvalidDefinitionSnapshot)
    }
}

fn digest_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(output, "{byte:02x}");
    }
    output
}
