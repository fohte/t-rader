use chrono::{DateTime, FixedOffset};
use core_application::strategy_task::{
    StrategyTaskPhase, StrategyTaskRepository, StrategyTaskStep, StrategyTaskStepKind,
    StrategyTaskStepStatus,
};
use core_application::unit_of_work::UnitOfWork;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use uuid::Uuid;

use crate::entities::sea_orm_active_enums::StrategyTaskStepStatus as DbStepStatus;
use crate::entities::{strategy, strategy_task_step};
use crate::{DatabaseHandle, PostgresUnitOfWork};

use super::super::PostgresStrategyTaskRepository;
use super::super::tests::{fixed_time, task};

#[test]
fn step_wire_defaults_legacy_payload_to_llm() {
    let now = fixed_time();
    let actual: super::StepWire = serde_json::from_value(serde_json::json!({
        "execution_step_id": "00000000-0000-0000-0000-000000000451",
        "phase_key": "fictional-phase",
        "label": "Fictional phase",
        "model": "fictional-model",
        "status": "completed",
        "started_at": now,
        "trace_id": "fictional-trace",
        "span_id": "fictional-span",
    }))
    .expect("parse legacy step wire");

    assert_eq!(
        actual,
        super::StepWire {
            execution_step_id: Uuid::from_u128(0x451),
            phase_key: "fictional-phase".to_string(),
            kind: "llm".to_string(),
            label: "Fictional phase".to_string(),
            model: Some("fictional-model".to_string()),
            model_call_limit_reached: false,
            status: "completed".to_string(),
            item: None,
            item_key: None,
            item_label: None,
            input_ref: None,
            output_ref: None,
            output: None,
            started_at: now,
            finished_at: None,
            trace_id: "fictional-trace".to_string(),
            span_id: "fictional-span".to_string(),
            error: None,
        },
    );
}

#[test]
fn step_wire_accepts_operation_references_without_a_model() {
    let now = fixed_time();
    let input_ref = serde_json::json!({ "payload_id": "00000000-0000-0000-0000-000000000452", "pointer": "/input" });
    let output_ref = serde_json::json!({ "payload_id": "00000000-0000-0000-0000-000000000453", "pointer": "/value" });
    let actual: super::StepWire = serde_json::from_value(serde_json::json!({
            "execution_step_id": "00000000-0000-0000-0000-000000000454",
            "phase_key": "fictional-operation",
            "kind": "operation",
            "label": "Fictional operation",
            "model": null,
            "status": "completed",
            "model_call_limit_reached": true,
            "item_key": { "payload_id": "00000000-0000-0000-0000-000000000455", "pointer": "/items", "index": 3 },
            "input_ref": input_ref,
            "output_ref": output_ref,
            "output": { "count": 1 },
            "started_at": now,
            "trace_id": "fictional-trace",
            "span_id": "fictional-span",
        }))
        .expect("parse operation step wire");

    assert_eq!(
        actual,
        super::StepWire {
            execution_step_id: Uuid::from_u128(0x454),
            phase_key: "fictional-operation".to_string(),
            kind: "operation".to_string(),
            label: "Fictional operation".to_string(),
            model: None,
            model_call_limit_reached: true,
            status: "completed".to_string(),
            item: None,
            item_key: Some(serde_json::json!({
                "payload_id": "00000000-0000-0000-0000-000000000455",
                "pointer": "/items",
                "index": 3,
            })),
            item_label: None,
            input_ref: Some(input_ref),
            output_ref: Some(output_ref),
            output: Some(serde_json::json!({ "count": 1 })),
            started_at: now,
            finished_at: None,
            trace_id: "fictional-trace".to_string(),
            span_id: "fictional-span".to_string(),
            error: None,
        },
    );
}

#[derive(Debug, PartialEq)]
struct StepView {
    execution_step_id: Uuid,
    task_id: Uuid,
    phase_key: String,
    kind: StrategyTaskStepKind,
    label: String,
    model: Option<String>,
    model_call_limit_reached: bool,
    status: StrategyTaskStepStatus,
    item: Option<serde_json::Value>,
    item_key: Option<serde_json::Value>,
    item_label: Option<String>,
    input_ref: Option<serde_json::Value>,
    output_ref: Option<serde_json::Value>,
    output: Option<serde_json::Value>,
    started_at: DateTime<FixedOffset>,
    finished_at: Option<DateTime<FixedOffset>>,
    trace_id: String,
    span_id: String,
    error: Option<String>,
    seq: i64,
}

impl From<StrategyTaskStep> for StepView {
    fn from(step: StrategyTaskStep) -> Self {
        Self {
            execution_step_id: step.execution_step_id,
            task_id: step.task_id,
            phase_key: step.phase_key,
            kind: step.kind,
            label: step.label,
            model: step.model,
            model_call_limit_reached: step.model_call_limit_reached,
            status: step.status,
            item: step.item,
            item_key: step.item_key,
            item_label: step.item_label,
            input_ref: step.input_ref,
            output_ref: step.output_ref,
            output: step.output,
            started_at: step.started_at,
            finished_at: step.finished_at,
            trace_id: step.trace_id,
            span_id: step.span_id,
            error: step.error,
            seq: step.seq,
        }
    }
}

#[backend_test_macros::database_test]
async fn list_steps_returns_every_field_in_sequence_order(db: DatabaseHandle) {
    let strategy_id = Uuid::from_u128(400);
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

    let task_id = Uuid::from_u128(401);
    let repository = PostgresStrategyTaskRepository::new(db.clone());
    let unit_of_work = PostgresUnitOfWork::new(db);
    let transaction = unit_of_work.begin().await.expect("begin setup transaction");
    repository
        .insert(
            &transaction,
            task(task_id, strategy_id, StrategyTaskPhase::Completed, None),
        )
        .await
        .expect("insert test task");

    let first_started_at = fixed_time();
    let second_started_at = first_started_at + chrono::Duration::minutes(1);
    let first_finished_at = first_started_at + chrono::Duration::minutes(2);
    let first_step_id = Uuid::from_u128(402);
    let second_step_id = Uuid::from_u128(403);
    let transaction_ref = transaction
        .downcast_ref::<sea_orm::DatabaseTransaction>()
        .expect("postgres transaction");
    strategy_task_step::ActiveModel {
        execution_step_id: Set(first_step_id),
        task_id: Set(task_id),
        phase_key: Set("fictional-complete-phase".to_string()),
        kind: Set("operation".to_string()),
        label: Set("Completed phase".to_string()),
        model: Set(None),
        model_call_limit_reached: Set(true),
        status: Set(DbStepStatus::Completed),
        item: Set(Some(serde_json::json!({ "ticker": "FAKE" }))),
        item_key: Set(Some(serde_json::json!({
            "payload_id": "00000000-0000-0000-0000-000000000444",
            "pointer": "/items",
            "index": 2,
        }))),
        item_label: Set(Some("Fictional item".to_string())),
        input_ref: Set(Some(serde_json::json!({
            "payload_id": "00000000-0000-0000-0000-000000000445",
            "pointer": "/input",
        }))),
        output_ref: Set(Some(serde_json::json!({
            "payload_id": "00000000-0000-0000-0000-000000000446",
            "pointer": "/value",
        }))),
        output: Set(Some(serde_json::json!({ "summary": "fictional output" }))),
        started_at: Set(first_started_at),
        finished_at: Set(Some(first_finished_at)),
        trace_id: Set("fictional-trace".to_string()),
        span_id: Set("fictional-span".to_string()),
        error: Set(Some("fictional warning".to_string())),
        seq: Set(20),
    }
    .insert(transaction_ref)
    .await
    .expect("insert first test task step");
    strategy_task_step::ActiveModel {
        execution_step_id: Set(second_step_id),
        task_id: Set(task_id),
        phase_key: Set("fictional-running-phase".to_string()),
        kind: Set("llm".to_string()),
        label: Set("Running phase".to_string()),
        model: Set(Some("fictional-model".to_string())),
        model_call_limit_reached: Set(false),
        status: Set(DbStepStatus::Running),
        item: Set(None),
        item_key: Set(None),
        item_label: Set(None),
        input_ref: Set(None),
        output_ref: Set(None),
        output: Set(None),
        started_at: Set(second_started_at),
        finished_at: Set(None),
        trace_id: Set("trace-placeholder".to_string()),
        span_id: Set("span-placeholder".to_string()),
        error: Set(None),
        seq: Set(10),
    }
    .insert(transaction_ref)
    .await
    .expect("insert second test task step");
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit setup transaction");

    let actual = repository
        .list_steps(task_id)
        .await
        .expect("list test task steps")
        .into_iter()
        .map(StepView::from)
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            StepView {
                execution_step_id: second_step_id,
                task_id,
                phase_key: "fictional-running-phase".to_string(),
                kind: StrategyTaskStepKind::Llm,
                label: "Running phase".to_string(),
                model: Some("fictional-model".to_string()),
                model_call_limit_reached: false,
                status: StrategyTaskStepStatus::Running,
                item: None,
                item_key: None,
                item_label: None,
                input_ref: None,
                output_ref: None,
                output: None,
                started_at: second_started_at,
                finished_at: None,
                trace_id: "trace-placeholder".to_string(),
                span_id: "span-placeholder".to_string(),
                error: None,
                seq: 10,
            },
            StepView {
                execution_step_id: first_step_id,
                task_id,
                phase_key: "fictional-complete-phase".to_string(),
                kind: StrategyTaskStepKind::Operation,
                label: "Completed phase".to_string(),
                model: None,
                model_call_limit_reached: true,
                status: StrategyTaskStepStatus::Completed,
                item: Some(serde_json::json!({ "ticker": "FAKE" })),
                item_key: Some(serde_json::json!({
                    "payload_id": "00000000-0000-0000-0000-000000000444",
                    "pointer": "/items",
                    "index": 2,
                })),
                item_label: Some("Fictional item".to_string()),
                input_ref: Some(serde_json::json!({
                    "payload_id": "00000000-0000-0000-0000-000000000445",
                    "pointer": "/input",
                })),
                output_ref: Some(serde_json::json!({
                    "payload_id": "00000000-0000-0000-0000-000000000446",
                    "pointer": "/value",
                })),
                output: Some(serde_json::json!({ "summary": "fictional output" })),
                started_at: first_started_at,
                finished_at: Some(first_finished_at),
                trace_id: "fictional-trace".to_string(),
                span_id: "fictional-span".to_string(),
                error: Some("fictional warning".to_string()),
                seq: 20,
            },
        ],
    );
}

#[backend_test_macros::database_test]
async fn step_limit_mark_survives_later_upserts(db: DatabaseHandle) {
    let strategy_id = Uuid::from_u128(0x510);
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

    let task_id = Uuid::from_u128(0x511);
    let execution_step_id = Uuid::from_u128(0x512);
    let repository = PostgresStrategyTaskRepository::new(db.clone());
    let unit_of_work = PostgresUnitOfWork::new(db);
    let transaction = unit_of_work.begin().await.expect("begin setup transaction");
    repository
        .insert(
            &transaction,
            task(task_id, strategy_id, StrategyTaskPhase::Running, None),
        )
        .await
        .expect("insert test task");
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit setup transaction");

    let started_at = fixed_time();
    let finished_at = started_at + chrono::Duration::seconds(1);
    let transaction = unit_of_work
        .begin()
        .await
        .expect("begin marked step transaction");
    repository
        .apply_status_and_steps(
            &transaction,
            task_id,
            None,
            None,
            Some(serde_json::json!([{
                "execution_step_id": execution_step_id,
                "phase_key": "fictional-phase",
                "kind": "llm",
                "label": "Fictional phase",
                "model": "fictional-model",
                "status": "running",
                "started_at": started_at,
                "trace_id": "fictional-trace",
                "span_id": "fictional-span",
                "model_call_limit_reached": true,
            }])),
        )
        .await
        .expect("persist limit mark");
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit marked step");

    let transaction = unit_of_work
        .begin()
        .await
        .expect("begin update transaction");
    repository
        .apply_status_and_steps(
            &transaction,
            task_id,
            None,
            None,
            Some(serde_json::json!([{
                "execution_step_id": execution_step_id,
                "phase_key": "fictional-phase",
                "kind": "llm",
                "label": "Fictional phase",
                "model": "fictional-model",
                "status": "completed",
                "started_at": started_at,
                "finished_at": finished_at,
                "trace_id": "fictional-trace",
                "span_id": "fictional-span",
                "output": { "summary": "fictional result" },
            }])),
        )
        .await
        .expect("update marked step");
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit step update");

    let mut actual = repository
        .list_steps(task_id)
        .await
        .expect("list updated task step")
        .into_iter()
        .map(StepView::from)
        .collect::<Vec<_>>();
    actual[0].seq = 0;
    assert_eq!(
        actual,
        vec![StepView {
            execution_step_id,
            task_id,
            phase_key: "fictional-phase".to_string(),
            kind: StrategyTaskStepKind::Llm,
            label: "Fictional phase".to_string(),
            model: Some("fictional-model".to_string()),
            model_call_limit_reached: true,
            status: StrategyTaskStepStatus::Completed,
            item: None,
            item_key: None,
            item_label: None,
            input_ref: None,
            output_ref: None,
            output: Some(serde_json::json!({ "summary": "fictional result" })),
            started_at,
            finished_at: Some(finished_at),
            trace_id: "fictional-trace".to_string(),
            span_id: "fictional-span".to_string(),
            error: None,
            seq: 0,
        }],
    );
}
