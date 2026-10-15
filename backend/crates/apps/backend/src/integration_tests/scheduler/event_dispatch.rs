use std::{sync::Arc, time::Duration as StdDuration};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use core_application::{
    agent_task_client::{
        AgentTaskClient, AgentTaskError, AgentTaskRef, AgentTaskState, AgentTaskStatus,
        FakeAgentTaskClient, SubmitAgentTask,
    },
    event::{EventRepository, NewEvent},
    strategy_task::DEFAULT_PURPOSE,
    trigger::{NewTrigger, TriggerKind, TriggerRepository},
    unit_of_work::UnitOfWork,
};
use gateway_postgres::{
    DatabaseHandle, PostgresEventRepository, PostgresTriggerRepository, PostgresUnitOfWork,
    entities::sea_orm_active_enums::StrategyTaskPhase,
    entities::{event as event_entity, event_firing, strategy_task},
};
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Value, json};
use tokio::sync::Notify;
use uuid::Uuid;

use crate::{services::use_cases::build_use_cases_without_agent_task_client, testing};
use core_application::event_dispatch::MAX_EVENT_CHAIN_DEPTH;

async fn create_event(
    db: &DatabaseHandle,
    event_type: &str,
    payload: Value,
    chain_depth: i64,
) -> Uuid {
    let repository = PostgresEventRepository::new(db.clone());
    let unit_of_work = PostgresUnitOfWork::new(db.clone());
    let transaction = unit_of_work.begin().await.expect("begin transaction");
    let event = repository
        .create(
            &transaction,
            NewEvent {
                event_type: event_type.to_string(),
                payload,
                chain_depth: Some(chain_depth),
            },
        )
        .await
        .expect("create event");
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit transaction");
    event.event_id
}

async fn create_event_trigger(
    db: &DatabaseHandle,
    strategy_id: Uuid,
    event_type: &str,
    event_match: Option<Value>,
    prompt_template: &str,
) -> Uuid {
    let repository = PostgresTriggerRepository::new(db.clone());
    let unit_of_work = PostgresUnitOfWork::new(db.clone());
    let trigger_id = Uuid::new_v4();
    let transaction = unit_of_work.begin().await.expect("begin transaction");
    repository
        .create(
            &transaction,
            NewTrigger {
                trigger_id,
                strategy_id,
                purpose: None,
                kind: TriggerKind::Event,
                schedule: None,
                hook_slug: None,
                event_type: Some(event_type.to_string()),
                event_match,
                prompt_template: prompt_template.to_string(),
                enabled: true,
                business_days_only: false,
            },
        )
        .await
        .expect("create event trigger");
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit transaction");
    trigger_id
}

#[derive(Debug, PartialEq, Eq)]
struct TaskShape {
    strategy_id: Uuid,
    source: String,
    prompt: String,
    phase: StrategyTaskPhase,
}

impl From<strategy_task::Model> for TaskShape {
    fn from(task: strategy_task::Model) -> Self {
        Self {
            strategy_id: task.strategy_id,
            source: task.source,
            prompt: task.prompt,
            phase: task.phase,
        }
    }
}

fn task_shape(strategy_id: Uuid, prompt: &str) -> TaskShape {
    TaskShape {
        strategy_id,
        source: "event".to_string(),
        prompt: prompt.to_string(),
        phase: StrategyTaskPhase::Running,
    }
}

async fn tasks(db: &DatabaseHandle) -> Vec<TaskShape> {
    strategy_task::Entity::find()
        .all(db)
        .await
        .expect("list strategy tasks")
        .into_iter()
        .map(TaskShape::from)
        .collect()
}

async fn firing_trigger_ids(db: &DatabaseHandle) -> Vec<Uuid> {
    event_firing::Entity::find()
        .all(db)
        .await
        .expect("list event firings")
        .into_iter()
        .map(|firing| firing.trigger_id)
        .collect()
}

async fn event_dispatch_state(db: &DatabaseHandle, event_id: Uuid) -> (Uuid, bool) {
    let event = event_entity::Entity::find_by_id(event_id)
        .one(db)
        .await
        .expect("read event")
        .expect("event exists");
    (event.event_id, event.dispatched_at.is_some())
}

async fn failed_event_shapes(db: &DatabaseHandle) -> Vec<Value> {
    event_entity::Entity::find()
        .filter(event_entity::Column::EventType.eq("strategy_task.failed"))
        .all(db)
        .await
        .expect("list failure events")
        .into_iter()
        .map(|event| {
            let mut payload = event.payload;
            if let Some(payload) = payload.as_object_mut() {
                payload.insert("task_id".to_string(), json!("dynamic-task-id"));
                payload.insert("strategy_id".to_string(), json!("dynamic-strategy-id"));
            }
            json!({
                "event_type": event.event_type,
                "payload": payload,
                "chain_depth": event.chain_depth,
                "dispatched": event.dispatched_at.is_some(),
            })
        })
        .collect()
}

async fn submitted_tasks(fake: &FakeAgentTaskClient) -> Vec<(Uuid, String, Option<String>)> {
    fake.submitted
        .lock()
        .await
        .iter()
        .map(|task: &SubmitAgentTask| (task.strategy_id, task.prompt.clone(), task.purpose.clone()))
        .collect()
}

struct BlockingSubmitAgentTaskClient {
    submit_started: Notify,
    resume_submit: Notify,
}

impl BlockingSubmitAgentTaskClient {
    fn new() -> Self {
        Self {
            submit_started: Notify::new(),
            resume_submit: Notify::new(),
        }
    }
}

#[async_trait]
impl AgentTaskClient for BlockingSubmitAgentTaskClient {
    async fn submit(&self, _request: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError> {
        self.submit_started.notify_one();
        self.resume_submit.notified().await;
        Ok(AgentTaskRef {
            task_id: "synthetic-agent-task".to_string(),
        })
    }

    async fn get(&self, _task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
        Ok(AgentTaskStatus {
            state: AgentTaskState::Working,
            result_text: None,
            error_message: None,
            error_kind: None,
            steps: None,
        })
    }
}

#[backend_test_macros::database_test]
async fn dispatches_matching_triggers_and_expands_payload(db: DatabaseHandle) {
    let strategy_id = testing::insert_test_strategy(&db, "synthetic strategy").await;
    testing::agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent configuration");
    let event_id = create_event(
        &db,
        "sample_process.finished",
        json!({"state": "ready", "value": "synthetic"}),
        0,
    )
    .await;
    let matching_trigger_id = create_event_trigger(
        &db,
        strategy_id,
        "sample_process.finished",
        Some(json!({"state": {"eq": "ready"}})),
        "value={{payload.value}}",
    )
    .await;
    create_event_trigger(
        &db,
        strategy_id,
        "sample_process.finished",
        Some(json!({"state": {"eq": "waiting"}})),
        "unused prompt",
    )
    .await;

    let fake = Arc::new(FakeAgentTaskClient::new());
    let dispatch =
        crate::services::use_cases::build_use_cases(db.clone(), fake.clone()).event_dispatch_job();
    let outcome = dispatch.enqueue_dispatch().await;
    let completed_dispatch = tokio::time::timeout(StdDuration::from_secs(2), async {
        loop {
            let event = event_dispatch_state(&db, event_id).await;
            let submitted = submitted_tasks(&fake).await;
            let current_tasks = tasks(&db).await;
            let firings = firing_trigger_ids(&db).await;
            if event.1 && !submitted.is_empty() {
                break (event, current_tasks, firings, submitted);
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    })
    .await
    .expect("background event dispatch should finish");

    assert_eq!(
        (
            outcome.map_err(|error| error.to_string()),
            completed_dispatch.1,
            completed_dispatch.2,
            completed_dispatch.3,
            completed_dispatch.0,
        ),
        (
            Ok(()),
            vec![task_shape(strategy_id, "value=synthetic")],
            vec![matching_trigger_id],
            vec![(
                strategy_id,
                "value=synthetic".to_string(),
                Some(DEFAULT_PURPOSE.to_string()),
            )],
            (event_id, true),
        ),
    );
}

#[backend_test_macros::database_test]
async fn enqueue_dispatch_returns_while_agent_submission_is_blocked(db: DatabaseHandle) {
    let strategy_id = testing::insert_test_strategy(&db, "synthetic strategy").await;
    testing::agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent configuration");
    let event_id = create_event(&db, "sample_process.finished", json!({}), 0).await;
    create_event_trigger(
        &db,
        strategy_id,
        "sample_process.finished",
        None,
        "synthetic prompt",
    )
    .await;
    let fake = Arc::new(BlockingSubmitAgentTaskClient::new());
    let dispatch =
        crate::services::use_cases::build_use_cases(db.clone(), fake.clone()).event_dispatch_job();

    let enqueue_result =
        tokio::time::timeout(StdDuration::from_millis(100), dispatch.enqueue_dispatch()).await;
    let submission_started =
        tokio::time::timeout(StdDuration::from_secs(2), fake.submit_started.notified())
            .await
            .is_ok();
    fake.resume_submit.notify_one();
    let event_dispatched = tokio::time::timeout(StdDuration::from_secs(2), async {
        loop {
            if event_dispatch_state(&db, event_id).await.1 {
                break true;
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or(false);

    assert_eq!(
        (
            enqueue_result.map(|result| result.map_err(|error| error.to_string())),
            submission_started,
            event_dispatched,
        ),
        (Ok(Ok(())), true, true),
    );
}

#[backend_test_macros::database_test]
async fn repeated_dispatch_submits_each_event_trigger_once(db: DatabaseHandle) {
    let strategy_id = testing::insert_test_strategy(&db, "synthetic strategy").await;
    testing::agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent configuration");
    create_event(&db, "sample_process.finished", json!({}), 0).await;
    let trigger_id = create_event_trigger(
        &db,
        strategy_id,
        "sample_process.finished",
        None,
        "synthetic prompt",
    )
    .await;
    let fake = Arc::new(FakeAgentTaskClient::new());
    let dispatch = build_use_cases_without_agent_task_client(db.clone()).event_dispatch();

    let first = dispatch.dispatch_and_cleanup(fake.as_ref()).await;
    let second = dispatch.dispatch_and_cleanup(fake.as_ref()).await;

    assert_eq!(
        (
            first.map_err(|error| error.to_string()),
            second.map_err(|error| error.to_string()),
            tasks(&db).await,
            firing_trigger_ids(&db).await,
            submitted_tasks(&fake).await.len(),
        ),
        (
            Ok((1, 0)),
            Ok((0, 0)),
            vec![task_shape(strategy_id, "synthetic prompt")],
            vec![trigger_id],
            1,
        ),
    );
}

#[backend_test_macros::database_test]
async fn dispatch_preserves_failure_events_for_event_triggered_tasks(db: DatabaseHandle) {
    let strategy_id = testing::insert_test_strategy(&db, "synthetic strategy").await;
    testing::agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent configuration");
    let source_event_id = create_event(&db, "sample_process.finished", json!({}), 0).await;
    let trigger_id = create_event_trigger(
        &db,
        strategy_id,
        "sample_process.finished",
        None,
        "synthetic prompt",
    )
    .await;
    let fake = Arc::new(FakeAgentTaskClient::new());
    fake.set_submit_error(AgentTaskError::NotConfigured).await;
    let dispatch =
        crate::services::use_cases::build_use_cases(db.clone(), fake.clone()).event_dispatch_job();

    let outcome = dispatch.enqueue_dispatch().await;
    let completed_dispatch = tokio::time::timeout(StdDuration::from_secs(2), async {
        loop {
            let task = strategy_task::Entity::find()
                .one(&db)
                .await
                .expect("read event-triggered task");
            let failed_events = failed_event_shapes(&db).await;
            if let Some(task) = task
                && task.phase == StrategyTaskPhase::Failed
                && !failed_events.is_empty()
                && failed_events
                    .iter()
                    .all(|event| event["dispatched"] == json!(true))
                && event_dispatch_state(&db, source_event_id).await.1
            {
                break (task, failed_events);
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    })
    .await
    .expect("background dispatch should retain failure events");
    let task = completed_dispatch.0;

    assert_eq!(
        (
            outcome.map_err(|error| error.to_string()),
            event_dispatch_state(&db, source_event_id).await,
            (
                task.strategy_id,
                task.source,
                task.prompt,
                task.phase,
                task.error_summary,
            ),
            completed_dispatch.1,
            firing_trigger_ids(&db).await,
            submitted_tasks(&fake).await,
        ),
        (
            Ok(()),
            (source_event_id, true),
            (
                strategy_id,
                "event".to_string(),
                "synthetic prompt".to_string(),
                StrategyTaskPhase::Failed,
                Some(
                    "agent task submission failed: agent task client is not configured".to_string()
                ),
            ),
            vec![json!({
                "event_type": "strategy_task.failed",
                "payload": {
                    "task_id": "dynamic-task-id",
                    "strategy_id": "dynamic-strategy-id",
                    "purpose": DEFAULT_PURPOSE,
                    "result_text": null,
                    "error_summary": "agent task submission failed: agent task client is not configured",
                },
                "chain_depth": 1,
                "dispatched": true,
            })],
            vec![trigger_id],
            Vec::<(Uuid, String, Option<String>)>::new(),
        ),
    );
}

#[backend_test_macros::database_test]
async fn invalid_trigger_configuration_does_not_block_later_events(db: DatabaseHandle) {
    let strategy_id = testing::insert_test_strategy(&db, "synthetic strategy").await;
    testing::agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent configuration");
    let invalid_event_id = create_event(&db, "sample_process.invalid", json!({}), 0).await;
    let valid_event_id = create_event(&db, "sample_process.valid", json!({}), 0).await;
    create_event_trigger(&db, strategy_id, "sample_process.invalid", None, "   ").await;
    let valid_trigger_id = create_event_trigger(
        &db,
        strategy_id,
        "sample_process.valid",
        None,
        "synthetic prompt",
    )
    .await;
    let fake = Arc::new(FakeAgentTaskClient::new());
    let dispatch = build_use_cases_without_agent_task_client(db.clone()).event_dispatch();

    let outcome = dispatch.dispatch_and_cleanup(fake.as_ref()).await;

    assert_eq!(
        (
            outcome.map_err(|error| error.to_string()),
            vec![
                event_dispatch_state(&db, invalid_event_id).await,
                event_dispatch_state(&db, valid_event_id).await,
            ],
            tasks(&db).await,
            firing_trigger_ids(&db).await,
            submitted_tasks(&fake).await,
        ),
        (
            Err("failed to process 1 event trigger(s)".to_string()),
            vec![(invalid_event_id, true), (valid_event_id, true)],
            vec![task_shape(strategy_id, "synthetic prompt")],
            vec![valid_trigger_id],
            vec![(
                strategy_id,
                "synthetic prompt".to_string(),
                Some(DEFAULT_PURPOSE.to_string()),
            )],
        ),
    );
}

#[backend_test_macros::database_test]
async fn dispatches_at_the_limit_and_skips_events_above_it(db: DatabaseHandle) {
    let strategy_id = testing::insert_test_strategy(&db, "synthetic strategy").await;
    testing::agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent configuration");
    let allowed_event_id = create_event(
        &db,
        "sample_process.within_limit",
        json!({}),
        MAX_EVENT_CHAIN_DEPTH,
    )
    .await;
    let skipped_event_id = create_event(
        &db,
        "sample_process.over_limit",
        json!({}),
        MAX_EVENT_CHAIN_DEPTH + 1,
    )
    .await;
    let allowed_trigger_id = create_event_trigger(
        &db,
        strategy_id,
        "sample_process.within_limit",
        None,
        "synthetic prompt",
    )
    .await;
    create_event_trigger(
        &db,
        strategy_id,
        "sample_process.over_limit",
        None,
        "unused prompt",
    )
    .await;
    let fake = Arc::new(FakeAgentTaskClient::new());
    let dispatch = build_use_cases_without_agent_task_client(db.clone()).event_dispatch();

    let outcome = dispatch.dispatch_and_cleanup(fake.as_ref()).await;

    assert_eq!(
        (
            outcome.map_err(|error| error.to_string()),
            vec![
                event_dispatch_state(&db, allowed_event_id).await,
                event_dispatch_state(&db, skipped_event_id).await,
            ],
            tasks(&db).await,
            firing_trigger_ids(&db).await,
            submitted_tasks(&fake).await,
        ),
        (
            Ok((2, 0)),
            vec![(allowed_event_id, true), (skipped_event_id, true)],
            vec![task_shape(strategy_id, "synthetic prompt")],
            vec![allowed_trigger_id],
            vec![(
                strategy_id,
                "synthetic prompt".to_string(),
                Some(DEFAULT_PURPOSE.to_string()),
            )],
        ),
    );
}

#[backend_test_macros::database_test]
async fn deletes_events_older_than_the_retention_period(db: DatabaseHandle) {
    let event_id = create_event(&db, "sample_process.finished", json!({}), 0).await;
    event_entity::Entity::update_many()
        .col_expr(
            event_entity::Column::OccurredAt,
            Expr::value((Utc::now() - Duration::days(31)).fixed_offset()),
        )
        .filter(event_entity::Column::EventId.eq(event_id))
        .exec(&db)
        .await
        .expect("set old event time");
    let fake = Arc::new(FakeAgentTaskClient::new());
    let dispatch = build_use_cases_without_agent_task_client(db.clone()).event_dispatch();

    let outcome = dispatch.dispatch_and_cleanup(fake.as_ref()).await;
    let event_exists = event_entity::Entity::find_by_id(event_id)
        .one(&db)
        .await
        .expect("read event")
        .is_some();

    assert_eq!(
        (outcome.map_err(|error| error.to_string()), event_exists),
        (Ok((0, 1)), false)
    );
}
