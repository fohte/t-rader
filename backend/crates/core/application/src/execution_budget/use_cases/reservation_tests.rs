use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use indoc::indoc;
use rstest::rstest;
use serde_json::json;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    execution_budget::{
        BudgetReservation, BudgetReservationCommand, BudgetReservationResult,
        BudgetReservationState, BudgetReservationTransition, BudgetResource, BudgetScopeLimit,
        ExecutionBudgetRepository, ExecutionBudgetRepositoryError, ExecutionBudgetUseCases,
        SubagentExecutionIdentity,
    },
    strategy_execution::{
        AuthorizedExecution, DefinitionSnapshot, PayloadCaptureMetadata, PayloadChunk,
        PayloadManifest, PayloadSeal, SharedStrategyExecutionRepository,
        StrategyExecutionRepository, StrategyExecutionRepositoryError,
    },
    unit_of_work::FakeUnitOfWork,
};

const TASK_ID: Uuid = Uuid::from_u128(42);
const STEP_ID: Uuid = Uuid::from_u128(43);
const TOOL_NAME: &str = "demo_tool";
const DEFINITION_KEY: &str = "demo_worker";

#[derive(Clone, Copy)]
enum ReservationCase {
    ParentModelCall,
    SubagentModelCall,
    TaskToolCall,
    SubagentToolCall,
    SubagentRun,
}

#[rstest]
#[case::parent_model_call(ReservationCase::ParentModelCall)]
#[case::subagent_model_call(ReservationCase::SubagentModelCall)]
#[case::task_tool_call(ReservationCase::TaskToolCall)]
#[case::subagent_tool_call(ReservationCase::SubagentToolCall)]
#[case::subagent_run(ReservationCase::SubagentRun)]
#[tokio::test]
async fn reservation_uses_start_snapshot_and_shared_budget_port(#[case] case: ReservationCase) {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let budget_repository = Arc::new(CapturingBudgetRepository::default());
    let use_cases = ExecutionBudgetUseCases::new(
        unit_of_work.clone(),
        budget_repository.clone(),
        snapshot_repository(),
    );
    let access = AuthorizedExecution {
        task_id: TASK_ID,
        strategy_id: Uuid::from_u128(45),
        a2a_task_id: "agent-attempt".to_string(),
        execution_step_id: Some(STEP_ID),
    };

    match case {
        ReservationCase::ParentModelCall => {
            use_cases
                .reserve_model_call(
                    &access,
                    call_id(case),
                    fingerprint(case),
                    &STEP_ID.to_string(),
                    None,
                )
                .await
                .expect("reserve parent model call");
        }
        ReservationCase::SubagentModelCall => {
            let execution_id = STEP_ID.to_string();
            use_cases
                .reserve_model_call(
                    &access,
                    call_id(case),
                    fingerprint(case),
                    &STEP_ID.to_string(),
                    Some(SubagentExecutionIdentity {
                        execution_id: &execution_id,
                        definition_key: DEFINITION_KEY,
                    }),
                )
                .await
                .expect("reserve subagent model call");
        }
        ReservationCase::TaskToolCall => {
            use_cases
                .reserve_tool_call(
                    &access,
                    call_id(case),
                    fingerprint(case),
                    TOOL_NAME,
                    Some(8),
                    None,
                )
                .await
                .expect("reserve task tool call");
        }
        ReservationCase::SubagentToolCall => {
            let execution_id = STEP_ID.to_string();
            use_cases
                .reserve_tool_call(
                    &access,
                    call_id(case),
                    fingerprint(case),
                    TOOL_NAME,
                    Some(8),
                    Some(SubagentExecutionIdentity {
                        execution_id: &execution_id,
                        definition_key: DEFINITION_KEY,
                    }),
                )
                .await
                .expect("reserve subagent tool call");
        }
        ReservationCase::SubagentRun => {
            use_cases
                .reserve_subagent_run(
                    &access,
                    call_id(case),
                    fingerprint(case),
                    "phase_example",
                    &STEP_ID.to_string(),
                    DEFINITION_KEY,
                )
                .await
                .expect("reserve subagent run");
        }
    }

    let command = budget_repository
        .commands
        .lock()
        .await
        .pop()
        .expect("reservation command is captured");
    let actual = (
        normalize_command(command),
        unit_of_work.committed.lock().await.len(),
    );
    assert_eq!(actual, (normalize_command(expected_command(case)), 1),);
}

fn fingerprint(case: ReservationCase) -> String {
    format!("fingerprint:{}", call_id(case))
}

fn call_id(case: ReservationCase) -> String {
    let suffix = match case {
        ReservationCase::ParentModelCall => "parent-model",
        ReservationCase::SubagentModelCall => "subagent-model",
        ReservationCase::TaskToolCall => "task-tool",
        ReservationCase::SubagentToolCall => "subagent-tool",
        ReservationCase::SubagentRun => "subagent-run",
    };
    format!("communication-{suffix}")
}

fn expected_command(case: ReservationCase) -> BudgetReservationCommand {
    let charges = match case {
        ReservationCase::ParentModelCall => vec![
            BudgetScopeLimit {
                resource_key: "model_call".to_string(),
                scope_key: format!("step:{STEP_ID}"),
                amount: 1,
                limit: Some(15),
            },
            BudgetScopeLimit {
                resource_key: "__task_total".to_string(),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(12),
            },
        ],
        ReservationCase::SubagentModelCall => vec![
            BudgetScopeLimit {
                resource_key: format!("subagent:{DEFINITION_KEY}"),
                scope_key: format!("subagent_run:{STEP_ID}"),
                amount: 1,
                limit: Some(4),
            },
            BudgetScopeLimit {
                resource_key: "__task_total".to_string(),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(12),
            },
        ],
        ReservationCase::TaskToolCall => vec![
            BudgetScopeLimit {
                resource_key: TOOL_NAME.to_string(),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(4),
            },
            BudgetScopeLimit {
                resource_key: format!("__technical_tool:{TOOL_NAME}"),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(8),
            },
            BudgetScopeLimit {
                resource_key: "__task_total".to_string(),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(13),
            },
        ],
        ReservationCase::SubagentToolCall => vec![
            BudgetScopeLimit {
                resource_key: TOOL_NAME.to_string(),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(4),
            },
            BudgetScopeLimit {
                resource_key: format!("__technical_tool:{TOOL_NAME}"),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(8),
            },
            BudgetScopeLimit {
                resource_key: "__task_total".to_string(),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(13),
            },
            BudgetScopeLimit {
                resource_key: format!("{DEFINITION_KEY}:{TOOL_NAME}"),
                scope_key: format!("subagent_run:{STEP_ID}"),
                amount: 1,
                limit: Some(2),
            },
        ],
        ReservationCase::SubagentRun => vec![
            BudgetScopeLimit {
                resource_key: "phase_example".to_string(),
                scope_key: format!("step:{STEP_ID}"),
                amount: 1,
                limit: Some(3),
            },
            BudgetScopeLimit {
                resource_key: "__task_total".to_string(),
                scope_key: "task".to_string(),
                amount: 1,
                limit: Some(5),
            },
        ],
    };
    let resource = match case {
        ReservationCase::ParentModelCall | ReservationCase::SubagentModelCall => {
            BudgetResource::ModelCall
        }
        ReservationCase::TaskToolCall | ReservationCase::SubagentToolCall => {
            BudgetResource::ToolCall
        }
        ReservationCase::SubagentRun => BudgetResource::SubagentRun,
    };
    BudgetReservationCommand {
        task_id: TASK_ID,
        call_id: call_id(case),
        request_fingerprint: fingerprint(case),
        resource,
        charges,
        now: Utc::now().fixed_offset(),
    }
}

fn normalize_command(command: BudgetReservationCommand) -> BudgetReservationCommand {
    BudgetReservationCommand {
        now: DateTime::from_timestamp(1_800_000_000, 0)
            .expect("fixed test time is valid")
            .fixed_offset(),
        ..command
    }
}

fn snapshot_repository() -> SharedStrategyExecutionRepository {
    let snapshot = DefinitionSnapshot {
        task_id: TASK_ID,
        definition_key: "task_start".to_string(),
        definition: json!({
            "agent_config": {
                "agent_graph": indoc! {"
                    tool_limits:
                      demo_tool: 4
                    execution_limits:
                      max_model_calls: 12
                      max_tool_calls: 13
                      max_subagent_runs: 5
                    phases:
                      - key: phase_example
                        label: Example phase
                        model: sample-model
                        prompt: Example prompt.
                        subagents: [demo_worker]
                        max_subagent_runs: 3
                "}
            },
            "subagent_definitions": [{
                "key": DEFINITION_KEY,
                "max_model_calls": 4,
                "tool_limits": { (TOOL_NAME): 2 }
            }]
        }),
        digest: "synthetic-snapshot".to_string(),
        created_at: Utc::now().fixed_offset(),
    };
    Arc::new(SnapshotRepository { snapshot })
}

#[derive(Clone)]
struct SnapshotRepository {
    snapshot: DefinitionSnapshot,
}

#[async_trait]
impl StrategyExecutionRepository for SnapshotRepository {
    async fn find_definition_snapshot(
        &self,
        task_id: Uuid,
        definition_key: &str,
    ) -> Result<Option<DefinitionSnapshot>, StrategyExecutionRepositoryError> {
        Ok(
            (task_id == self.snapshot.task_id && definition_key == "task_start")
                .then(|| self.snapshot.clone()),
        )
    }

    async fn insert_definition_snapshot(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        _snapshot: DefinitionSnapshot,
    ) -> Result<DefinitionSnapshot, StrategyExecutionRepositoryError> {
        Err(unexpected_repository_call())
    }

    async fn insert_payload(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        _payload: PayloadManifest,
    ) -> Result<(), StrategyExecutionRepositoryError> {
        Err(unexpected_repository_call())
    }

    async fn find_payload_for_update(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        _task_id: Uuid,
        _payload_id: Uuid,
    ) -> Result<Option<PayloadManifest>, StrategyExecutionRepositoryError> {
        Err(unexpected_repository_call())
    }

    async fn list_payload_chunks_for_update(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        _task_id: Uuid,
        _payload_id: Uuid,
    ) -> Result<Vec<PayloadChunk>, StrategyExecutionRepositoryError> {
        Err(unexpected_repository_call())
    }

    async fn append_payload_chunk(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        _task_id: Uuid,
        _chunk: PayloadChunk,
    ) -> Result<PayloadManifest, StrategyExecutionRepositoryError> {
        Err(unexpected_repository_call())
    }

    async fn update_payload_capture_metadata(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        _task_id: Uuid,
        _payload_id: Uuid,
        _metadata: PayloadCaptureMetadata,
    ) -> Result<(), StrategyExecutionRepositoryError> {
        Err(unexpected_repository_call())
    }

    async fn mark_payload_sealed(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        _seal: PayloadSeal,
    ) -> Result<PayloadManifest, StrategyExecutionRepositoryError> {
        Err(unexpected_repository_call())
    }

    async fn mark_payload_incomplete(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        _task_id: Uuid,
        _payload_id: Uuid,
        _reason: String,
    ) -> Result<(), StrategyExecutionRepositoryError> {
        Err(unexpected_repository_call())
    }

    async fn find_payload(
        &self,
        _task_id: Uuid,
        _payload_id: Uuid,
    ) -> Result<Option<PayloadManifest>, StrategyExecutionRepositoryError> {
        Err(unexpected_repository_call())
    }

    async fn list_payload_chunks(
        &self,
        _task_id: Uuid,
        _payload_id: Uuid,
    ) -> Result<Vec<PayloadChunk>, StrategyExecutionRepositoryError> {
        Err(unexpected_repository_call())
    }

    async fn list_payload_chunks_for_range(
        &self,
        _task_id: Uuid,
        _payload_id: Uuid,
        _start_item: u64,
        _item_count: usize,
    ) -> Result<Vec<PayloadChunk>, StrategyExecutionRepositoryError> {
        Err(unexpected_repository_call())
    }
}

fn unexpected_repository_call() -> StrategyExecutionRepositoryError {
    StrategyExecutionRepositoryError::InvalidTransaction
}

#[derive(Default)]
struct CapturingBudgetRepository {
    commands: Mutex<Vec<BudgetReservationCommand>>,
}

#[async_trait]
impl ExecutionBudgetRepository for CapturingBudgetRepository {
    async fn initialize_task(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        _task_id: Uuid,
        _deadline_at: DateTime<FixedOffset>,
    ) -> Result<(), ExecutionBudgetRepositoryError> {
        Ok(())
    }

    async fn reserve(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        command: BudgetReservationCommand,
    ) -> Result<BudgetReservationResult, ExecutionBudgetRepositoryError> {
        self.commands.lock().await.push(command);
        Ok(BudgetReservationResult {
            reservations: Vec::<BudgetReservation>::new(),
            completed_response: None,
        })
    }

    async fn transition_call(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        _task_id: Uuid,
        _call_id: &str,
        _state: BudgetReservationState,
        _now: DateTime<FixedOffset>,
    ) -> Result<BudgetReservationTransition, ExecutionBudgetRepositoryError> {
        Ok(BudgetReservationTransition {
            reservations: Vec::new(),
            transitioned: false,
        })
    }

    async fn complete_call(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        _task_id: Uuid,
        _call_id: &str,
        _response: serde_json::Value,
        _now: DateTime<FixedOffset>,
    ) -> Result<(), ExecutionBudgetRepositoryError> {
        Ok(())
    }
}
