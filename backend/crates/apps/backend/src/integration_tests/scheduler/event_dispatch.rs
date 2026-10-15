use std::sync::Arc;

use chrono::{Duration, Utc};
use core_application::{
    agent_task_client::{FakeAgentTaskClient, SubmitAgentTask},
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
use uuid::Uuid;

use crate::{services::use_cases::build_use_cases, testing};
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

async fn submitted_tasks(fake: &FakeAgentTaskClient) -> Vec<(Uuid, String, Option<String>)> {
    fake.submitted
        .lock()
        .await
        .iter()
        .map(|task: &SubmitAgentTask| (task.strategy_id, task.prompt.clone(), task.purpose.clone()))
        .collect()
}

#[backend_test_macros::database_test]
async fn dispatches_matching_triggers_and_expands_payload(db: DatabaseHandle) {
    let strategy_id = testing::insert_test_strategy(&db, "synthetic strategy").await;
    testing::agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent configuration");
    create_event(
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
    let dispatch = build_use_cases(db.clone()).event_dispatch();
    let outcome = dispatch.dispatch_and_cleanup(fake.as_ref()).await;

    assert_eq!(
        (
            outcome.map_err(|error| error.to_string()),
            tasks(&db).await,
            firing_trigger_ids(&db).await,
            submitted_tasks(&fake).await,
        ),
        (
            Ok((1, 0)),
            vec![task_shape(strategy_id, "value=synthetic")],
            vec![matching_trigger_id],
            vec![(
                strategy_id,
                "value=synthetic".to_string(),
                Some(DEFAULT_PURPOSE.to_string()),
            )],
        ),
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
    let dispatch = build_use_cases(db.clone()).event_dispatch();

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
async fn skips_triggers_when_chain_depth_exceeds_the_limit(db: DatabaseHandle) {
    let strategy_id = testing::insert_test_strategy(&db, "synthetic strategy").await;
    testing::agent_config::create(&db, DEFAULT_PURPOSE.to_string())
        .await
        .expect("insert test agent configuration");
    let event_id = create_event(
        &db,
        "sample_process.finished",
        json!({}),
        MAX_EVENT_CHAIN_DEPTH + 1,
    )
    .await;
    create_event_trigger(
        &db,
        strategy_id,
        "sample_process.finished",
        None,
        "synthetic prompt",
    )
    .await;
    let fake = Arc::new(FakeAgentTaskClient::new());
    let dispatch = build_use_cases(db.clone()).event_dispatch();

    let outcome = dispatch.dispatch_and_cleanup(fake.as_ref()).await;
    let event_dispatched = event_entity::Entity::find_by_id(event_id)
        .one(&db)
        .await
        .expect("read event")
        .is_some_and(|event| event.dispatched_at.is_some());

    assert_eq!(
        (
            outcome.map_err(|error| error.to_string()),
            tasks(&db).await,
            firing_trigger_ids(&db).await,
            submitted_tasks(&fake).await,
            event_dispatched,
        ),
        (Ok((1, 0)), vec![], vec![], vec![], true),
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
    let dispatch = build_use_cases(db.clone()).event_dispatch();

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
