use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    agent_config::graph::{AgentGraphConfig, parse_agent_graph},
    strategy_execution::{
        AuthorizedExecution, DefinitionSnapshot, SharedStrategyExecutionRepository,
    },
    unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction},
};

use super::{
    BudgetReservationCommand, BudgetReservationResult, BudgetReservationState,
    BudgetReservationTransition, BudgetResource, BudgetScopeLimit, ExecutionBudgetRepository,
    ExecutionBudgetUseCaseError,
};

const DEFAULT_MODEL_CALLS_PER_STEP: u64 = 15;

#[cfg(test)]
mod reservation_tests;

#[derive(Clone)]
pub struct ExecutionBudgetUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: Arc<dyn ExecutionBudgetRepository + Send + Sync>,
    snapshots: SharedStrategyExecutionRepository,
}

impl ExecutionBudgetUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: Arc<dyn ExecutionBudgetRepository + Send + Sync>,
        snapshots: SharedStrategyExecutionRepository,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            snapshots,
        }
    }

    pub async fn initialize_task(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        deadline_at: DateTime<FixedOffset>,
    ) -> Result<(), ExecutionBudgetUseCaseError> {
        self.repository
            .initialize_task(transaction, task_id, deadline_at)
            .await?;
        Ok(())
    }

    pub async fn reserve_model_call(
        &self,
        access: &AuthorizedExecution,
        call_id: String,
        request_fingerprint: String,
        execution_step_id: &str,
        subagent_definition_key: Option<&str>,
        subagent_execution_id: Option<&str>,
    ) -> Result<BudgetReservationResult, ExecutionBudgetUseCaseError> {
        let execution_step_id = Uuid::parse_str(execution_step_id)
            .ok()
            .filter(|step_id| access.execution_step_id() == Some(*step_id))
            .ok_or(ExecutionBudgetUseCaseError::InvalidRequest)?;
        let snapshot = self.task_start_snapshot(access.task_id()).await?;
        let graph = task_graph(&snapshot.definition)?;
        let task_limit = graph
            .as_ref()
            .and_then(|graph| graph.execution_limits.as_ref())
            .and_then(|limits| limits.max_model_calls)
            .map(u64::from);
        let (resource_key, scope_key, local_limit) =
            match (subagent_definition_key, subagent_execution_id) {
                (Some(definition_key), Some(subagent_execution_id)) => {
                    let subagent_execution_id =
                        matching_execution_id(subagent_execution_id, execution_step_id)?;
                    let definition = find_subagent(&snapshot.definition, definition_key)?;
                    let model_limit = definition
                        .get("max_model_calls")
                        .and_then(Value::as_u64)
                        .ok_or(ExecutionBudgetUseCaseError::InvalidTaskStartSnapshot)?;
                    (
                        format!("subagent:{definition_key}"),
                        format!("subagent_run:{subagent_execution_id}"),
                        Some(model_limit),
                    )
                }
                (None, None) => (
                    "model_call".to_string(),
                    format!("step:{execution_step_id}"),
                    Some(DEFAULT_MODEL_CALLS_PER_STEP),
                ),
                _ => return Err(ExecutionBudgetUseCaseError::InvalidRequest),
            };
        let mut charges = vec![BudgetScopeLimit {
            resource_key,
            scope_key,
            amount: 1,
            limit: local_limit,
        }];
        if let Some(limit) = task_limit {
            charges.push(BudgetScopeLimit {
                resource_key: "__task_total".to_string(),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(limit),
            });
        }
        self.reserve(
            access.task_id(),
            call_id,
            request_fingerprint,
            BudgetResource::ModelCall,
            charges,
        )
        .await
    }

    pub async fn reserve_tool_call(
        &self,
        access: &AuthorizedExecution,
        call_id: String,
        request_fingerprint: String,
        tool_name: &str,
        default_task_limit: Option<u64>,
        subagent: Option<(&str, &str)>,
    ) -> Result<BudgetReservationResult, ExecutionBudgetUseCaseError> {
        let snapshot = self.task_start_snapshot(access.task_id()).await?;
        let graph = task_graph(&snapshot.definition)?;
        let configured_task_limit = graph
            .as_ref()
            .and_then(|graph| graph.tool_limits.get(tool_name))
            .map(|limit| u64::from(*limit));
        let execution_limits = graph
            .as_ref()
            .and_then(|graph| graph.execution_limits.as_ref());
        let subagent_identity = match subagent {
            Some((execution_id, definition_key)) => {
                let current_execution_id = access
                    .execution_step_id()
                    .ok_or(ExecutionBudgetUseCaseError::InvalidRequest)?;
                let execution_id = matching_execution_id(execution_id, current_execution_id)?;
                Some((execution_id, definition_key))
            }
            None => None,
        };
        let mut charges = vec![BudgetScopeLimit {
            resource_key: tool_name.to_string(),
            scope_key: "task".to_string(),
            amount: 1,
            limit: configured_task_limit,
        }];
        if let Some(limit) = default_task_limit {
            charges.push(BudgetScopeLimit {
                resource_key: format!("__technical_tool:{tool_name}"),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(limit),
            });
        }
        if let Some(limit) = execution_limits.and_then(|limits| limits.max_tool_calls) {
            charges.push(BudgetScopeLimit {
                resource_key: "__task_total".to_string(),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(u64::from(limit)),
            });
        }
        if let Some((execution_id, definition_key)) = subagent_identity {
            let definition = find_subagent(&snapshot.definition, definition_key)?;
            if let Some(limit) = definition
                .get("tool_limits")
                .and_then(|value| value.get(tool_name))
                .and_then(Value::as_u64)
            {
                charges.push(BudgetScopeLimit {
                    resource_key: format!("{definition_key}:{tool_name}"),
                    scope_key: format!("subagent_run:{execution_id}"),
                    amount: 1,
                    limit: Some(limit),
                });
            }
        }
        self.reserve(
            access.task_id(),
            call_id,
            request_fingerprint,
            BudgetResource::ToolCall,
            charges,
        )
        .await
    }

    pub async fn reserve_subagent_run(
        &self,
        access: &AuthorizedExecution,
        call_id: String,
        request_fingerprint: String,
        phase_key: &str,
        parent_execution_step_id: &str,
        definition_key: &str,
    ) -> Result<BudgetReservationResult, ExecutionBudgetUseCaseError> {
        let parent_execution_step_id = Uuid::parse_str(parent_execution_step_id)
            .ok()
            .filter(|step_id| access.execution_step_id() == Some(*step_id))
            .ok_or(ExecutionBudgetUseCaseError::InvalidRequest)?;
        let snapshot = self.task_start_snapshot(access.task_id()).await?;
        let graph = task_graph(&snapshot.definition)?
            .ok_or(ExecutionBudgetUseCaseError::InvalidTaskStartSnapshot)?;
        let phase = graph
            .phases
            .iter()
            .find(|phase| phase.key == phase_key)
            .ok_or(ExecutionBudgetUseCaseError::InvalidRequest)?;
        if !phase.subagents.iter().any(|key| key == definition_key) {
            return Err(ExecutionBudgetUseCaseError::InvalidRequest);
        }
        let limit = phase
            .max_subagent_runs
            .map(u64::from)
            .ok_or(ExecutionBudgetUseCaseError::InvalidTaskStartSnapshot)?;
        let mut charges = vec![BudgetScopeLimit {
            resource_key: phase_key.to_string(),
            scope_key: format!("step:{parent_execution_step_id}"),
            amount: 1,
            limit: Some(limit),
        }];
        if let Some(task_limit) = graph
            .execution_limits
            .as_ref()
            .and_then(|limits| limits.max_subagent_runs)
        {
            charges.push(BudgetScopeLimit {
                resource_key: "__task_total".to_string(),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(u64::from(task_limit)),
            });
        }
        self.reserve(
            access.task_id(),
            call_id,
            request_fingerprint,
            BudgetResource::SubagentRun,
            charges,
        )
        .await
    }

    pub async fn transition_call(
        &self,
        access: &AuthorizedExecution,
        call_id: &str,
        state: BudgetReservationState,
    ) -> Result<BudgetReservationTransition, ExecutionBudgetUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let result = self
            .repository
            .transition_call(
                &transaction,
                access.task_id(),
                call_id,
                state,
                Utc::now().fixed_offset(),
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(result)
    }

    pub async fn complete_call(
        &self,
        access: &AuthorizedExecution,
        call_id: &str,
        response: Value,
    ) -> Result<(), ExecutionBudgetUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        self.repository
            .complete_call(
                &transaction,
                access.task_id(),
                call_id,
                response,
                Utc::now().fixed_offset(),
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    async fn reserve(
        &self,
        task_id: Uuid,
        call_id: String,
        request_fingerprint: String,
        resource: BudgetResource,
        charges: Vec<BudgetScopeLimit>,
    ) -> Result<BudgetReservationResult, ExecutionBudgetUseCaseError> {
        if call_id.trim().is_empty()
            || call_id.len() > 255
            || request_fingerprint.trim().is_empty()
            || request_fingerprint.len() > 64
            || charges.is_empty()
            || charges.iter().any(|charge| {
                charge.resource_key.trim().is_empty()
                    || charge.scope_key.trim().is_empty()
                    || charge.resource_key.len() > 128
                    || charge.scope_key.len() > 128
                    || charge.amount == 0
                    || charge.limit.is_some_and(|limit| charge.amount > limit)
            })
        {
            return Err(ExecutionBudgetUseCaseError::InvalidRequest);
        }
        let transaction = self.unit_of_work.begin().await?;
        let result = self
            .repository
            .reserve(
                &transaction,
                BudgetReservationCommand {
                    task_id,
                    call_id,
                    request_fingerprint,
                    resource,
                    charges,
                    now: Utc::now().fixed_offset(),
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(result)
    }

    async fn task_start_snapshot(
        &self,
        task_id: Uuid,
    ) -> Result<DefinitionSnapshot, ExecutionBudgetUseCaseError> {
        self.snapshots
            .find_definition_snapshot(task_id, "task_start")
            .await?
            .ok_or(ExecutionBudgetUseCaseError::MissingTaskStartSnapshot)
    }
}

fn task_graph(definition: &Value) -> Result<Option<AgentGraphConfig>, ExecutionBudgetUseCaseError> {
    let yaml = definition
        .get("agent_config")
        .and_then(|value| value.get("agent_graph"))
        .and_then(Value::as_str)
        .ok_or(ExecutionBudgetUseCaseError::InvalidTaskStartSnapshot)?;
    parse_agent_graph(yaml).map_err(|_| ExecutionBudgetUseCaseError::InvalidTaskStartSnapshot)
}

fn matching_execution_id(value: &str, expected: Uuid) -> Result<Uuid, ExecutionBudgetUseCaseError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|execution_id| *execution_id == expected)
        .ok_or(ExecutionBudgetUseCaseError::InvalidRequest)
}

fn find_subagent<'a>(
    definition: &'a Value,
    key: &str,
) -> Result<&'a Value, ExecutionBudgetUseCaseError> {
    definition
        .get("subagent_definitions")
        .and_then(Value::as_array)
        .and_then(|definitions| {
            definitions
                .iter()
                .find(|definition| definition.get("key").and_then(Value::as_str) == Some(key))
        })
        .ok_or(ExecutionBudgetUseCaseError::InvalidRequest)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use uuid::Uuid;

    use super::matching_execution_id;

    #[rstest]
    #[case::matching("00000000-0000-0000-0000-000000000001", true)]
    #[case::malformed("not-a-uuid", false)]
    #[case::different("00000000-0000-0000-0000-000000000002", false)]
    fn subagent_scope_matches_the_authorized_execution_step(
        #[case] value: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(
            matching_execution_id(value, Uuid::from_u128(1)).is_ok(),
            expected,
        );
    }
}
