use chrono::{DateTime, FixedOffset};
use core_application::strategy_task::{
    StrategyTask, StrategyTaskPhase, StrategyTaskRepository, StrategyTaskStepStatus,
};
use core_application::unit_of_work::UnitOfWork;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

use crate::entities::sea_orm_active_enums::StrategyTaskStepStatus as DbStepStatus;
use crate::entities::{agent_config, strategy, strategy_task_step};
use crate::{DatabaseHandle, PostgresUnitOfWork};

use super::PostgresStrategyTaskRepository;

#[derive(Debug, PartialEq, Eq)]
struct ClaimObservation {
    name: &'static str,
    claimed: bool,
    phase: StrategyTaskPhase,
    auto_resumed_at: Option<DateTime<FixedOffset>>,
    deadline_at: DateTime<FixedOffset>,
}

struct ClaimCase {
    name: &'static str,
    phase: StrategyTaskPhase,
    step_status: Option<StrategyTaskStepStatus>,
    auto_resumed_at: Option<DateTime<FixedOffset>>,
    mark_auto_resumed: bool,
    deadline_expired: bool,
    expected_claimed: bool,
    expected_phase: StrategyTaskPhase,
    expected_auto_resumed_at: Option<DateTime<FixedOffset>>,
}

pub(super) fn fixed_time() -> DateTime<FixedOffset> {
    (DateTime::<chrono::Utc>::UNIX_EPOCH + chrono::Duration::days(20_454)).fixed_offset()
}

pub(super) fn task(
    task_id: Uuid,
    strategy_id: Uuid,
    phase: StrategyTaskPhase,
    auto_resumed_at: Option<DateTime<FixedOffset>>,
) -> StrategyTask {
    let now = fixed_time();
    StrategyTask {
        task_id,
        strategy_id,
        a2a_task_id: None,
        source: "review".to_string(),
        prompt: "fictional prompt".to_string(),
        phase,
        error_summary: None,
        result_text: None,
        deadline_at: now,
        purpose: Some("fictional-purpose".to_string()),
        as_of: Some(now),
        context_kind: None,
        context_id: None,
        auto_resumed_at,
        created_at: now,
        updated_at: now,
    }
}

fn step_model(
    task_id: Uuid,
    execution_step_id: Uuid,
    status: StrategyTaskStepStatus,
) -> strategy_task_step::ActiveModel {
    let status = match status {
        StrategyTaskStepStatus::Running => DbStepStatus::Running,
        StrategyTaskStepStatus::Completed => DbStepStatus::Completed,
        StrategyTaskStepStatus::Failed => DbStepStatus::Failed,
    };
    strategy_task_step::ActiveModel {
        execution_step_id: Set(execution_step_id),
        task_id: Set(task_id),
        phase_key: Set("fictional-phase".to_string()),
        kind: Set("llm".to_string()),
        label: Set("Fictional phase".to_string()),
        model: Set(Some("fictional-model".to_string())),
        model_call_limit_reached: Set(false),
        status: Set(status),
        item: Set(None),
        item_key: Set(None),
        item_label: Set(None),
        input_ref: Set(None),
        output_ref: Set(None),
        output: Set(None),
        started_at: Set(fixed_time()),
        finished_at: Set(None),
        trace_id: Set("trace-placeholder".to_string()),
        span_id: Set("span-placeholder".to_string()),
        error: Set(None),
        seq: NotSet,
    }
}

#[backend_test_macros::database_test]
async fn find_agent_graph_by_purpose_returns_matching_configuration(db: DatabaseHandle) {
    for (id, purpose, agent_graph) in [
        (900, "other-purpose", "other-agent-graph"),
        (901, "fictional-purpose", "fictional-agent-graph"),
    ] {
        agent_config::ActiveModel {
            id: Set(Uuid::from_u128(id)),
            purpose: Set(purpose.to_string()),
            agents_md: NotSet,
            skills: NotSet,
            agent_graph: Set(agent_graph.to_string()),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await
        .expect("insert test agent config");
    }

    let repository = PostgresStrategyTaskRepository::new(db);
    let actual = (
        repository
            .find_agent_graph_by_purpose("fictional-purpose")
            .await
            .expect("find matching agent graph"),
        repository
            .find_agent_graph_by_purpose("missing-purpose")
            .await
            .expect("find missing agent graph"),
    );

    assert_eq!(actual, (Some("fictional-agent-graph".to_string()), None,),);
}

#[backend_test_macros::database_test]
async fn claim_resumable_updates_only_eligible_tasks(db: DatabaseHandle) {
    let strategy_id = Uuid::from_u128(100);
    strategy::ActiveModel {
        id: Set(strategy_id),
        name: Set("Fictional strategy".to_string()),
        description: Set(None),
        sort_order: Set(0),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(&db)
    .await
    .expect("insert test strategy");

    let now = fixed_time();
    let deadline_at = now + chrono::Duration::minutes(30);
    let previous_auto_resume = now + chrono::Duration::hours(1);
    let cases = [
        ClaimCase {
            name: "failed task is manually resumable",
            phase: StrategyTaskPhase::Failed,
            step_status: None,
            auto_resumed_at: None,
            mark_auto_resumed: false,
            deadline_expired: false,
            expected_claimed: true,
            expected_phase: StrategyTaskPhase::Running,
            expected_auto_resumed_at: None,
        },
        ClaimCase {
            name: "completed task with failed step is resumable",
            phase: StrategyTaskPhase::Completed,
            step_status: Some(StrategyTaskStepStatus::Failed),
            auto_resumed_at: None,
            mark_auto_resumed: false,
            deadline_expired: false,
            expected_claimed: true,
            expected_phase: StrategyTaskPhase::Running,
            expected_auto_resumed_at: None,
        },
        ClaimCase {
            name: "completed task without steps is not resumable",
            phase: StrategyTaskPhase::Completed,
            step_status: None,
            auto_resumed_at: None,
            mark_auto_resumed: false,
            deadline_expired: false,
            expected_claimed: false,
            expected_phase: StrategyTaskPhase::Completed,
            expected_auto_resumed_at: None,
        },
        ClaimCase {
            name: "completed task with completed step is not resumable",
            phase: StrategyTaskPhase::Completed,
            step_status: Some(StrategyTaskStepStatus::Completed),
            auto_resumed_at: None,
            mark_auto_resumed: false,
            deadline_expired: false,
            expected_claimed: false,
            expected_phase: StrategyTaskPhase::Completed,
            expected_auto_resumed_at: None,
        },
        ClaimCase {
            name: "running task with failed step is not resumable",
            phase: StrategyTaskPhase::Running,
            step_status: Some(StrategyTaskStepStatus::Failed),
            auto_resumed_at: None,
            mark_auto_resumed: false,
            deadline_expired: false,
            expected_claimed: false,
            expected_phase: StrategyTaskPhase::Running,
            expected_auto_resumed_at: None,
        },
        ClaimCase {
            name: "pending task with failed step is not resumable",
            phase: StrategyTaskPhase::Pending,
            step_status: Some(StrategyTaskStepStatus::Failed),
            auto_resumed_at: None,
            mark_auto_resumed: false,
            deadline_expired: false,
            expected_claimed: false,
            expected_phase: StrategyTaskPhase::Pending,
            expected_auto_resumed_at: None,
        },
        ClaimCase {
            name: "already auto-resumed task is not claimed again",
            phase: StrategyTaskPhase::Failed,
            step_status: None,
            auto_resumed_at: Some(previous_auto_resume),
            mark_auto_resumed: true,
            deadline_expired: false,
            expected_claimed: false,
            expected_phase: StrategyTaskPhase::Failed,
            expected_auto_resumed_at: Some(previous_auto_resume),
        },
        ClaimCase {
            name: "manual resume preserves auto-resume marker",
            phase: StrategyTaskPhase::Failed,
            step_status: None,
            auto_resumed_at: Some(previous_auto_resume),
            mark_auto_resumed: false,
            deadline_expired: false,
            expected_claimed: true,
            expected_phase: StrategyTaskPhase::Running,
            expected_auto_resumed_at: Some(previous_auto_resume),
        },
        ClaimCase {
            name: "expired task is not manually resumable",
            phase: StrategyTaskPhase::Failed,
            step_status: None,
            auto_resumed_at: None,
            mark_auto_resumed: false,
            deadline_expired: true,
            expected_claimed: false,
            expected_phase: StrategyTaskPhase::Failed,
            expected_auto_resumed_at: None,
        },
        ClaimCase {
            name: "expired task is not automatically resumable",
            phase: StrategyTaskPhase::Failed,
            step_status: None,
            auto_resumed_at: None,
            mark_auto_resumed: true,
            deadline_expired: true,
            expected_claimed: false,
            expected_phase: StrategyTaskPhase::Failed,
            expected_auto_resumed_at: None,
        },
    ];

    let repository = PostgresStrategyTaskRepository::new(db.clone());
    let unit_of_work = PostgresUnitOfWork::new(db);
    let transaction = unit_of_work.begin().await.expect("begin setup transaction");
    for (index, case) in cases.iter().enumerate() {
        let task_id = Uuid::from_u128(200 + index as u128);
        let mut strategy_task = task(task_id, strategy_id, case.phase, case.auto_resumed_at);
        strategy_task.deadline_at = if case.deadline_expired {
            now
        } else {
            deadline_at
        };
        repository
            .insert(&transaction, strategy_task)
            .await
            .expect("insert test task");
        let setup_transaction = transaction
            .downcast_ref::<sea_orm::DatabaseTransaction>()
            .expect("postgres transaction");
        setup_transaction
                .execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "INSERT INTO task_execution_state (task_id, deadline_at, created_at, updated_at) VALUES ($1, $2, $3, $3)",
                    [
                        task_id.into(),
                        if case.deadline_expired { now } else { deadline_at }.into(),
                        now.into(),
                    ],
                ))
                .await
                .expect("insert test execution state");
        if let Some(status) = case.step_status {
            step_model(task_id, Uuid::from_u128(300 + index as u128), status)
                .insert(
                    transaction
                        .downcast_ref::<sea_orm::DatabaseTransaction>()
                        .expect("postgres transaction"),
                )
                .await
                .expect("insert test task step");
        }
    }
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit setup transaction");

    let mut actual = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let task_id = Uuid::from_u128(200 + index as u128);
        let transaction = unit_of_work.begin().await.expect("begin claim transaction");
        let claimed = repository
            .claim_resumable(&transaction, task_id, now, case.mark_auto_resumed)
            .await
            .expect("claim resumable task");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit claim transaction");
        let task = repository
            .find_by_id(task_id)
            .await
            .expect("read test task")
            .expect("test task exists");
        actual.push(ClaimObservation {
            name: case.name,
            claimed,
            phase: task.phase,
            auto_resumed_at: task.auto_resumed_at,
            deadline_at: task.deadline_at,
        });
    }

    assert_eq!(
        actual,
        cases
            .iter()
            .map(|case| ClaimObservation {
                name: case.name,
                claimed: case.expected_claimed,
                phase: case.expected_phase,
                auto_resumed_at: case.expected_auto_resumed_at,
                deadline_at: if case.deadline_expired {
                    now
                } else {
                    deadline_at
                },
            })
            .collect::<Vec<_>>(),
    );
}
