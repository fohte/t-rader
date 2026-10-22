use axum::http::{HeaderMap, HeaderValue};
use chrono::Utc;
use core_application::{
    agent_task_client::{
        AgentTaskState, AgentTaskStatus, EXECUTION_LOST_ERROR_KIND, FakeAgentTaskClient,
    },
    strategy_task::TaskSource,
    subagent_definition::SubagentDefinitionFields,
};
use indoc::formatdoc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde_json::{Value, json};
use uuid::Uuid;

use gateway_postgres::entities::{
    execution_reservation, sea_orm_active_enums::StrategyTaskPhase as DbPhase, strategy,
    task_execution_state,
};

#[derive(Debug, Clone, Copy)]
enum ResumeMode {
    Manual,
    Automatic,
}

#[backend_test_macros::database_test]
async fn task_global_limits_survive_manual_and_automatic_resume(
    db: gateway_postgres::DatabaseHandle,
) {
    let manual = reserve_after_resume(&db, ResumeMode::Manual).await;
    let automatic = reserve_after_resume(&db, ResumeMode::Automatic).await;
    assert_eq!(
        (manual, automatic),
        (
            expected_outcome("synthetic-manual-attempt-two"),
            expected_outcome("synthetic-auto-attempt-two"),
        ),
    );
}

#[backend_test_macros::database_test]
async fn zero_model_and_subagent_task_limits_reject_without_reservations(
    db: gateway_postgres::DatabaseHandle,
) {
    const CHILD_KEY: &str = "demo_worker";
    let strategy_id = insert_strategy(&db).await;
    let use_cases =
        crate::services::use_cases::build_use_cases_without_agent_task_client(db.clone());
    let purpose = format!("sample-zero-budget-{}", Uuid::new_v4());
    use_cases
        .agent_configs()
        .create(purpose.clone())
        .await
        .expect("create synthetic agent config");
    use_cases
        .subagent_definitions()
        .create(
            CHILD_KEY.to_string(),
            SubagentDefinitionFields {
                description: "Synthetic helper".to_string(),
                model: "sample-model".to_string(),
                reasoning_effort: None,
                prompt: "Use the provided synthetic input.".to_string(),
                tools: Vec::new(),
                timeout_minutes: 1,
                max_model_calls: 1,
                tool_limits: std::collections::BTreeMap::new(),
                output: serde_json::Map::new(),
            },
        )
        .await
        .expect("create synthetic child definition");
    let graph = formatdoc! {"
        execution_limits:
          max_model_calls: 0
          max_tool_calls: 0
          max_subagent_runs: 0
        phases:
          - key: demo_phase
            label: Synthetic phase
            model: sample-model
            prompt: Use only synthetic input.
            subagents: [{CHILD_KEY}]
            max_subagent_runs: 1
    "};
    use_cases
        .agent_configs()
        .save_agent_graph(&purpose, &graph)
        .await
        .expect("save synthetic graph configuration");
    let fake = FakeAgentTaskClient::new();
    let a2a_task_id = format!("synthetic-zero-budget-{}", Uuid::new_v4());
    fake.set_next_task_id(&a2a_task_id).await;
    let task = use_cases
        .strategy_tasks()
        .submit_task(
            &fake,
            strategy_id,
            "synthetic budget task",
            TaskSource::Review,
            Some(purpose),
        )
        .await
        .expect("submit synthetic task");
    let server = crate::mcp::StrategyServer::new(crate::mcp::strategy_server_dependencies(
        &use_cases, None, None, None, None,
    ));
    let step_id = Uuid::from_u128(404);
    let execution = ExecutionIdentity {
        strategy_id,
        task_id: task.task_id,
        a2a_task_id: &task.a2a_task_id,
        execution_step_id: step_id,
        subagent: None,
    };
    let model_call = invoke_budget_operation(
        &server,
        execution,
        json!({
            "action": "reserve_model_call",
            "call_id": "synthetic-zero-model-call",
        }),
    )
    .await;
    let child_run = invoke_budget_operation(
        &server,
        execution,
        json!({
            "action": "reserve_subagent_run",
            "call_id": "synthetic-zero-child-run",
            "phase_key": "demo_phase",
            "parent_execution_step_id": step_id,
            "definition_key": CHILD_KEY,
        }),
    )
    .await;
    let state = task_execution_state::Entity::find_by_id(task.task_id)
        .one(&db)
        .await
        .expect("query task execution budget state")
        .expect("task execution budget state exists");
    let reservation_count = execution_reservation::Entity::find()
        .filter(execution_reservation::Column::TaskId.eq(task.task_id))
        .count(&db)
        .await
        .expect("count task execution reservations");

    assert_eq!(
        (
            model_call,
            child_run,
            state.model_calls_used,
            state.tool_calls_used,
            state.subagent_runs_used,
            reservation_count,
        ),
        (
            Err("execution budget limit was exceeded for __task_total".into()),
            Err("execution budget limit was exceeded for __task_total".into()),
            0,
            0,
            0,
            0,
        ),
    );
}

fn expected_outcome(resumed_task_id: &str) -> BudgetResumeOutcome {
    BudgetResumeOutcome {
        resumed_task_id: resumed_task_id.to_string(),
        task_and_child_limit_responses: std::array::from_fn(|_| {
            Err("execution budget limit was exceeded for __task_total".to_string())
        }),
        initial_budget_api_responses: std::array::from_fn(|_| expected_budget_api_response()),
        initial_tool_response: Ok(empty_portfolio()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BudgetResumeOutcome {
    resumed_task_id: String,
    task_and_child_limit_responses: [Result<Value, String>; 5],
    initial_budget_api_responses: [BudgetApiResponse; 2],
    initial_tool_response: Result<Value, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BudgetApiResponse {
    reservation: Result<Value, String>,
    transition: Result<Value, String>,
}

fn expected_budget_api_response() -> BudgetApiResponse {
    BudgetApiResponse {
        reservation: Ok(json!({"available": true})),
        transition: Ok(json!({"transitioned": true})),
    }
}

fn empty_portfolio() -> Value {
    json!({
        "priced_at": null,
        "account": {
            "trade_count": 0,
            "realized_pnl": 0.0,
            "market_value": -0.0,
            "positions": [],
        },
        "strategy": {
            "trade_count": 0,
            "realized_pnl": 0.0,
            "market_value": -0.0,
            "positions": [],
            "investable_amount": null,
            "unused_investable_amount": null,
        },
    })
}

async fn reserve_after_resume(
    db: &gateway_postgres::DatabaseHandle,
    resume_mode: ResumeMode,
) -> BudgetResumeOutcome {
    let strategy_id = insert_strategy(db).await;
    let use_cases =
        crate::services::use_cases::build_use_cases_without_agent_task_client(db.clone());
    let (purpose, child_key, initial_attempt_id, resumed_attempt_id) = match resume_mode {
        ResumeMode::Manual => (
            "synthetic-manual-purpose",
            "demo_worker_manual",
            "synthetic-manual-attempt-one",
            "synthetic-manual-attempt-two",
        ),
        ResumeMode::Automatic => (
            "synthetic-auto-purpose",
            "demo_worker_auto",
            "synthetic-auto-attempt-one",
            "synthetic-auto-attempt-two",
        ),
    };
    use_cases
        .agent_configs()
        .create(purpose.to_string())
        .await
        .expect("create synthetic agent config");
    use_cases
        .subagent_definitions()
        .create(
            child_key.to_string(),
            SubagentDefinitionFields {
                description: "Synthetic helper".to_string(),
                model: "sample-model".to_string(),
                reasoning_effort: None,
                prompt: "Use the provided synthetic input.".to_string(),
                tools: vec!["read_portfolio".to_string()],
                timeout_minutes: 1,
                max_model_calls: 3,
                tool_limits: [("read_portfolio".to_string(), 3)].into(),
                output: serde_json::Map::new(),
            },
        )
        .await
        .expect("create synthetic child definition");
    let graph = formatdoc! {"
                execution_limits:
                  max_model_calls: 1
                  max_tool_calls: 1
                  max_subagent_runs: 1
                tool_limits:
                  read_portfolio: 3
                phases:
                  - key: demo_phase
                    label: Synthetic phase
                    model: sample-model
                    prompt: Use only synthetic input.
                    subagents: [{child_key}]
                    max_subagent_runs: 3
            "};
    use_cases
        .agent_configs()
        .save_agent_graph(purpose, &graph)
        .await
        .expect("save synthetic graph configuration");

    let fake = FakeAgentTaskClient::new();
    fake.set_next_task_id(initial_attempt_id).await;
    let submitted = use_cases
        .strategy_tasks()
        .submit_task(
            &fake,
            strategy_id,
            "synthetic task prompt",
            TaskSource::Review,
            Some(purpose.to_string()),
        )
        .await
        .expect("submit synthetic task");
    let server = crate::mcp::StrategyServer::new(crate::mcp::strategy_server_dependencies(
        &use_cases, None, None, None, None,
    ));
    let first_step_id = Uuid::from_u128(101);
    let initial_execution = ExecutionIdentity {
        strategy_id,
        task_id: submitted.task_id,
        a2a_task_id: &submitted.a2a_task_id,
        execution_step_id: first_step_id,
        subagent: None,
    };
    let initial_budget_api_responses = [
        reserve_and_consume(
            &server,
            initial_execution,
            json!({
                "action": "reserve_model_call",
                "call_id": "synthetic-model-call-one",
            }),
        )
        .await,
        reserve_and_consume(
            &server,
            initial_execution,
            json!({
                "action": "reserve_subagent_run",
                "call_id": "synthetic-child-run-one",
                "phase_key": "demo_phase",
                "parent_execution_step_id": first_step_id,
                "definition_key": child_key,
            }),
        )
        .await,
    ];
    let initial_tool_response =
        invoke_strategy_tool(&server, initial_execution, "read_portfolio", json!({})).await;

    fake.set_next_task_id(resumed_attempt_id).await;
    let resumed_task_id = match resume_mode {
        ResumeMode::Manual => {
            gateway_postgres::entities::strategy_task::ActiveModel {
                task_id: Set(submitted.task_id),
                phase: Set(DbPhase::Failed),
                ..Default::default()
            }
            .update(db)
            .await
            .expect("mark task failed before manual resume");
            use_cases
                .strategy_tasks()
                .resume(&fake, submitted.task_id)
                .await
                .expect("resume synthetic task")
                .a2a_task_id
        }
        ResumeMode::Automatic => {
            fake.set_status(
                &submitted.a2a_task_id,
                AgentTaskStatus {
                    state: AgentTaskState::Failed,
                    result_text: None,
                    error_message: None,
                    error_kind: Some(EXECUTION_LOST_ERROR_KIND.to_string()),
                    steps: None,
                },
            )
            .await;
            let tasks = use_cases.strategy_tasks();
            let task = tasks
                .list_in_flight_tasks()
                .await
                .expect("list in-flight synthetic task")
                .into_iter()
                .find(|task| task.task_id == submitted.task_id)
                .expect("submitted task is in flight");
            tasks
                .reconcile_task(&fake, task, Utc::now().fixed_offset())
                .await
                .expect("reconcile and automatically resume task");
            resumed_attempt_id.to_string()
        }
    };
    let second_step_id = Uuid::from_u128(202);
    let child_model_step_id = Uuid::from_u128(303);
    let child_execution = ExecutionIdentity {
        strategy_id,
        task_id: submitted.task_id,
        a2a_task_id: &resumed_task_id,
        execution_step_id: child_model_step_id,
        subagent: Some((child_model_step_id, child_key)),
    };
    let resumed_root_execution = ExecutionIdentity {
        strategy_id,
        task_id: submitted.task_id,
        a2a_task_id: &resumed_task_id,
        execution_step_id: second_step_id,
        subagent: None,
    };
    let task_and_child_limit_responses = [
        invoke_budget_operation(
            &server,
            child_execution,
            json!({
                "action": "reserve_model_call",
                "call_id": "synthetic-child-model-call",
            }),
        )
        .await,
        invoke_strategy_tool(&server, child_execution, "read_portfolio", json!({})).await,
        invoke_budget_operation(
            &server,
            resumed_root_execution,
            json!({
                "action": "reserve_subagent_run",
                "call_id": "synthetic-child-run-two",
                "phase_key": "demo_phase",
                "parent_execution_step_id": second_step_id,
                "definition_key": child_key,
            }),
        )
        .await,
        invoke_budget_operation(
            &server,
            resumed_root_execution,
            json!({
                "action": "reserve_model_call",
                "call_id": "synthetic-model-call-two",
            }),
        )
        .await,
        invoke_strategy_tool(&server, resumed_root_execution, "read_portfolio", json!({})).await,
    ];

    BudgetResumeOutcome {
        resumed_task_id,
        task_and_child_limit_responses,
        initial_budget_api_responses,
        initial_tool_response,
    }
}

#[derive(Clone, Copy)]
struct ExecutionIdentity<'a> {
    strategy_id: Uuid,
    task_id: Uuid,
    a2a_task_id: &'a str,
    execution_step_id: Uuid,
    subagent: Option<(Uuid, &'a str)>,
}

async fn reserve_and_consume(
    server: &crate::mcp::StrategyServer,
    execution: ExecutionIdentity<'_>,
    reservation: Value,
) -> BudgetApiResponse {
    let call_id = reservation["call_id"]
        .as_str()
        .expect("reservation call ID is a string")
        .to_string();
    let reservation = invoke_budget_operation(server, execution, reservation).await;
    let transition = invoke_budget_operation(
        server,
        execution,
        json!({"action": "transition", "call_id": call_id, "state": "consumed"}),
    )
    .await;
    BudgetApiResponse {
        reservation,
        transition,
    }
}

async fn invoke_budget_operation(
    server: &crate::mcp::StrategyServer,
    execution: ExecutionIdentity<'_>,
    arguments: Value,
) -> Result<Value, String> {
    let headers = execution_headers(execution);
    crate::integration_tests::mcp_tool::call_tool_output_with_headers::<_, Value>(
        server,
        "__execution_budget",
        arguments,
        headers,
    )
    .await
    .map_err(|error| error.message.to_string())
}

async fn invoke_strategy_tool(
    server: &crate::mcp::StrategyServer,
    execution: ExecutionIdentity<'_>,
    tool_name: &'static str,
    arguments: Value,
) -> Result<Value, String> {
    crate::integration_tests::mcp_tool::call_tool_output_with_headers::<_, Value>(
        server,
        tool_name,
        arguments,
        execution_headers(execution),
    )
    .await
    .map_err(|error| error.message.to_string())
}

fn execution_headers(execution: ExecutionIdentity<'_>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-strategy-id",
        HeaderValue::from_str(&execution.strategy_id.to_string()).expect("valid strategy ID"),
    );
    headers.insert(
        "x-strategy-task-id",
        HeaderValue::from_str(&execution.task_id.to_string()).expect("valid strategy task ID"),
    );
    headers.insert(
        "x-execution-id",
        HeaderValue::from_str(&format!(
            "{}:{}",
            execution.a2a_task_id, execution.execution_step_id
        ))
        .expect("valid attempt and step ID"),
    );
    if let Some((subagent_execution_id, definition_key)) = execution.subagent {
        headers.insert(
            "x-subagent-execution-id",
            HeaderValue::from_str(&subagent_execution_id.to_string())
                .expect("valid subagent execution ID"),
        );
        headers.insert(
            "x-subagent-definition-key",
            HeaderValue::from_str(definition_key).expect("valid subagent definition key"),
        );
    }
    headers
}

async fn insert_strategy(db: &gateway_postgres::DatabaseHandle) -> Uuid {
    let strategy_id = Uuid::new_v4();
    strategy::ActiveModel {
        id: Set(strategy_id),
        name: Set("synthetic strategy".to_string()),
        description: Set(None),
        sort_order: Set(0),
        created_at: sea_orm::ActiveValue::NotSet,
        updated_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(db)
    .await
    .expect("insert synthetic strategy");
    strategy_id
}
